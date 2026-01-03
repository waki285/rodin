use anyhow::Result;

#[path = "build/assets.rs"]
mod assets;
#[path = "src/constants.rs"]
pub mod constants;
#[path = "src/fonts.rs"]
mod fonts;
#[path = "src/frontmatter.rs"]
mod frontmatter;
#[path = "build/markdown.rs"]
mod markdown;
#[path = "build/posts.rs"]
mod posts;
#[path = "build/sitemap.rs"]
mod sitemap;

const PREAMBLE_PATH: &str = "content/_preamble.typ";
const GENERATED_DIR: &str = "static/generated";
const GENERATED_MD_DIR: &str = "static/generated/md";
const PANDOC_FILTER: &str = "scripts/pandoc/html-to-md.lua";
const MARKDOWN_ENV_KEY: &str = "RODIN_MARKDOWN_ENABLED";
const SITEMAP_PATH: &str = "static/generated/sitemap.xml";

#[cfg(not(windows))]
fn subset_regular_font() -> Result<()> {
    println!("cargo:rerun-if-changed={}", fonts::REGULAR_FONT_SRC);
    println!("cargo:rerun-if-changed={}", fonts::BOLD_FONT_SRC);
    println!("cargo:rerun-if-changed={}", fonts::SEMIBOLD_FONT_SRC);
    println!("cargo:rerun-if-changed=content");
    for src in fonts::TEXT_SOURCES {
        println!("cargo:rerun-if-changed={src}");
    }

    let glyphs = fonts::collect_glyphs()?;
    if glyphs.is_empty() {
        println!("cargo:warning=No glyphs collected for font subsetting; skipping.");
        return Ok(());
    }

    // Collect minimal glyphs for Bold font (H1 headings + extra chars)
    let bold_glyphs = fonts::collect_bold_glyphs()?;

    fonts::subset_font(
        fonts::SEMIBOLD_FONT_SRC,
        fonts::SEMIBOLD_FONT_TTF_OUT,
        fonts::SEMIBOLD_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    fonts::subset_font(
        fonts::REGULAR_FONT_SRC,
        fonts::REGULAR_FONT_TTF_OUT,
        fonts::REGULAR_FONT_WOFF2_OUT,
        &glyphs,
    )?;
    // Bold font uses minimal subset (H1 headings only + extra chars)
    fonts::subset_font(
        fonts::BOLD_FONT_SRC,
        fonts::BOLD_FONT_TTF_OUT,
        fonts::BOLD_FONT_WOFF2_OUT,
        &bold_glyphs,
    )?;
    Ok(())
}

#[cfg(windows)]
fn subset_regular_font() -> Result<()> {
    use std::{fs, path::Path};

    println!("cargo:rerun-if-changed={}", fonts::REGULAR_FONT_SRC);
    println!("cargo:rerun-if-changed={}", fonts::BOLD_FONT_SRC);
    println!("cargo:rerun-if-changed={}", fonts::SEMIBOLD_FONT_SRC);

    if !Path::new("static/build").exists() {
        fs::create_dir_all("static/build")?;
    }

    let fonts_list = [
        (
            fonts::REGULAR_FONT_SRC,
            fonts::REGULAR_FONT_TTF_OUT,
            fonts::REGULAR_FONT_WOFF2_OUT,
        ),
        (
            fonts::SEMIBOLD_FONT_SRC,
            fonts::SEMIBOLD_FONT_TTF_OUT,
            fonts::SEMIBOLD_FONT_WOFF2_OUT,
        ),
        (
            fonts::BOLD_FONT_SRC,
            fonts::BOLD_FONT_TTF_OUT,
            fonts::BOLD_FONT_WOFF2_OUT,
        ),
    ];

    for (src, ttf_dst, woff2_dst) in fonts_list {
        fs::copy(src, ttf_dst)?;
        fonts::compress_to_woff2(ttf_dst, woff2_dst)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    if is_rust_analyzer() {
        return Ok(());
    }

    // ETag
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=content");
    println!("cargo:rerun-if-changed=static/app.js");
    println!("cargo:rerun-if-changed=static/home.js");
    println!("cargo:rerun-if-changed=static/css");
    println!("cargo:rerun-if-changed={PANDOC_FILTER}");

    println!("cargo:rustc-env={MARKDOWN_ENV_KEY}=false");

    // フォントを先に生成してから、アセット処理（ハッシュ化含む）を行う
    subset_regular_font()?;
    assets::minify_assets()?;

    let mut metas = posts::build_posts(PREAMBLE_PATH, GENERATED_DIR)?;
    let markdown_ok = markdown::build_markdown(&mut metas, GENERATED_MD_DIR, PANDOC_FILTER)?;
    if markdown_ok {
        println!("cargo:rustc-env={MARKDOWN_ENV_KEY}=true");
    } else {
        // keep index consistent
        for m in metas.iter_mut() {
            m.markdown = None;
        }
    }
    markdown::write_index(&metas, GENERATED_DIR)?;
    // index.json を読むのでこの順
    posts::build_home(PREAMBLE_PATH, GENERATED_DIR)?;
    posts::build_profile(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_meta = posts::build_pgp(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_meta_ref = pgp_meta.as_ref();
    sitemap::write_sitemap(&metas, pgp_meta_ref, constants::SITE_URL, SITEMAP_PATH)?;
    Ok(())
}

fn is_rust_analyzer() -> bool {
    std::env::var("RUST_ANALYZER").is_ok()
        || std::env::var("RUST_ANALYZER_INTERNALS_DO_NOT_USE").is_ok()
        || std::env::var("RA_RUNNING").is_ok()
}
