use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use futures::{stream, StreamExt, TryStreamExt};
use regex::Regex;
use tokio::fs;
use tokio::sync::RwLock;

use crate::frontmatter::FrontMatter;

use super::{
    markdown_enabled,
    render::{
        prerender_blog_page, prerender_profile_page, prerender_static_page, prerender_top_page,
    },
};

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("<[^>]+>").expect("valid regex"));

#[derive(Clone)]
pub struct AppState {
    pub(crate) prerender_top: Arc<str>,
    pub(crate) prerender_profile: Arc<str>,
    pub(crate) prerender_pgp: Arc<str>,
    pub(crate) blog_pages: Arc<HashMap<String, Arc<str>>>,
    pub(crate) blog_markdowns: Arc<HashMap<String, Arc<str>>>,
    pub(crate) blog_typs: Arc<HashMap<String, Arc<str>>>,
    pub(crate) search_index: Arc<Vec<SearchIndexEntry>>,
}

pub type SharedAppState = Arc<RwLock<AppState>>;

#[derive(Clone)]
pub struct SearchIndexEntry {
    pub slug: String,
    pub title: String,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub title_lc: String,
    pub body_lc: String,
    pub body_chars: Arc<[char]>,
    pub body_lower: Arc<[char]>,
}

pub async fn build_prerendered_state() -> anyhow::Result<AppState> {
    let base = PathBuf::from("static/generated");
    let meta_path = base.join("index.json");
    let home_path = base.join("home.html");
    let profile_path = base.join("profile.html");
    let profile_meta_path = base.join("profile_meta.json");
    let pgp_path = base.join("pgp.html");
    let pgp_meta_path = base.join("pgp_meta.json");

    let index_bytes = fs::read(&meta_path).await?;
    let metas: Vec<FrontMatter> = serde_json::from_slice(&index_bytes)?;

    let results: Vec<_> = stream::iter(metas.into_iter())
        .map(|meta| async move {
            let slug = meta.slug.clone();
            let html_path = PathBuf::from("static").join(&meta.html);
            let html_content = fs::read_to_string(&html_path).await?;
            let prerendered = Arc::<str>::from(prerender_blog_page(&meta, &html_content));

            let typ_src = {
                let typ_path = PathBuf::from("content").join(format!("{slug}.typ"));
                match fs::read_to_string(&typ_path).await {
                    Ok(src) => Some(Arc::<str>::from(src)),
                    Err(_) => None,
                }
            };

            let markdown = if markdown_enabled() {
                if let Some(md_rel) = meta.markdown.as_ref() {
                    let md_path = PathBuf::from("static").join(md_rel);
                    match fs::read_to_string(&md_path).await {
                        Ok(md_content) => Some(Arc::<str>::from(md_content)),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let plain = html_to_plain(&html_content);
            let body_chars: Arc<[char]> = plain.chars().collect::<Vec<_>>().into();
            let body_lower_str = plain.to_lowercase();
            let body_lower: Arc<[char]> = body_lower_str.chars().collect::<Vec<_>>().into();

            let description = meta
                .meta
                .get("description")
                .or_else(|| meta.meta.get("og:description"))
                .cloned();

            let search_entry = SearchIndexEntry {
                slug: slug.clone(),
                title: meta.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                published_at: meta.published_at.clone(),
                updated_at: meta.updated_at.clone(),
                description,
                tags: meta.tags.clone(),
                title_lc: meta
                    .title
                    .as_deref()
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default(),
                body_lc: body_lower_str,
                body_chars,
                body_lower,
            };

            anyhow::Ok((slug, prerendered, typ_src, markdown, search_entry))
        })
        .buffer_unordered(8)
        .try_collect()
        .await?;

    let mut blog_pages = HashMap::new();
    let mut blog_markdowns = HashMap::new();
    let mut blog_typs = HashMap::new();
    let mut search_entries = Vec::new();
    for (slug, prerendered, typ_src, markdown, search_entry) in results {
        blog_pages.insert(slug.clone(), prerendered);
        if let Some(src) = typ_src {
            blog_typs.insert(slug.clone(), src);
        }
        if let Some(md) = markdown {
            blog_markdowns.insert(slug.clone(), md);
        }
        search_entries.push(search_entry);
    }

    let home_html = fs::read_to_string(&home_path).await.unwrap_or_default();
    let top = Arc::<str>::from(prerender_top_page(&home_html));
    let profile_html = fs::read_to_string(&profile_path).await.unwrap_or_default();
    let profile_meta: FrontMatter = fs::read_to_string(&profile_meta_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| FrontMatter {
            title: Some("プロフィール".to_string()),
            ..Default::default()
        });
    let profile = Arc::<str>::from(prerender_profile_page(&profile_meta, &profile_html));
    let pgp_meta: FrontMatter = fs::read_to_string(&pgp_meta_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| FrontMatter {
            title: Some("PGP 公開鍵".to_string()),
            slug: "pgp".to_string(),
            ..Default::default()
        });
    let pgp_html = fs::read_to_string(&pgp_path).await.unwrap_or_default();
    let pgp = Arc::<str>::from(prerender_static_page(
        &pgp_meta,
        &pgp_html,
        "/pgp",
        "PGP 公開鍵",
    ));

    Ok(AppState {
        prerender_top: top,
        prerender_profile: profile,
        prerender_pgp: pgp,
        blog_pages: Arc::new(blog_pages),
        blog_markdowns: Arc::new(blog_markdowns),
        blog_typs: Arc::new(blog_typs),
        search_index: Arc::new(search_entries),
    })
}

pub async fn build_shared_state() -> anyhow::Result<SharedAppState> {
    let state = build_prerendered_state().await?;
    Ok(Arc::new(RwLock::new(state)))
}

pub async fn reload_state(shared: &SharedAppState) -> anyhow::Result<()> {
    // 先に新しい状態を構築してから書き換えることで、ロック時間を最小化する
    let next = build_prerendered_state().await?;
    let mut guard = shared.write().await;
    *guard = next;
    Ok(())
}

fn html_to_plain(html: &str) -> String {
    let text = TAG_RE.replace_all(html, " ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
