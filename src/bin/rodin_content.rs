use anyhow::Result;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::json;
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
const PANDOC_FILTER: &str = "scripts/pandoc/html-to-md.lua";
const DEFAULT_SITE_URL: &str = "https://suzuneu.com";
const DEFAULT_SITEMAP_PATH: &str = "static/generated/sitemap.xml";
const DEFAULT_RELOAD_URL: &str = "http://127.0.0.1:3000/__admin/reload";
const DEFAULT_OS_INDEX: &str = "rodin-blog";

#[derive(Clone)]
struct OpenSearchCfg {
    endpoint: String,
    index: String,
    username: Option<String>,
    password: Option<String>,
    site_url: String,
    reset: bool,
}

fn main() -> Result<()> {
    let mut skip_markdown = false;
    let mut site_url = DEFAULT_SITE_URL.to_string();
    let mut do_reload = false;
    let mut reload_url: Option<String> = None;
    let mut reload_token: Option<String> = None;
    let mut os_cfg: Option<OpenSearchCfg> = None;

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
            "--opensearch" => {
                os_cfg = Some(OpenSearchCfg {
                    endpoint: std::env::var("OPENSEARCH_ENDPOINT")
                        .unwrap_or_else(|_| "http://127.0.0.1:9200".to_string()),
                    index: std::env::var("OPENSEARCH_INDEX")
                        .unwrap_or_else(|_| DEFAULT_OS_INDEX.to_string()),
                    username: std::env::var("OPENSEARCH_USERNAME").ok(),
                    password: std::env::var("OPENSEARCH_PASSWORD").ok(),
                    site_url: site_url.clone(),
                    reset: false,
                });
            }
            _ if arg.starts_with("--opensearch-endpoint=") => {
                os_cfg
                    .get_or_insert_with(|| OpenSearchCfg {
                        endpoint: "http://127.0.0.1:9200".to_string(),
                        index: DEFAULT_OS_INDEX.to_string(),
                        username: std::env::var("OPENSEARCH_USERNAME").ok(),
                        password: std::env::var("OPENSEARCH_PASSWORD").ok(),
                        site_url: site_url.clone(),
                        reset: false,
                    })
                    .endpoint = arg.trim_start_matches("--opensearch-endpoint=").to_string();
            }
            _ if arg.starts_with("--opensearch-index=") => {
                os_cfg
                    .get_or_insert_with(|| OpenSearchCfg {
                        endpoint: std::env::var("OPENSEARCH_ENDPOINT")
                            .unwrap_or_else(|_| "http://127.0.0.1:9200".to_string()),
                        index: DEFAULT_OS_INDEX.to_string(),
                        username: std::env::var("OPENSEARCH_USERNAME").ok(),
                        password: std::env::var("OPENSEARCH_PASSWORD").ok(),
                        site_url: site_url.clone(),
                        reset: false,
                    })
                    .index = arg.trim_start_matches("--opensearch-index=").to_string();
            }
            "--opensearch-reset" => {
                os_cfg
                    .get_or_insert_with(|| OpenSearchCfg {
                        endpoint: std::env::var("OPENSEARCH_ENDPOINT")
                            .unwrap_or_else(|_| "http://127.0.0.1:9200".to_string()),
                        index: DEFAULT_OS_INDEX.to_string(),
                        username: std::env::var("OPENSEARCH_USERNAME").ok(),
                        password: std::env::var("OPENSEARCH_PASSWORD").ok(),
                        site_url: site_url.clone(),
                        reset: false,
                    })
                    .reset = true;
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

    if let Some(mut cfg) = os_cfg {
        cfg.site_url = site_url.clone();
        push_to_opensearch(&metas, &cfg)?;
    }

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

fn push_to_opensearch(metas: &[frontmatter::FrontMatter], cfg: &OpenSearchCfg) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("rodin-content/1.0")
        .build()?;

    let index_url = format!("{}/{}", cfg.endpoint.trim_end_matches('/'), cfg.index);

    if cfg.reset {
        let _ = client
            .delete(&index_url)
            .basic_auth_opt(cfg.username.as_ref(), cfg.password.as_ref())
            .send();
    }

    // create index if not exists (with Japanese analyzers)
    let create_resp = client
        .put(&index_url)
        .basic_auth_opt(cfg.username.as_ref(), cfg.password.as_ref())
        .json(&json!({
            "settings": {
                "index": { "refresh_interval": "1s" },
                "analysis": {
                    "tokenizer": {
                        "ja_kuromoji": {
                            "type": "kuromoji_tokenizer",
                            "mode": "search",
                            "discard_punctuation": true
                        }
                    },
                    "filter": {
                        "ja_pos": {
                            "type": "kuromoji_part_of_speech",
                            "stoptags": ["助詞-格助詞-一般", "助詞-終助詞"]
                        },
                        "ja_baseform": { "type": "kuromoji_baseform" },
                        "ja_stemmer": { "type": "kuromoji_stemmer", "minimum_length": 4 },
                        "ja_stop": { "type": "stop", "stopwords": "_japanese_" },
                        "ja_reading": { "type": "kuromoji_readingform", "use_romaji": false },
                        "folding": { "type": "icu_folding" }
                    },
                    "char_filter": {
                        "ja_normalize": { "type": "icu_normalizer" }
                    },
                    "analyzer": {
                        "ja_index": {
                            "type": "custom",
                            "tokenizer": "ja_kuromoji",
                            "char_filter": ["ja_normalize"],
                            "filter": [
                                "ja_baseform",
                                "ja_pos",
                                "ja_stop",
                                "lowercase",
                                "ja_stemmer",
                                "ja_reading",
                                "folding"
                            ]
                        },
                        "ja_search": {
                            "type": "custom",
                            "tokenizer": "ja_kuromoji",
                            "char_filter": ["ja_normalize"],
                            "filter": [
                                "ja_baseform",
                                "ja_pos",
                                "ja_stop",
                                "lowercase",
                                "ja_reading",
                                "folding"
                            ]
                        },
                        "ja_keyword": {
                            "type": "custom",
                            "tokenizer": "keyword",
                            "char_filter": ["ja_normalize"],
                            "filter": ["lowercase", "folding"]
                        }
                    }
                }
            },
            "mappings": {
                "properties": {
                    "slug": { "type": "keyword" },
                    "title": {
                        "type": "text",
                        "analyzer": "ja_index",
                        "search_analyzer": "ja_search",
                        "fields": { "raw": { "type": "keyword" } }
                    },
                    "description": { "type": "text", "analyzer": "ja_index", "search_analyzer": "ja_search" },
                    "body": { "type": "text", "analyzer": "ja_index", "search_analyzer": "ja_search" },
                    "tags": {
                        "type": "text",
                        "analyzer": "ja_index",
                        "search_analyzer": "ja_search",
                        "fields": { "raw": { "type": "keyword" } }
                    },
                    "breadcrumbs": { "type": "text", "analyzer": "ja_index", "search_analyzer": "ja_search" },
                    "published_at": { "type": "date", "format": "strict_date_optional_time||yyyy-MM-dd" },
                    "updated_at": { "type": "date", "format": "strict_date_optional_time||yyyy-MM-dd" },
                    "url": { "type": "keyword" }
                }
            }
        }))
        .send()?;
    if !create_resp.status().is_success() && create_resp.status().as_u16() != 400 {
        println!(
            "opensearch: create index returned {} (continuing)",
            create_resp.status()
        );
    }

    let tag_re = Regex::new("<[^>]+>").unwrap();
    let mut bulk = String::new();
    for m in metas {
        let html_path = format!("static/{}", m.html);
        let html = std::fs::read_to_string(&html_path).unwrap_or_default();
        let plain = tag_re
            .replace_all(&html, " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let doc = json!({
            "slug": m.slug,
            "title": m.title.as_deref().unwrap_or(""),
            "description": m.meta.get("description").cloned().unwrap_or_default(),
            "tags": m.tags,
            "breadcrumbs": m.breadcrumbs,
            "published_at": m.published_at,
            "updated_at": m.updated_at,
            "body": plain,
            "url": format!("{}/blog/{}", cfg.site_url.trim_end_matches('/'), m.slug),
        });
        bulk.push_str(&format!(
            "{{\"index\":{{\"_index\":\"{}\",\"_id\":\"{}\"}}}}\n{}\n",
            cfg.index,
            m.slug,
            doc.to_string()
        ));
    }

    let bulk_url = format!("{}/_bulk?refresh=true", index_url);
    let mut req = client
        .post(&bulk_url)
        .header("content-type", "application/x-ndjson")
        .body(bulk);
    if let Some(u) = cfg.username.as_ref() {
        req = req.basic_auth(u, cfg.password.as_ref());
    }
    let resp = req.send()?;
    if !resp.status().is_success() {
        anyhow::bail!("opensearch bulk failed: {}", resp.status());
    }
    println!(
        "opensearch: indexed {} documents into {}",
        metas.len(),
        cfg.index
    );
    Ok(())
}

