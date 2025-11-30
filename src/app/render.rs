use crate::{
    components::{BlogPage, TopPage},
    frontmatter::FrontMatter,
};
use leptos::prelude::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[cfg(not(debug_assertions))]
use minify_html::{minify, Cfg as HtmlMinCfg};

pub(crate) const CLIENT_IP_TOKEN: &str = "__CLIENT_IP_PLACEHOLDER__";
pub(crate) const CSP_NONCE_TOKEN: &str = "__CSP_NONCE__";
const SITE_URL: &str = "https://suzuneu.com";
const ORG_ID: &str = "https://suzuneu.com/#organization";

#[derive(Clone)]
pub struct SearchHit {
    pub title: String,
    pub slug: String,
    pub snippet: String,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HtmlOptions {
    pub meta: Option<HashMap<String, String>>,
    pub structured_data: Option<Vec<String>>,
    pub head_links: Vec<String>,
    pub head_scripts: Vec<String>,
}

pub(crate) fn wrap_html_with_options(body: &str, title: &str, opts: &HtmlOptions) -> String {
    let meta_tags = opts.meta.as_ref().map(render_meta_tags).unwrap_or_default();
    let structured_json = opts
        .structured_data
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|s| {
                    format!(
                        r#"<script type="application/ld+json" nonce="{CSP_NONCE_TOKEN}">{s}</script>"#
                    )
                })
                .collect::<Vec<_>>()
                .join("\n  ")
        })
        .unwrap_or_default();
    let head_links = if opts.head_links.is_empty() {
        String::new()
    } else {
        opts.head_links.join("\n  ")
    };
    let head_scripts = if opts.head_scripts.is_empty() {
        String::new()
    } else {
        opts.head_scripts.join("\n  ")
    };
    format!(
        r##"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="utf-8" />
  <link rel="preload" href="/assets/build/critical.css" as="style" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  {meta_tags}
  {structured_json}
  <meta name="theme-color" content="#EF4647" />
  <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
  <meta name="apple-mobile-web-app-capable" content="yes" />
  <meta name="mobile-web-app-capable" content="yes">
  <meta name="format-detection" content="telephone=no" />
  <link rel="icon" href="/favicon.ico" />
  <link rel="icon" href="/favicon.svg" type="image/svg+xml" sizes="any" />
  <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
  <link rel="icon" href="/android-chrome-192x192.png" sizes="192x192" />
  <link rel="icon" href="/android-chrome-512x512.png" sizes="512x512" />
  <link rel="stylesheet" href="/assets/build/critical.css" />
  <link rel="stylesheet" href="/assets/build/prose.css" />
  <link rel="stylesheet" href="/assets/build/lazy.css" data-unblock-css="1" media="print" />
  {head_links}
  <script nonce="{CSP_NONCE_TOKEN}">
    const links=[...document.querySelectorAll('link[data-unblock-css=\"1\"]')];
    links.forEach(l=>{{
      const enable=()=>{{l.media='all';}};
      l.addEventListener('load',enable,{{once:true}});
      requestAnimationFrame(()=>{{if(l.sheet) enable();}});
    }});
  </script>
  {head_scripts}
</head>
<body>
{body}
<script type="module" src="/assets/build/app.js" nonce="{CSP_NONCE_TOKEN}"></script>
</body>
</html>"##
    )
}

