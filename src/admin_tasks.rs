use crate::fonts;
use crate::frontmatter::FrontMatter;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::json;
use std::{env, process::Command, time::Duration};

#[path = "../build/markdown.rs"]
mod markdown;
#[path = "../build/posts.rs"]
mod posts;
#[path = "../build/sitemap.rs"]
mod sitemap;

const PREAMBLE_PATH: &str = "content/_preamble.typ";
const GENERATED_DIR: &str = "static/generated";
const GENERATED_MD_DIR: &str = "static/generated/md";
const PANDOC_FILTER: &str = "scripts/pandoc/html-to-md.lua";
const DEFAULT_SITE_URL: &str = crate::constants::SITE_URL;
const DEFAULT_SITEMAP_PATH: &str = "static/generated/sitemap.xml";
const DEFAULT_OS_INDEX: &str = "rodin-blog";

pub fn run_build_and_index(
    opensearch: bool,
    reset_os: bool,
    skip_markdown: bool,
) -> anyhow::Result<String> {
    let mut log = String::new();
    log.push_str("generating content...\n");

    let mut metas = posts::build_posts(PREAMBLE_PATH, GENERATED_DIR)?;
    log.push_str(&format!("generated {} posts\n", metas.len()));

    if skip_markdown {
        for m in metas.iter_mut() {
            m.markdown = None;
        }
        log.push_str("markdown skipped\n");
    } else {
        match markdown::build_markdown(&mut metas, GENERATED_MD_DIR, PANDOC_FILTER) {
            Ok(true) => log.push_str("markdown generated\n"),
            Ok(false) => {
                for m in metas.iter_mut() {
                    m.markdown = None;
                }
                log.push_str("markdown skipped (pandoc missing)\n");
            }
            Err(e) => {
                for m in metas.iter_mut() {
                    m.markdown = None;
                }
                log.push_str(&format!("markdown error: {e}\n"));
            }
        }
    }

    markdown::write_index(&metas, GENERATED_DIR)?;
    posts::build_home(PREAMBLE_PATH, GENERATED_DIR)?;
    posts::build_profile(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_meta = posts::build_pgp(PREAMBLE_PATH, GENERATED_DIR)?;
    let pgp_ref = pgp_meta.as_ref();
    let site_url = env::var("SITE_URL").unwrap_or_else(|_| DEFAULT_SITE_URL.to_string());
    sitemap::write_sitemap(&metas, pgp_ref, &site_url, DEFAULT_SITEMAP_PATH)?;

    if opensearch {
        let cfg = OpenSearchCfg::load(site_url.clone(), reset_os);
        if let Some(cfg) = cfg {
            push_to_opensearch(&metas, &cfg)?;
            log.push_str(&format!("opensearch: indexed {} docs\n", metas.len()));
        } else {
            log.push_str("opensearch: endpoint not configured, skipped\n");
        }
    }

    Ok(log)
}

#[derive(Clone)]
struct OpenSearchCfg {
    endpoint: String,
    index: String,
    username: Option<String>,
    password: Option<String>,
    site_url: String,
    reset: bool,
}

impl OpenSearchCfg {
    fn load(site_url: String, reset: bool) -> Option<Self> {
        let endpoint = env::var("OPENSEARCH_ENDPOINT").ok()?;
        let index = env::var("OPENSEARCH_INDEX").unwrap_or_else(|_| DEFAULT_OS_INDEX.to_string());
        let username = env::var("OPENSEARCH_USERNAME").ok();
        let password = env::var("OPENSEARCH_PASSWORD").ok();
        Some(OpenSearchCfg {
            endpoint,
            index,
            username,
            password,
            site_url,
            reset,
        })
    }
}

fn push_to_opensearch(metas: &[FrontMatter], cfg: &OpenSearchCfg) -> anyhow::Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("rodin-admin/1.0")
        .build()?;

    let index_url = format!("{}/{}", cfg.endpoint.trim_end_matches('/'), cfg.index);

    if cfg.reset {
        let _ = client
            .delete(&index_url)
            .basic_auth_opt(cfg.username.as_ref(), cfg.password.as_ref())
            .send();
    }

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
        anyhow::bail!("opensearch create index failed: {}", create_resp.status());
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
            cfg.index, m.slug, doc
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

/// Git pull for content directory
///
/// Uses `git pull` for cloned repos (production) or `git submodule update --remote` for submodules (development)
pub fn run_git_pull() -> anyhow::Result<String> {
    let mut log = String::new();

    // Check if content is a submodule by looking for .git file (not directory)
    let content_git = std::path::Path::new("content/.git");
    let is_submodule = content_git.exists() && content_git.is_file();

    if is_submodule {
        log.push_str("content is a submodule, running git submodule update --remote...\n");
        let output = Command::new("git")
            .args(["submodule", "update", "--remote", "content"])
            .output()?;
        log.push_str(&String::from_utf8_lossy(&output.stdout));
        log.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            anyhow::bail!(
                "git submodule update failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        log.push_str("content is a cloned repo, running git pull...\n");
        let output = Command::new("git")
            .args(["-C", "content", "pull", "--ff-only"])
            .output()?;
        log.push_str(&String::from_utf8_lossy(&output.stdout));
        log.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            anyhow::bail!(
                "git pull failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    log.push_str("git pull completed\n");
    Ok(log)
}

/// Run font subsetting
/// Collects glyphs from content and source files, then subsets fonts
pub fn run_font_subset() -> anyhow::Result<String> {
    use std::{fs, path::Path};

    let mut log = String::new();
    log.push_str("running font subset...\n");

    #[cfg(not(windows))]
    {
        // Create output directory
        if !Path::new("static/build").exists() {
            fs::create_dir_all("static/build")?;
        }

        // Use existing font subsetting logic from build
        let glyphs = fonts::collect_glyphs()?;
        let bold_glyphs = fonts::collect_bold_glyphs()?;

        log.push_str(&format!(
            "collected {} glyphs for regular/semibold fonts\n",
            glyphs.len()
        ));
        log.push_str(&format!(
            "collected {} glyphs for bold font\n",
            bold_glyphs.len()
        ));

        // Subset fonts
        log.push_str(&format!("subsetting {}...\n", fonts::SEMIBOLD_FONT_SRC));
        fonts::subset_font(
            fonts::SEMIBOLD_FONT_SRC,
            fonts::SEMIBOLD_FONT_TTF_OUT,
            fonts::SEMIBOLD_FONT_WOFF2_OUT,
            &glyphs,
        )?;

        log.push_str(&format!("subsetting {}...\n", fonts::REGULAR_FONT_SRC));
        fonts::subset_font(
            fonts::REGULAR_FONT_SRC,
            fonts::REGULAR_FONT_TTF_OUT,
            fonts::REGULAR_FONT_WOFF2_OUT,
            &glyphs,
        )?;

        log.push_str(&format!("subsetting {}...\n", fonts::BOLD_FONT_SRC));
        fonts::subset_font(
            fonts::BOLD_FONT_SRC,
            fonts::BOLD_FONT_TTF_OUT,
            fonts::BOLD_FONT_WOFF2_OUT,
            &bold_glyphs,
        )?;

        log.push_str("font subset completed\n");
    }

    #[cfg(windows)]
    {
        log.push_str("font subsetting is not available on Windows (hb-subset not supported)\n");
        log.push_str("fonts will be copied without subsetting\n");

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
            log.push_str(&format!("copied {src} to {ttf_dst}\n"));
            log.push_str(&format!("created {woff2_dst}\n"));
        }
    }

    Ok(log)
}

/// Purge Cloudflare cache for site URLs
/// Requires CLOUDFLARE_ZONE_ID and CLOUDFLARE_API_TOKEN environment variables
pub fn purge_cloudflare_cache() -> anyhow::Result<String> {
    let mut log = String::new();

    let zone_id = env::var("CLOUDFLARE_ZONE_ID")
        .map_err(|_| anyhow::anyhow!("CLOUDFLARE_ZONE_ID environment variable not set"))?;
    let api_token = env::var("CLOUDFLARE_API_TOKEN")
        .map_err(|_| anyhow::anyhow!("CLOUDFLARE_API_TOKEN environment variable not set"))?;

    log.push_str("purging Cloudflare cache...\n");

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("rodin-admin/1.0")
        .build()?;

    // Purge by prefix (SITE_URL)
    let site_url = crate::constants::SITE_URL;
    log.push_str(&format!("purging prefix: {site_url}\n"));

    let resp = client
        .post(format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache"
        ))
        .header("Authorization", format!("Bearer {api_token}"))
        .header("Content-Type", "application/json")
        .json(&json!({
            "prefixes": [site_url]
        }))
        .send()?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().unwrap_or_default();

    if !status.is_success() {
        let errors = body
            .get("errors")
            .and_then(|e| serde_json::to_string(e).ok())
            .unwrap_or_else(|| "unknown error".to_string());
        anyhow::bail!("Cloudflare API error ({}): {}", status, errors);
    }

    let success = body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        let errors = body
            .get("errors")
            .and_then(|e| serde_json::to_string(e).ok())
            .unwrap_or_else(|| "unknown error".to_string());
        anyhow::bail!("Cloudflare purge failed: {}", errors);
    }

    log.push_str("Cloudflare cache purged successfully\n");
    Ok(log)
}
