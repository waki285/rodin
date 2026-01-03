use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Json, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    net::SocketAddr,
    sync::{LazyLock, OnceLock},
};

use super::{
    markdown_enabled,
    render::{inject_runtime_tokens, SITE_URL},
    state::{self, AppState, SharedAppState},
};
use crate::app::{
    render::{render_search_page, render_tag_page, BlogListItem, SearchHit},
    state::SearchIndexEntry,
};

const CSP_PREFIX: &str = "default-src 'self'; script-src 'self' 'nonce-";
const CSP_SUFFIX: &str = "' static.cloudflareinsights.com platform.twitter.com js.hcaptcha.com hcaptcha.com newassets.hcaptcha.com 'strict-dynamic'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https://*.hcaptcha.com; font-src 'self'; connect-src 'self' cloudflareinsights.com https://hcaptcha.com https://*.hcaptcha.com https://newassets.hcaptcha.com; object-src 'none'; frame-src https://platform.twitter.com https://syndication.twitter.com https://hcaptcha.com https://newassets.hcaptcha.com; frame-ancestors 'self'; base-uri 'none'; form-action 'self'; trusted-types default rodin-spa rodin-twitter; require-trusted-types-for 'script'";
const RSS_LIMIT: usize = 30;
const DEFAULT_OS_INDEX: &str = "rodin-blog";

pub(crate) static TRUST_PROXY_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    env::var("TRUST_PROXY")
        .map(|v| v == "true")
        .unwrap_or(false)
});
static RELOAD_TOKEN: OnceLock<Option<String>> = OnceLock::new();

fn reload_token() -> Option<&'static str> {
    RELOAD_TOKEN
        .get_or_init(|| env::var("RELOAD_TOKEN").ok())
        .as_deref()
}

/// Extract client IP from headers (with proxy support) or fallback to socket address
pub fn get_client_ip(headers: &HeaderMap, socket_addr: &SocketAddr) -> String {
    client_ip_from_headers(headers).unwrap_or_else(|| socket_addr.ip().to_string())
}

/// AIクローラーのUser-Agentかどうかを判定
const AI_CRAWLER_PATTERNS: &[&str] = &[
    "https://openai.com",
    "Google-Extended",
    "https://perplexity.ai",
    "Claude",
    "CCBot",
    "AMZN-User",
    "Amazonbot",
];

fn is_ai_crawler(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| AI_CRAWLER_PATTERNS.iter().any(|p| ua.contains(p)))
        .unwrap_or(false)
}

#[derive(Clone)]
struct OpenSearchConfig {
    endpoint: String,
    index: String,
    username: Option<String>,
    password: Option<String>,
}

#[inline]
fn load_opensearch_config() -> Option<OpenSearchConfig> {
    let endpoint = env::var("OPENSEARCH_ENDPOINT").ok()?;
    let index = env::var("OPENSEARCH_INDEX").unwrap_or_else(|_| DEFAULT_OS_INDEX.to_string());
    let username = env::var("OPENSEARCH_USERNAME").ok();
    let password = env::var("OPENSEARCH_PASSWORD").ok();
    Some(OpenSearchConfig {
        endpoint,
        index,
        username,
        password,
    })
}

#[inline]
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn to_rfc2822(date: &str) -> Option<String> {
    let iso = format!("{date}T00:00:00+09:00");
    chrono::DateTime::parse_from_rfc3339(&iso)
        .ok()
        .map(|d| d.to_rfc2822())
}

pub async fn reload_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    // トークンが設定されていればヘッダーで検証、無ければループバック限定
    if let Some(token) = reload_token() {
        let ok = headers
            .get("X-Rodin-Reload-Token")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == token)
            .unwrap_or(false);
        if !ok {
            return (StatusCode::UNAUTHORIZED, "reload token required").into_response();
        }
    } else if !addr.ip().is_loopback() {
        return (
            StatusCode::FORBIDDEN,
            "reload is allowed only from loopback without RELOAD_TOKEN",
        )
            .into_response();
    }

    match state::reload_state(&state).await {
        Ok(()) => (StatusCode::OK, "reloaded").into_response(),
        Err(e) => {
            eprintln!("reload failed: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "reload failed").into_response()
        }
    }
}