pub(crate) fn prerender_top_page(home_html: &str) -> String {
    let rendered = Owner::new_root(None).with(|| {
        view! { <TopPage client_ip=CLIENT_IP_TOKEN.to_string() home_html=home_html.to_string() current_path="/".to_string() /> }.to_html()
    });
    let site_structured = build_site_structured_data();
    let homepage_structured = build_homepage_structured_data();
    let opts = HtmlOptions {
        meta: Some(top_meta()),
        structured_data: Some(vec![site_structured, homepage_structured]),
        head_links: vec![r#"<link rel="stylesheet" href="/assets/build/post-card.css" data-unblock-css="1" media="print" />"#.to_string()],
        head_scripts: vec![],
    };
    maybe_minify(wrap_html_with_options(
        &rendered,
        "すずねーうのウェブサイト",
        &opts,
    ))
}

pub(crate) fn prerender_blog_page(meta: &FrontMatter, html_content: &str) -> String {
    let rendered = Owner::new_root(None).with(|| {
        view! {
            <BlogPage
                client_ip=CLIENT_IP_TOKEN.to_string()
                html_content=html_content.to_string()
                meta=meta.clone()
                current_path=format!("/blog/{}", meta.slug)
            />
        }
        .to_html()
    });

    let page_title = meta
        .title
        .as_ref()
        .map(|t| format!("{t}｜すずねーう"))
        .unwrap_or_else(|| "すずねーう".to_string());

    let mut structured_vec = vec![build_site_structured_data()];
    if let Some(a) = build_article_structured_data(meta) {
        structured_vec.push(a);
    }
    if let Some(bc) = build_breadcrumb_structured_data(
        meta,
        &format!("/blog/{}", meta.slug),
        meta.title.as_deref().unwrap_or(meta.slug.as_str()),
    ) {
        structured_vec.push(bc);
    }
    let mut meta_map = meta.meta.clone();
    meta_map
        .entry("link:canonical".to_string())
        .or_insert_with(|| format!("{SITE_URL}/blog/{}", meta.slug));

    let opts = HtmlOptions {
        meta: Some(meta_map),
        structured_data: if structured_vec.is_empty() {
            None
        } else {
            Some(structured_vec)
        },
        ..Default::default()
    };

    maybe_minify(wrap_html_with_options(&rendered, &page_title, &opts))
}

pub(crate) fn prerender_profile_page(meta: &FrontMatter, profile_html: &str) -> String {
    let mut meta_map = meta.meta.clone();
    meta_map
        .entry("link:canonical".to_string())
        .or_insert_with(|| format!("{SITE_URL}/profile"));
    meta_map
        .entry("description".to_string())
        .or_insert_with(|| "すずねーうのプロフィールページ".to_string());
    meta_map
        .entry("og:description".to_string())
        .or_insert_with(|| "すずねーうのプロフィールページ".to_string());
    meta_map
        .entry("og:title".to_string())
        .or_insert_with(|| "プロフィール｜すずねーう".to_string());
    meta_map
        .entry("og:type".to_string())
        .or_insert_with(|| "profile".to_string());

    let mut meta_full = meta.clone();
    meta_full.meta = meta_map.clone();

    let rendered = Owner::new_root(None).with(|| {
        view! {
            <BlogPage
                client_ip=CLIENT_IP_TOKEN.to_string()
                html_content=profile_html.to_string()
                meta=meta_full.clone()
                current_path="/profile".to_string()
            />
        }
        .to_html()
    });

    let mut structured = vec![build_site_structured_data()];
    if let Some(bc) = build_breadcrumb_structured_data(meta, "/profile", "プロフィール") {
        structured.push(bc);
    }
    let opts = HtmlOptions {
        meta: Some(meta_map),
        structured_data: Some(structured),
        ..Default::default()
    };
    maybe_minify(wrap_html_with_options(
        &rendered,
        "プロフィール｜すずねーう",
        &opts,
    ))
}

pub(crate) fn prerender_static_page(
    meta: &FrontMatter,
    body_html: &str,
    path: &str,
    page_title: &str,
) -> String {
    let mut meta_map = meta.meta.clone();
    meta_map
        .entry("link:canonical".to_string())
        .or_insert_with(|| format!("{SITE_URL}{}", path));
    let fallback_desc = meta_map
        .get("description")
        .or_else(|| meta_map.get("og:description"))
        .cloned()
        .unwrap_or_default();
    meta_map
        .entry("description".to_string())
        .or_insert_with(|| fallback_desc.clone());
    meta_map
        .entry("og:description".to_string())
        .or_insert_with(|| fallback_desc.clone());
    meta_map
        .entry("og:title".to_string())
        .or_insert_with(|| page_title.to_string());

    let rendered = Owner::new_root(None).with(|| {
        view! {
            <BlogPage
                client_ip=CLIENT_IP_TOKEN.to_string()
                html_content=body_html.to_string()
                meta={
                    let mut m = meta.clone();
                    m.meta = meta_map.clone();
                    m
                }
                current_path=path.to_string()
            />
        }
        .to_html()
    });

    let mut structured = vec![build_site_structured_data()];
    if let Some(bc) = build_breadcrumb_structured_data(meta, path, page_title) {
        structured.push(bc);
    }
    let opts = HtmlOptions {
        meta: Some(meta_map),
        structured_data: Some(structured),
        ..Default::default()
    };
    maybe_minify(wrap_html_with_options(
        &rendered,
        &format!("{page_title}｜すずねーう"),
        &opts,
    ))
}

pub(crate) fn render_search_page(
    query: String,
    hits: &[SearchHit],
    client_ip: &str,
    nonce: &str,
) -> String {
    let rendered = Owner::new_root(None).with(|| {
        view! {
            <crate::components::SearchPage
                client_ip=client_ip.to_string()
                query=query.clone()
                results=hits.to_vec()
                current_path="/search".to_string()
            />
        }
        .to_html()
    });
    let mut meta = HashMap::new();
    meta.insert("link:canonical".to_string(), format!("{SITE_URL}/search"));
    meta.insert("robots".to_string(), "noindex, nofollow".to_string());
    let opts = HtmlOptions {
        meta: Some(meta),
        head_links: vec![r#"<link rel="stylesheet" href="/assets/build/search.css" />"#.to_string()],
        ..Default::default()
    };
    let html = wrap_html_with_options(&rendered, "検索｜すずねーう", &opts);
    inject_runtime_tokens(&html, client_ip, nonce)
}

pub(crate) fn inject_runtime_tokens(template: &str, client_ip: &str, nonce: &str) -> String {
    template
        .replace(CLIENT_IP_TOKEN, client_ip)
        .replace(CSP_NONCE_TOKEN, nonce)
}

fn render_meta_tags(meta: &HashMap<String, String>) -> String {
    meta.iter()
        .map(|(k, v)| {
            if let Some(rel) = k.strip_prefix("link:") {
                return format!(r#"<link rel="{rel}" href="{v}" />"#);
            }

            // Open Graph / Facebook / Article は property
            let attr = if k.starts_with("og:") || k.starts_with("fb:") || k.starts_with("article:")
            {
                "property"
            } else {
                "name"
            };
            format!(r#"<meta {attr}="{k}" content="{v}" />"#)
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn top_meta() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(
        "description".to_string(),
        "すずねーうのウェブサイト。Rust/Leptos/Typstで作ったブログとポートフォリオ".to_string(),
    );
    m.insert("og:description".to_string(), m["description"].clone());
    m.insert(
        "og:title".to_string(),
        "すずねーうのウェブサイト".to_string(),
    );
    m.insert("og:site_name".to_string(), "すずねーう".to_string());
    m.insert("og:type".to_string(), "website".to_string());
    m.insert("og:locale".to_string(), "ja_JP".to_string());
    m.insert("link:canonical".to_string(), SITE_URL.to_string());
    m
}

fn build_site_structured_data() -> String {
    json!({
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": "Organization",
                "@id": ORG_ID,
                "name": "すずねーう",
                "url": SITE_URL,
                "logo": {
                    "@type": "ImageObject",
                    "@id": format!("{SITE_URL}/#logo"),
                    "url": absolute_url("/android-chrome-192x192.png")
                }
            },
            {
                "@type": "WebSite",
                "@id": format!("{SITE_URL}/#website"),
                "url": SITE_URL,
                "name": "すずねーう",
                "inLanguage": "ja",
                "publisher": { "@id": ORG_ID },
                "potentialAction": {
                    "@type": "SearchAction",
                    "target": format!("{SITE_URL}/search?q={{query}}"),
                    "query-input": "required name=query"
                }
            }
        ]
    })
    .to_string()
}

fn build_homepage_structured_data() -> String {
    json!({
        "@context": "https://schema.org",
        "@type": "WebPage",
        "@id": format!("{SITE_URL}/#webpage"),
        "url": SITE_URL,
        "name": "すずねーう",
        "description": "すずねーうのウェブサイト。Rust/Leptos/Typstで作ったブログとポートフォリオ",
        "inLanguage": "ja",
        "isPartOf": { "@id": ORG_ID },
        "primaryImageOfPage": format!("{SITE_URL}/android-chrome-192x192.png")
    })
    .to_string()
}

fn build_breadcrumb_structured_data(
    meta: &FrontMatter,
    path: &str,
    page_title: &str,
) -> Option<String> {
    if meta.breadcrumbs.is_empty() {
        return None;
    }
    let registry = breadcrumb_registry();
    let mut items = Vec::new();
    let mut pos = 1;
    for (idx, key) in meta.breadcrumbs.iter().enumerate() {
        let is_last = idx == meta.breadcrumbs.len() - 1;
        if is_last {
            // Last: always use current title.
            let name = page_title.to_string();
            items.push(json!({
                "@type": "ListItem",
                "position": pos,
                "name": name,
                "item": absolute_url(path),
            }));
        } else if let Some((name, url)) = registry.get(key.as_str()) {
            items.push(json!({
                "@type": "ListItem",
                "position": pos,
                "name": name,
                "item": absolute_url(url),
            }));
        }
        pos += 1;
    }

    Some(
        json!({
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            "itemListElement": items
        })
        .to_string(),
    )
}

pub(crate) fn breadcrumb_registry(
) -> std::collections::HashMap<&'static str, (&'static str, &'static str)> {
    let mut m = std::collections::HashMap::new();
    m.insert("home", ("ホーム", "/"));
    m.insert("profile", ("プロフィール", "/profile"));
    m.insert("blog", ("ブログ", "/blog"));
    m.insert("rust", ("Rust", "/tags/rust"));
    m
}

fn build_article_structured_data(meta: &FrontMatter) -> Option<String> {
    // Headline is the most important field; bail if we can't infer it.
    let headline = meta
        .title
        .as_deref()
        .unwrap_or(meta.slug.as_str())
        .to_string();

    let description = meta
        .meta
        .get("og:description")
        .or_else(|| meta.meta.get("description"))
        .cloned();

    // Prefer explicit OG image; otherwise fall back to site avatar.
    let mut images = Vec::new();
    if let Some(img) = meta.meta.get("og:image") {
        images.push(absolute_url(img));
    }
    if images.is_empty() {
        images.push(absolute_url("/static/images/suzuneu.webp"));
    }

    let author_name = meta
        .meta
        .get("author")
        .cloned()
        .unwrap_or_else(|| "すずねーう".to_string());
    let author_url = meta.meta.get("link:author").cloned();

    let published = meta
        .published_at
        .as_deref()
        .map(normalize_iso8601)
        .or_else(|| meta.meta.get("article:published_time").cloned());
    let modified = meta
        .updated_at
        .as_deref()
        .map(normalize_iso8601)
        .or_else(|| published.clone());

    let mut obj = Map::new();
    obj.insert("@context".into(), json!("https://schema.org"));
    obj.insert("@type".into(), json!("BlogPosting"));
    obj.insert("headline".into(), json!(headline));
    obj.insert(
        "mainEntityOfPage".into(),
        json!({
            "@type": "WebPage",
            "@id": absolute_url(&format!("/blog/{}", meta.slug))
        }),
    );
    obj.insert(
        "url".into(),
        json!(absolute_url(&format!("/blog/{}", meta.slug))),
    );
    obj.insert("inLanguage".into(), json!("ja"));
    obj.insert("image".into(), json!(images));

    if let Some(desc) = description {
        obj.insert("description".into(), json!(desc));
    }
    if let Some(pubd) = published {
        obj.insert("datePublished".into(), json!(pubd));
    }
    if let Some(modd) = modified {
        obj.insert("dateModified".into(), json!(modd));
    }

    let mut author_obj = Map::new();
    author_obj.insert("@type".into(), json!("Person"));
    author_obj.insert("name".into(), json!(author_name));
    if let Some(url) = author_url {
        author_obj.insert("url".into(), json!(url));
    }
    obj.insert("author".into(), Value::Object(author_obj));

    obj.insert("publisher".into(), json!({ "@id": ORG_ID }));

    if !meta.tags.is_empty() {
        obj.insert("keywords".into(), json!(meta.tags));
    }
    if let Some(section) = meta.subtitle.as_ref() {
        if !section.is_empty() {
            obj.insert("articleSection".into(), json!(section));
        }
    }

    Some(Value::Object(obj).to_string())
}

fn normalize_iso8601(date: &str) -> String {
    if date.contains('T') {
        date.to_string()
    } else {
        format!("{date}T00:00:00.000Z")
    }
}

fn absolute_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with('/') {
        format!("{SITE_URL}{url}")
    } else {
        format!("{SITE_URL}/{}", url.trim_start_matches('/'))
    }
}

#[cfg(not(debug_assertions))]
fn maybe_minify(html: String) -> String {
    let cfg = HtmlMinCfg {
        minify_js: true,
        minify_css: false,
        ..Default::default()
    };
    let min = minify(html.as_bytes(), &cfg);
    String::from_utf8(min).unwrap_or(html)
}

#[cfg(debug_assertions)]
fn maybe_minify(html: String) -> String {
    html
}