trait BasicAuthOpt: Sized {
    fn basic_auth_opt(self, user: Option<&String>, pass: Option<&String>) -> Self;
}

impl BasicAuthOpt for reqwest::blocking::RequestBuilder {
    fn basic_auth_opt(self, user: Option<&String>, pass: Option<&String>) -> Self {
        if let Some(u) = user {
            self.basic_auth(u, pass.as_ref())
        } else {
            self
        }
    }
}

fn print_help() {
    println!("Usage: rodin-content [--skip-markdown] [--site=BASE_URL] [--opensearch ...]");
    println!(
        "  builds Typst articles in ./content into static/generated (HTML, index.json, sitemap)"
    );
    println!("  skips font steps; only content generation runs");
    println!("  --skip-markdown : do not run pandoc even if available");
    println!("  --site=URL      : override sitemap base (default {DEFAULT_SITE_URL})");
    println!("  --reload        : call POST {DEFAULT_RELOAD_URL} after build");
    println!("  --reload-url=U  : override reload URL (http:// only)");
    println!("  --reload-token=T: set X-Rodin-Reload-Token header");
    println!("  --opensearch            : push index to OpenSearch (env overrides available)");
    println!("  --opensearch-endpoint=U : e.g. http://127.0.0.1:9200");
    println!("  --opensearch-index=NAME : index name (default {DEFAULT_OS_INDEX})");
    println!("  --opensearch-reset      : delete index before re-creating");
}