pub async fn index_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let state = state.read().await;
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_top, &client_ip, &nonce);
    Html(html).into_response()
}

pub async fn blog_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let state = state.read().await;
    let is_curl = is_curl(&headers);
    // Strip any number of trailing ".html" for lookup; redirect only for non-curl
    let mut slug_clean = slug.clone();
    let mut stripped = false;
    while let Some(s) = slug_clean.strip_suffix(".html") {
        slug_clean = s.to_string();
        stripped = true;
        if slug_clean.is_empty() {
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    if stripped && !is_curl {
        let loc = format!("/blog/{slug_clean}");
        return Redirect::permanent(&loc).into_response();
    }

    // curl が /blog/{slug}.typ か /blog/{slug} にリクエストしたら Typst ソースを返す
    if let Some(stripped) = slug_clean.strip_suffix(".typ") {
        return raw_typ_response(&state, stripped, &headers).await;
    }
    if is_curl && !slug.contains('.') {
        return raw_typ_response(&state, &slug_clean, &headers).await;
    }

    // /blog/{slug}.md にリクエストしたら Markdown ソースを返す
    if let Some(stripped) = slug_clean.strip_suffix(".md") {
        return markdown_response(&state, stripped, &headers).await;
    }

    let prerendered = match state.blog_pages.get(&slug_clean) {
        Some(p) => p,
        None => return not_found_response().await,
    };

    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(prerendered, &client_ip, &nonce);
    Html(html).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BlogListQuery {
    page: Option<u32>,
}

const POSTS_PER_PAGE: usize = 10;

pub async fn blog_list_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(nonce): Extension<String>,
    Query(params): Query<BlogListQuery>,
) -> Response {
    let state = state.read().await;
    let client_ip = addr.ip().to_string();
    let page = params.page.unwrap_or(1).max(1) as usize;

    // 投稿を日付でソート（新しい順）
    let mut posts: Vec<_> = state
        .search_index
        .iter()
        .map(|e| crate::app::render::BlogListItem {
            slug: e.slug.clone(),
            title: e.title.clone(),
            published_at: e.published_at.clone(),
            updated_at: e.updated_at.clone(),
            description: e.description.clone(),
            tags: e.tags.clone(),
        })
        .collect();
    posts.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    let total = posts.len();
    let total_pages = total.div_ceil(POSTS_PER_PAGE);
    let start = (page - 1) * POSTS_PER_PAGE;
    let page_posts: Vec<_> = posts.into_iter().skip(start).take(POSTS_PER_PAGE).collect();

    let html = crate::app::render::render_blog_list_page(
        &client_ip,
        &nonce,
        page_posts,
        page as u32,
        total_pages as u32,
    );
    Html(html).into_response()
}

pub async fn search_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(nonce): Extension<String>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let state_guard = state.read().await;
    let client_ip = addr.ip().to_string();
    let q_raw = params.q.unwrap_or_default();
    let q = q_raw.trim();

    // どの検索パスを通ったかを明示するためのラベル
    let mut search_source = "none";

    let hits = if q.is_empty() {
        Vec::new()
    } else if let Some(cfg) = load_opensearch_config() {
        search_source = "opensearch";
        match search_opensearch(&cfg, q).await {
            Ok(res) => res,
            Err(e) => {
                eprintln!("opensearch search failed: {e}");
                search_source = "local-fallback";
                search_local(&state_guard.search_index, q)
            }
        }
    } else {
        search_source = "local";
        search_local(&state_guard.search_index, q)
    };

    let html = render_search_page(q.to_string(), &hits, &client_ip, &nonce);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        "X-Robots-Tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    // 検索がどの経路を使ったかをレスポンスヘッダーで可視化
    if let Ok(val) = HeaderValue::from_str(search_source) {
        res.headers_mut().insert("Rodin-Search-Source", val);
    }
    res
}

