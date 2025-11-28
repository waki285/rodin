use anyhow::Result;

#[path = "build/assets.rs"]
mod assets;
#[path = "build/fonts.rs"]
mod fonts;
#[path = "build/markdown.rs"]
mod markdown;
#[path = "build/posts.rs"]
mod posts;
#[path = "build/sitemap.rs"]
mod sitemap;
#[path = "build/tailwind.rs"]
mod tailwind;
// Reuse the runtime FrontMatter definition to avoid duplication.
#[path = "src/frontmatter.rs"]
mod frontmatter;

const PREAMBLE_PATH: &str = "static/preamble.typ";
const GENERATED_DIR: &str = "static/generated";
const GENERATED_MD_DIR: &str = "static/generated/md";
const PANDOC_FILTER: &str = "scripts/pandoc/noimg.lua";
const MARKDOWN_ENV_KEY: &str = "RODIN_MARKDOWN_ENABLED";
const SITE_URL: &str = "https://suzuneu.com";
const SITEMAP_PATH: &str = "static/generated/sitemap.xml";

fn main() -> Result<()> {
    if is_rust_analyzer() {
        return Ok(());
    }

    // expose git hash for ETag
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

    println!("cargo:rerun-if-changed=static/input.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=content");
    println!("cargo:rerun-if-changed=static/app.js");
    println!("cargo:rerun-if-changed=static/custom.css");
    println!("cargo:rerun-if-changed=static/tailwind-fallback.css");
    println!("cargo:rerun-if-changed={PREAMBLE_PATH}");
    println!("cargo:rerun-if-changed={PANDOC_FILTER}");

    // default: markdown disabled; flip to true only if generation succeeds
    println!("cargo:rustc-env={MARKDOWN_ENV_KEY}=false");

    tailwind::build_tailwind();
    assets::minify_assets()?;
    fonts::subset_regular_font()?;

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
    // Build home after index.json exists so Typst can read metadata.
    posts::build_home(PREAMBLE_PATH, GENERATED_DIR)?;
    posts::build_profile(PREAMBLE_PATH, GENERATED_DIR)?;
    sitemap::write_sitemap(&metas, SITE_URL, SITEMAP_PATH)?;
    Ok(())
}

fn is_rust_analyzer() -> bool {
    std::env::var("RUST_ANALYZER").is_ok()
        || std::env::var("RUST_ANALYZER_INTERNALS_DO_NOT_USE").is_ok()
        || std::env::var("RA_RUNNING").is_ok()
}
