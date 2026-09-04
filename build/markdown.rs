use crate::frontmatter::FrontMatter;
use anyhow::Result;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn build_markdown(
    metas: &mut [FrontMatter],
    generated_md_dir: &str,
    pandoc_filter: &str,
) -> Result<bool> {
    let pandoc = match find_pandoc()? {
        Some(path) => path,
        None => {
            println!("cargo:warning=pandoc not found; skipping markdown generation");
            return Ok(false);
        }
    };

    let out_dir = PathBuf::from(generated_md_dir);
    fs::create_dir_all(&out_dir)?;

    let mut all_ok = true;
    for meta in metas.iter_mut() {
        let slug = &meta.slug;
        let html_rel = &meta.html;
        let html_path = PathBuf::from("static").join(html_rel);

        let md_rel = format!("generated/md/{slug}.md");
        let md_path = PathBuf::from("static").join(&md_rel);

        if let Err(e) = run_pandoc_to_markdown(&pandoc, &html_path, &md_path, pandoc_filter) {
            println!("cargo:warning=pandoc failed for {slug}: {e}");
            all_ok = false;
            break;
        } else {
            meta.markdown = Some(md_rel);
        }
    }

    if !all_ok { Ok(false) } else { Ok(true) }
}

pub fn write_index(metas: &[FrontMatter], generated_dir: &str) -> Result<()> {
    let out_dir = PathBuf::from(generated_dir);
    fs::create_dir_all(&out_dir)?;
    fs::write(
        out_dir.join("index.json"),
        serde_json::to_string_pretty(metas)?,
    )?;
    Ok(())
}

fn run_pandoc_to_markdown(
    pandoc: &str,
    html_path: &Path,
    md_path: &Path,
    pandoc_filter: &str,
) -> Result<()> {
    if !html_path.exists() {
        return Err(anyhow::anyhow!(
            "source HTML not found: {}",
            html_path.display()
        ));
    }
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new(pandoc)
        .args([
            "-f",
            "html",
            "-t",
            "gfm",
            "--wrap",
            "none",
            "--lua-filter",
            pandoc_filter,
            "-o",
            md_path.to_str().unwrap(),
            html_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| anyhow::anyhow!("pandoc invocation failed: {e}"))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "pandoc exited with status {status} for {}",
            html_path.display()
        ));
    }
    Ok(())
}

/// Locate pandoc in a few common places, including an override via PANDOC_BIN.
fn find_pandoc() -> Result<Option<String>> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(p) = env::var("PANDOC_BIN")
        && !p.trim().is_empty()
    {
        candidates.push(p);
    }
    // PATH fallback
    candidates.push("pandoc".to_string());
    // Typical Homebrew / system paths (in case PATH is restricted)
    candidates.push("/opt/homebrew/bin/pandoc".to_string());
    candidates.push("/usr/local/bin/pandoc".to_string());
    candidates.push("/usr/bin/pandoc".to_string());

    for cand in candidates {
        let status = Command::new(&cand).arg("--version").status();
        if let Ok(s) = status
            && s.success()
        {
            return Ok(Some(cand));
        }
    }
    Ok(None)
}
