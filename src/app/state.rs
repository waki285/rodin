use std::{collections::HashMap, path::PathBuf, sync::Arc};

use regex::Regex;
use tokio::fs;

use crate::frontmatter::FrontMatter;

use super::{
    markdown_enabled,
    render::{prerender_blog_page, prerender_profile_page, prerender_top_page},
};

#[derive(Clone)]
pub struct AppState {
    pub(crate) prerender_top: Arc<str>,
    pub(crate) prerender_profile: Arc<str>,
    pub(crate) blog_pages: Arc<HashMap<String, Arc<str>>>,
    pub(crate) blog_markdowns: Arc<HashMap<String, Arc<str>>>,
    pub(crate) search_index: Arc<Vec<SearchIndexEntry>>,
}

#[derive(Clone)]
pub struct SearchIndexEntry {
    pub slug: String,
    pub title: String,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub body_plain: String,
    pub title_lc: String,
    pub body_lc: String,
}

pub async fn build_prerendered_state() -> anyhow::Result<AppState> {
    let base = PathBuf::from("static/generated");
    let meta_path = base.join("index.json");
    let home_path = base.join("home.html");
    let profile_path = base.join("profile.html");
    let profile_meta_path = base.join("profile_meta.json");

    let index_bytes = fs::read(&meta_path).await?;
    let metas: Vec<FrontMatter> = serde_json::from_slice(&index_bytes)?;

    let mut blog_pages = HashMap::new();
    let mut blog_markdowns = HashMap::new();
    let mut search_entries = Vec::new();
    for meta in metas {
        let slug = meta.slug.clone();
        let html_path = PathBuf::from("static").join(&meta.html);
        let html_content = fs::read_to_string(&html_path).await?;
        let prerendered = Arc::<str>::from(prerender_blog_page(&meta, &html_content));
        blog_pages.insert(slug.clone(), prerendered);

        if markdown_enabled() {
            if let Some(md_rel) = meta.markdown.as_ref() {
                let md_path = PathBuf::from("static").join(md_rel);
                let md_content = fs::read_to_string(&md_path).await?;
                blog_markdowns.insert(meta.slug.clone(), Arc::<str>::from(md_content));
            }
        }

        let plain = html_to_plain(&html_content);
        search_entries.push(SearchIndexEntry {
            slug: slug.clone(),
            title: meta.title.clone().unwrap_or_else(|| "Untitled".to_string()),
            published_at: meta.published_at.clone(),
            updated_at: meta.updated_at.clone(),
            title_lc: meta
                .title
                .as_deref()
                .map(|s| s.to_lowercase())
                .unwrap_or_default(),
            body_plain: plain.clone(),
            body_lc: plain.to_lowercase(),
        });
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

    Ok(AppState {
        prerender_top: top,
        prerender_profile: profile,
        blog_pages: Arc::new(blog_pages),
        blog_markdowns: Arc::new(blog_markdowns),
        search_index: Arc::new(search_entries),
    })
}

fn html_to_plain(html: &str) -> String {
    let tag_re = Regex::new("<[^>]+>").expect("valid regex");
    let text = tag_re.replace_all(html, " ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
