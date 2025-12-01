use anyhow::Result;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[path = "../../src/frontmatter.rs"]
mod frontmatter;
#[path = "../../build/markdown.rs"]
mod markdown;
#[path = "../../build/posts.rs"]
mod posts;
#[path = "../../build/sitemap.rs"]
mod sitemap;

const PREAMBLE_PATH: &str = "static/preamble.typ";
const GENERATED_DIR: &str = "static/generated";
const GENERATED_MD_DIR: &str = "static/generated/md";
const PANDOC_FILTER: &str = "scripts/pandoc/noimg.lua";
const DEFAULT_SITE_URL: &str = "https://suzuneu.com";
const DEFAULT_SITEMAP_PATH: &str = "static/generated/sitemap.xml";
const DEFAULT_RELOAD_URL: &str = "http://127.0.0.1:3000/__admin/reload";

fn main() -> Result<()> {
    let mut skip_markdown = false;
    let mut site_url = DEFAULT_SITE_URL.to_string();
    let mut do_reload = false;
    let mut reload_url: Option<String> = None;
    let mut reload_token: Option<String> = None;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--skip-markdown" | "--no-markdown" | "--no-md" => {
                skip_markdown = true;
            }
            "--reload" => {
                do_reload = true;
            }
            _ if arg.starts_with("--reload-url=") => {
                do_reload = true;
                reload_url = Some(arg.trim_start_matches("--reload-url=").to_string());
            }
            _ if arg.starts_with("--reload-token=") => {
                reload_token = Some(arg.trim_start_matches("--reload-token=").to_string());
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

    if do_reload {
        let url = reload_url
            .or_else(|| std::env::var("RODIN_RELOAD_URL").ok())
            .unwrap_or_else(|| DEFAULT_RELOAD_URL.to_string());
        let token = reload_token.or_else(|| std::env::var("RODIN_RELOAD_TOKEN").ok());
        trigger_reload(&url, token.as_deref())?;
    }

    Ok(())
}

fn trigger_reload(url: &str, token: Option<&str>) -> Result<()> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("reload URL must start with http://"))?;

    let (host_port, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (rest, "/".to_string()),
    };

    let mut stream = TcpStream::connect_timeout(&host_port.parse()?, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Length: 0\r\nConnection: close\r\n",
        path, host_port
    );
    if let Some(tok) = token {
        req.push_str(&format!("X-Rodin-Reload-Token: {}\r\n", tok));
    }
    req.push_str("\r\n");

    println!("POST {url} (reload)");
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    let status_line = buf.lines().next().unwrap_or("");
    if status_line.contains(" 200 ") {
        println!("reload succeeded");
        Ok(())
    } else {
        anyhow::bail!("reload failed: {}", status_line)
    }
}

fn print_help() {
    println!("Usage: rodin-content [--skip-markdown] [--site=BASE_URL]");
    println!(
        "  builds Typst articles in ./content into static/generated (HTML, index.json, sitemap)"
    );
    println!("  skips font steps; only content generation runs");
    println!("  --skip-markdown : do not run pandoc even if available");
    println!("  --site=URL      : override sitemap base (default {DEFAULT_SITE_URL})");
    println!("  --reload        : call POST {DEFAULT_RELOAD_URL} after build");
    println!("  --reload-url=U  : override reload URL (http:// only)");
    println!("  --reload-token=T: set X-Rodin-Reload-Token header");
}