pub async fn tag_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(tag_raw): Path<String>,
    Extension(nonce): Extension<String>,
) -> Response {
    let tag = tag_raw.trim();
    if tag.is_empty() {
        return not_found_response().await;
    }

    let state = state.read().await;
    let client_ip = addr.ip().to_string();
    let tag_lc = tag.to_lowercase();

    let mut posts: Vec<BlogListItem> = state
        .search_index
        .iter()
        .filter(|entry| {
            entry
                .tags
                .iter()
                .any(|t| t.eq_ignore_ascii_case(tag) || t.to_lowercase() == tag_lc)
                || entry
                    .breadcrumbs
                    .iter()
                    .any(|b| b.eq_ignore_ascii_case(tag) || b.to_lowercase() == tag_lc)
        })
        .map(|e| BlogListItem {
            slug: e.slug.clone(),
            title: e.title.clone(),
            published_at: e.published_at.clone(),
            updated_at: e.updated_at.clone(),
            description: e.description.clone(),
            tags: e.tags.clone(),
        })
        .collect();

    if posts.is_empty() {
        return not_found_response().await;
    }

    posts.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    let html = render_tag_page(&client_ip, &nonce, tag, posts);
    Html(html).into_response()
}

fn build_snippet(body_chars: &[char], body_lower: &[char], needle: &[char]) -> String {
    let hit = find_subsequence(body_lower, needle);
    if let Some(pos) = hit {
        let start = pos.saturating_sub(40);
        let end = (pos + needle.len() + 120).min(body_chars.len());
        let mut snippet = String::new();
        snippet.extend(body_chars[start..pos].iter());
        snippet.push_str("<mark>");
        snippet.extend(body_chars[pos..pos + needle.len()].iter());
        snippet.push_str("</mark>");
        snippet.extend(body_chars[pos + needle.len()..end].iter());
        if end < body_chars.len() {
            snippet.push('…');
        }
        snippet
    } else {
        let end = body_chars.len().min(160);
        let mut snippet: String = body_chars[..end].iter().collect();
        if end < body_chars.len() {
            snippet.push('…');
        }
        snippet
    }
}

fn search_local(index: &[SearchIndexEntry], q: &str) -> Vec<SearchHit> {
    let q_lc = q.to_lowercase();
    let q_chars: Vec<char> = q_lc.chars().collect();
    let mut hits = Vec::new();
    for entry in index.iter() {
        if entry.title_lc.contains(&q_lc) || entry.body_lc.contains(&q_lc) {
            let snippet = build_snippet(&entry.body_chars, &entry.body_lower, &q_chars);
            hits.push(SearchHit {
                title: entry.title.clone(),
                slug: entry.slug.clone(),
                snippet,
                published_at: entry.published_at.clone(),
                updated_at: entry.updated_at.clone(),
            });
            if hits.len() >= 30 {
                break;
            }
        }
    }
    hits
}

