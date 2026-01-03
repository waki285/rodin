use crate::frontmatter::FrontMatter;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::json;
use std::{env, time::Duration};

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
            cfg.index,
            m.slug,
            doc
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
