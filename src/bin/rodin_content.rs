use anyhow::Result;

#[path = "../../build/posts.rs"]
mod posts;
#[path = "../../build/markdown.rs"]
mod markdown;
#[path = "../../build/sitemap.rs"]
mod sitemap;
#[path = "../../src/frontmatter.rs"]
mod frontmatter;

const PREAMBLE_PATH: &str = "static/preamble.typ";
const GENERATED_DIR: &str = "static/generated";
const GENERATED_MD_DIR: &str = "static/generated/md";
const PANDOC_FILTER: &str = "scripts/pandoc/noimg.lua";
const DEFAULT_SITE_URL: &str = "https://suzuneu.com";
const DEFAULT_SITEMAP_PATH: &str = "static/generated/sitemap.xml";

fn main() -> Result<()> {
    let mut skip_markdown = false;
    let mut site_url = DEFAULT_SITE_URL.to_string();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--skip-markdown" | "--no-markdown" | "--no-md" => {
                skip_markdown = true;
            }
            _ if arg.starts_with("--site=") => {
                site_url = arg.trim_start_matches("--site=").to_string();
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_help();
                return Ok(());
            }
        }
    }

    println!("rodin-content: generating HTML from Typst sources in ./content");

    let mut metas = posts::build_posts(PREAMBLE_PATH, GENERATED_DIR)?;

    if skip_markdown {
        for m in metas.iter_mut() {
            m.markdown = None;
        }
        println!("markdown generation skipped (--skip-markdown)");
    } else {
        match markdown::build_markdown(&mut metas, GENERATED_MD_DIR, PANDOC_FILTER) {
            Ok(true) => println!("markdown generated for {} posts", metas.len()),
            Ok(false) => {
                println!("markdown generation skipped (pandoc missing or failed); disabling markdown links");
                for m in metas.iter_mut() {
                    m.markdown = None;
                }
            }
            Err(e) => {
                println!("markdown generation error: {e}; disabling markdown links");
                for m in metas.iter_mut() {
                    m.markdown = None;
                }
            }
        }
    }

    markdown::write_index(&metas, GENERATED_DIR)?;
    posts::build_home(PREAMBLE_PATH, GENERATED_DIR)?;
    posts::build_profile(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_meta = posts::build_pgp(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_ref = pgp_meta.as_ref();
    sitemap::write_sitemap(&metas, pgp_ref, &site_url, DEFAULT_SITEMAP_PATH)?;

    println!("done. outputs are under {GENERATED_DIR}");
    Ok(())
}

fn print_help() {
    println!("Usage: rodin-content [--skip-markdown] [--site=BASE_URL]");
    println!("  builds Typst articles in ./content into static/generated (HTML, index.json, sitemap)");
    println!("  skips Tailwind/font steps; only content generation runs");
    println!("  --skip-markdown : do not run pandoc even if available");
    println!("  --site=URL      : override sitemap base (default {DEFAULT_SITE_URL})");
}