async fn search_opensearch(cfg: &OpenSearchConfig, q: &str) -> Result<Vec<SearchHit>> {
    #[derive(Deserialize)]
    struct HitSource {
        slug: String,
        title: String,
        description: Option<String>,
        published_at: Option<String>,
        updated_at: Option<String>,
    }

    #[derive(Deserialize)]
    struct Hit {
        _source: HitSource,
        #[serde(default)]
        highlight: Value,
    }

    #[derive(Deserialize)]
    struct SearchResponse {
        hits: SearchHits,
    }

    #[derive(Deserialize)]
    struct SearchHits {
        hits: Vec<Hit>,
    }

    let client = Client::builder().user_agent("rodin-search/1.0").build()?;

    let url = format!(
        "{}/{}/_search",
        cfg.endpoint.trim_end_matches('/'),
        cfg.index
    );

    let body = serde_json::json!({
        "size": 20,
        "query": {
            "multi_match": {
                "query": q,
                "fields": ["title^3", "description^2", "body", "tags^2", "breadcrumbs^1.5"]
            }
        },
        "highlight": {
            "fields": {
                "body": { "fragment_size": 120, "number_of_fragments": 1 }
            }
        }
    });

    let mut req = client.post(url).json(&body);
    if let Some(user) = cfg.username.as_ref() {
        req = req.basic_auth(user, cfg.password.as_ref());
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("opensearch status {}", resp.status());
    }
    let parsed: SearchResponse = resp.json().await?;

    let mut results = Vec::new();
    for hit in parsed.hits.hits.into_iter() {
        let src = hit._source;
        let snippet = hit
            .highlight
            .get("body")
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str())
            .map(|s| {
                s.replace("<em>", "<mark>")
                    .replace("</em>", "</mark>")
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace("&lt;mark>", "<mark>")
                    .replace("&lt;/mark>", "</mark>")
            })
            .or(src.description.clone())
            .unwrap_or_else(|| src.title.clone());
        results.push(SearchHit {
            title: src.title,
            slug: src.slug,
            snippet,
            published_at: src.published_at,
            updated_at: src.updated_at,
        });
    }

    Ok(results)
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    if !*TRUST_PROXY_ENABLED {
        return None;
    }
    if let Some(val) = headers.get("CF-Connecting-IP") {
        if let Ok(s) = val.to_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    if let Some(val) = headers.get("X-Forwarded-For") {
        if let Ok(s) = val.to_str() {
            if let Some(first) = s.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

fn is_curl(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| ua.to_lowercase().contains("curl"))
        .unwrap_or(false)
}

pub async fn rss_handler(State(state): State<SharedAppState>) -> Response {
    let state = state.read().await;
    let mut items: Vec<&SearchIndexEntry> = state.search_index.iter().collect();
    items.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    let now = Utc::now().to_rfc2822();

    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str("<rss version=\"2.0\">\n<channel>\n");
    xml.push_str(&format!(
        "<title>{}</title>\n",
        escape_xml("すずねーうのブログ")
    ));
    xml.push_str(&format!("<link>{}</link>\n", SITE_URL));
    xml.push_str(&format!(
        "<description>{}</description>\n",
        escape_xml("すずねーうのブログのRSSフィード")
    ));
    xml.push_str("<language>ja</language>\n");
    xml.push_str(&format!("<lastBuildDate>{}</lastBuildDate>\n", now));
    xml.push_str("<generator>rodin</generator>\n");

    for entry in items.into_iter().take(RSS_LIMIT) {
        let link = format!("{}/blog/{}", SITE_URL, entry.slug);
        let desc = entry.description.as_deref().unwrap_or(entry.title.as_str());
        let pub_date = entry
            .published_at
            .as_deref()
            .and_then(to_rfc2822)
            .or_else(|| entry.updated_at.as_deref().and_then(to_rfc2822));

        xml.push_str("<item>\n");
        xml.push_str(&format!("<title>{}</title>\n", escape_xml(&entry.title)));
        xml.push_str(&format!("<link>{}</link>\n", link));
        xml.push_str(&format!("<guid isPermaLink=\"true\">{}</guid>\n", link));
        xml.push_str(&format!(
            "<description>{}</description>\n",
            escape_xml(desc)
        ));
        if let Some(pd) = pub_date {
            xml.push_str(&format!("<pubDate>{}</pubDate>\n", pd));
        }
        xml.push_str("</item>\n");
    }

    xml.push_str("</channel>\n</rss>");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/rss+xml; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, "inline")
        .body(xml.into())
        .unwrap()
}

pub async fn profile_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let state = state.read().await;
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_profile, &client_ip, &nonce);
    Html(html).into_response()
}

pub async fn pgp_handler(
    State(state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let state = state.read().await;
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_pgp, &client_ip, &nonce);
    Html(html).into_response()
}

pub async fn contact_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let site_key = env::var("HCAPTCHA_SITE_KEY").ok();
    let html = crate::app::render::render_contact_page(&client_ip, &nonce, site_key);
    Html(html).into_response()
}

#[derive(Debug, Deserialize)]
pub(crate) struct ContactFormPayload {
    name: String,
    email: String,
    message: String,
    #[serde(rename = "h-captcha-response")]
    hcaptcha_response: Option<String>,
}

#[derive(Debug, Serialize)]
struct ContactApiResponse {
    ok: bool,
    message: String,
}

