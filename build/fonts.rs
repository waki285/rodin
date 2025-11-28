use anyhow::{Context, Result};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const REGULAR_FONT_SRC: &str = "static/fonts/IBMPlexSansJP-Regular.ttf";
const MEDIUM_FONT_SRC: &str = "static/fonts/IBMPlexSansJP-Medium.ttf";
const SEMIBOLD_FONT_SRC: &str = "static/fonts/IBMPlexSansJP-SemiBold.ttf";
const BOLD_FONT_SRC: &str = "static/fonts/IBMPlexSansJP-Bold.ttf";

const REGULAR_FONT_TTF_OUT: &str = "static/build/IBMPlexSansJP-Regular.subset.ttf";
const REGULAR_FONT_WOFF2_OUT: &str = "static/build/IBMPlexSansJP-Regular.subset.woff2";
const MEDIUM_FONT_TTF_OUT: &str = "static/build/IBMPlexSansJP-Medium.subset.ttf";
const MEDIUM_FONT_WOFF2_OUT: &str = "static/build/IBMPlexSansJP-Medium.subset.woff2";
const SEMIBOLD_FONT_TTF_OUT: &str = "static/build/IBMPlexSansJP-Semibold.subset.ttf";
const SEMIBOLD_FONT_WOFF2_OUT: &str = "static/build/IBMPlexSansJP-Semibold.subset.woff2";
const BOLD_FONT_TTF_OUT: &str = "static/build/IBMPlexSansJP-Bold.subset.ttf";
const BOLD_FONT_WOFF2_OUT: &str = "static/build/IBMPlexSansJP-Bold.subset.woff2";

const TEXT_SOURCES: &[&str] = &[
    "src/app/handlers.rs",      // not_found_response HTML
    "src/components.rs",        // top/profile/blog chrome
    "src/components/search.rs", // search page strings
    "static/app.js",            // UI strings in client JS
    "static/preamble.typ",      // preamble for typst
];

pub fn subset_regular_font() -> Result<()> {
    println!("cargo:rerun-if-changed={REGULAR_FONT_SRC}");
    println!("cargo:rerun-if-changed={BOLD_FONT_SRC}");
    println!("cargo:rerun-if-changed={SEMIBOLD_FONT_SRC}");
    println!("cargo:rerun-if-changed={MEDIUM_FONT_SRC}");
    println!("cargo:rerun-if-changed=content");
    for src in TEXT_SOURCES {
        println!("cargo:rerun-if-changed={src}");
    }

    let glyphs = collect_glyphs()?;
    if glyphs.is_empty() {
        println!("cargo:warning=No glyphs collected for font subsetting; skipping.");
        return Ok(());
    }

    subset_font(
        MEDIUM_FONT_SRC,
        MEDIUM_FONT_TTF_OUT,
        MEDIUM_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    subset_font(
        SEMIBOLD_FONT_SRC,
        SEMIBOLD_FONT_TTF_OUT,
        SEMIBOLD_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    subset_font(
        REGULAR_FONT_SRC,
        REGULAR_FONT_TTF_OUT,
        REGULAR_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    subset_font(
        BOLD_FONT_SRC,
        BOLD_FONT_TTF_OUT,
        BOLD_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    Ok(())
}

fn collect_glyphs() -> Result<BTreeSet<char>> {
    let mut set = BTreeSet::new();

    for path in TEXT_SOURCES {
        collect_from_file(path, &mut set)?;
    }
    collect_from_content_dir("content", &mut set)?;

    set.insert(' ');
    set.insert('\u{00A0}'); // non-breaking space

    Ok(set)
}

fn collect_from_content_dir(dir: &str, set: &mut BTreeSet<char>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading directory {dir}"))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "typ") {
            collect_from_file(&path, set)?;
        }
    }
    Ok(())
}

fn collect_from_file(path: impl AsRef<Path>, set: &mut BTreeSet<char>) -> Result<()> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref)
        .with_context(|| format!("failed to read glyph source {}", path_ref.display()))?;
    for ch in content.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            continue;
        }
        set.insert(ch);
    }
    Ok(())
}

fn write_if_changed(path: &str, data: &[u8]) -> Result<()> {
    let dst = PathBuf::from(path);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let need_write = match fs::read(&dst) {
        Ok(existing) => existing != data,
        Err(_) => true,
    };
    if need_write {
        fs::write(&dst, data).with_context(|| format!("failed to write {}", dst.display()))?;
    }
    Ok(())
}

fn subset_font(src: &str, ttf_out: &str, woff2_out: &str, glyphs: &BTreeSet<char>) -> Result<()> {
    let font = fs::read(src).with_context(|| format!("failed to read {}", src))?;
    let subset = hb_subset::subset(&font, glyphs.iter().copied())
        .with_context(|| format!("hb-subset failed for {}", src))?;
    write_if_changed(ttf_out, &subset)?;
    compress_to_woff2(ttf_out, woff2_out)
}

fn compress_to_woff2(ttf_path: &str, woff2_path: &str) -> Result<()> {
    let ttf = PathBuf::from(ttf_path);
    if let Some(parent) = ttf.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let status = Command::new("woff2_compress")
        .arg(&ttf)
        .status()
        .with_context(|| "failed to spawn woff2_compress (install with `brew install woff2`)")?;
    if !status.success() {
        anyhow::bail!("woff2_compress exited with {}", status);
    }

    let produced = ttf.with_extension("woff2");
    if !produced.exists() {
        anyhow::bail!("woff2_compress did not produce {}", produced.display());
    }
    let target = PathBuf::from(woff2_path);
    if produced != target {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        fs::rename(&produced, &target).with_context(|| {
            format!(
                "failed to move {} to {}",
                produced.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}
