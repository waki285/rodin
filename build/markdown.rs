use crate::frontmatter::FrontMatter;
use anyhow::Result;
use serde_json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn build_markdown(
    metas: &mut [FrontMatter],
    generated_md_dir: &str,
    pandoc_filter: &str,
) -> Result<bool> {
    if !pandoc_available()? {
        println!("cargo:warning=pandoc not found; skipping markdown generation");
        return Ok(false);
    }

    let out_dir = PathBuf::from(generated_md_dir);
    fs::create_dir_all(&out_dir)?;

    let mut all_ok = true;
    for meta in metas.iter_mut() {
        let slug = &meta.slug;
        let html_rel = &meta.html;
        let html_path = PathBuf::from("static").join(html_rel);

        let md_rel = format!("generated/md/{slug}.md");
        let md_path = PathBuf::from("static").join(&md_rel);

        if let Err(e) = run_pandoc_to_markdown(&html_path, &md_path, pandoc_filter) {
            println!("cargo:warning=pandoc failed for {slug}: {e}");
            all_ok = false;
            break;
        } else {
            meta.markdown = Some(md_rel);
        }
    }

    if !all_ok {
        Ok(false)
    } else {
        Ok(true)
    }
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

fn run_pandoc_to_markdown(html_path: &Path, md_path: &Path, pandoc_filter: &str) -> Result<()> {
    if !html_path.exists() {
        return Err(anyhow::anyhow!(
            "source HTML not found: {}",
            html_path.display()
        ));
    }
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new("pandoc")
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

fn pandoc_available() -> Result<bool> {
    let status = Command::new("pandoc").arg("--version").status();
    match status {
        Ok(s) => Ok(s.success()),
        Err(_) => Ok(false),
    }
}