pub async fn contact_submit_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<ContactFormPayload>,
) -> Response {
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());

    let name = payload.name.trim();
    let email = payload.email.trim();
    let message = payload.message.trim();

    if name.is_empty() || email.is_empty() || message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ContactApiResponse {
                ok: false,
                message: "必須項目が未入力です。".to_string(),
            }),
        )
            .into_response();
    }
    if name.len() > 100 || email.len() > 200 || message.len() > 5000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ContactApiResponse {
                ok: false,
                message: "入力内容が長すぎます。".to_string(),
            }),
        )
            .into_response();
    }

    let token = match payload.hcaptcha_response.as_deref() {
        Some(t) if !t.trim().is_empty() => t.trim(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ContactApiResponse {
                    ok: false,
                    message: "hCaptcha の認証が必要です。".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Validate hCaptcha
    match verify_hcaptcha(token, &client_ip).await {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ContactApiResponse {
                    ok: false,
                    message: "hCaptcha の検証に失敗しました。".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::warn!("hcaptcha verify error: {e:?}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ContactApiResponse {
                    ok: false,
                    message: "サーバー側で認証に失敗しました。".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Send to Discord
    if let Err(e) = send_discord_webhook(&payload, &client_ip).await {
        tracing::error!("discord webhook error: {e:?}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ContactApiResponse {
                ok: false,
                message: "送信に失敗しました。時間をおいて再度お試しください。".to_string(),
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(ContactApiResponse {
            ok: true,
            message: "送信しました。ありがとうございます。".to_string(),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct HcaptchaVerifyResponse {
    success: bool,
    #[serde(default)]
    #[expect(dead_code)]
    score: Option<f64>,
    #[serde(rename = "error-codes", default)]
    #[expect(dead_code)]
    error_codes: Vec<String>,
}

async fn verify_hcaptcha(token: &str, client_ip: &str) -> Result<bool> {
    let secret =
        env::var("HCAPTCHA_SECRET").map_err(|_| anyhow::anyhow!("HCAPTCHA_SECRET is not set"))?;

    let client = Client::builder().user_agent("rodin-contact/1.0").build()?;

    let resp = client
        .post("https://hcaptcha.com/siteverify")
        .form(&[
            ("secret", secret.as_str()),
            ("response", token),
            ("remoteip", client_ip),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("hcaptcha status {}", resp.status());
    }

    let body: HcaptchaVerifyResponse = resp.json().await?;
    Ok(body.success)
}

async fn send_discord_webhook(payload: &ContactFormPayload, client_ip: &str) -> Result<()> {
    let webhook_url = env::var("DISCORD_WEBHOOK_URL")
        .map_err(|_| anyhow::anyhow!("DISCORD_WEBHOOK_URL is not set"))?;

    let mut message = payload.message.trim().to_string();
    if message.len() > 1500 {
        message.truncate(1500);
        message.push('…');
    }

    let content = format!(
        "**お問い合わせフォーム**\n名前: {}\nメール: {}\nIP: {}\n時刻: {}\n\n{}",
        payload.name.trim(),
        payload.email.trim(),
        client_ip,
        Utc::now().to_rfc3339(),
        message
    );

    let client = Client::builder().user_agent("rodin-contact/1.0").build()?;

    let resp = client
        .post(webhook_url)
        .json(&serde_json::json!({ "content": content }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("discord webhook status {}", resp.status());
    }
    Ok(())
}

pub async fn raw_typ_response(state: &AppState, slug: &str, headers: &HeaderMap) -> Response {
    // 悪意駆動型人生を送っている人を防ぐ
    if slug.contains('/') || slug.starts_with('_') {
        return not_found_response().await;
    }
    match state.blog_typs.get(slug) {
        Some(src) => {
            // AIクローラーの場合は text/plain を返す
            let content_type = if is_ai_crawler(headers) {
                "text/plain; charset=utf-8"
            } else {
                "text/vnd.typst; charset=utf-8"
            };
            (
                [(axum::http::header::CONTENT_TYPE, content_type)],
                src.as_ref().to_string(),
            )
                .into_response()
        }
        None => not_found_response().await,
    }
}

pub async fn markdown_response(state: &AppState, slug: &str, headers: &HeaderMap) -> Response {
    if !markdown_enabled() {
        return (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            )],
            format!(
                "Markdown配信はこのサーバーでは無効です。\nTypstソースが必要なら /blog/{}.typ を参照してください。",
                slug
            ),
        )
            .into_response();
    }
    if slug.contains('/') || slug.starts_with('_') {
        return not_found_response().await;
    }
    match state.blog_markdowns.get(slug) {
        Some(md) => {
            // AIクローラーの場合は text/plain を返す
            let content_type = if is_ai_crawler(headers) {
                "text/plain; charset=utf-8"
            } else {
                "text/markdown; charset=utf-8"
            };
            (
                [(axum::http::header::CONTENT_TYPE, content_type)],
                md.as_ref().to_string(),
            )
                .into_response()
        }
        None => not_found_response().await,
    }
}

pub async fn not_found_response() -> Response {
    let html = r#"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta name="robots" content="noindex,nofollow" />
  <title>404 Not Found</title>
  <style>
    body{margin:0;display:flex;align-items:center;justify-content:center;height:100vh;background:#0f172a;color:#e5e7eb;font-family:system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;}
    .card{padding:24px 28px;border:1px solid #334155;border-radius:14px;background:#111827;box-shadow:0 12px 30px rgba(0,0,0,0.35);text-align:center;max-width:360px;}
    h1{margin:0 0 12px;font-size:20px;}
    p{margin:0;color:#cbd5e1;font-size:14px;}
    a{color:#60a5fa;text-decoration:none;} a:hover{text-decoration:underline;}
  </style>
</head>
<body>
  <div class="card">
    <h1>404 Not Found</h1>
    <p>お探しのページは見つかりませんでした。</p>
    <p><a href="/">ホームに戻る</a></p>
  </div>
</body>
</html>"#;
    (StatusCode::NOT_FOUND, Html(html)).into_response()
}

pub async fn security_middleware(mut req: Request<Body>, next: Next) -> Response {
    let nonce = generate_nonce();
    req.extensions_mut().insert(nonce.clone());

    let path_len = req.uri().path().len();
    if path_len >= 200 {
        return StatusCode::URI_TOO_LONG.into_response();
    }
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;
    let res_headers = res.headers_mut();
    res_headers.insert(
        axum::http::header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    res_headers.insert(
        axum::http::header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );
    res_headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    res_headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    let mut csp = String::with_capacity(CSP_PREFIX.len() + nonce.len() + CSP_SUFFIX.len());
    csp.push_str(CSP_PREFIX);
    csp.push_str(&nonce);
    csp.push_str(CSP_SUFFIX);
    if let Ok(val) = HeaderValue::from_str(&csp) {
        res_headers.insert(axum::http::header::CONTENT_SECURITY_POLICY, val);
    }
    res_headers.insert(
        "X-Permitted-Cross-Domain-Policies",
        HeaderValue::from_static("none"),
    );
    res_headers.insert("Permissions-Policy", HeaderValue::from_static("geolocation=(), microphone=(), camera=(), browsing-topics=(), interest-cohort=(), fullscreen=(), idle-detection=(), local-fonts=(), payment=(), screen-wake-lock=()"));
    res_headers.insert(
        "Cross-Origin-Opener-Policy",
        HeaderValue::from_static("same-origin"),
    );
    res_headers.insert(
        "For-Inspectors",
        HeaderValue::from_str(&format!("Follow {} please!", crate::constants::TWITTER_URL))
            .unwrap(),
    );
    res_headers.insert("For-Scrapers", HeaderValue::from_static("You can use /blog/[slug].typ to get the raw Typst source. Please be kind to the server!"));

    // .typ や .md の場合 noindex
    if path.ends_with(".typ") || path.ends_with(".md") {
        res_headers.insert("X-Robots-Tag", HeaderValue::from_static("noindex,nofollow"));
    }
    res
}

#[inline]
fn generate_nonce() -> String {
    use rand::Rng;
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..16)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}
