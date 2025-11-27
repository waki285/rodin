use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::{net::SocketAddr, path::PathBuf};
use tokio::fs;

use super::{markdown_enabled, render::inject_runtime_tokens, state::AppState};
use crate::app::render::{render_search_page, SearchHit};

pub async fn index_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_top, &client_ip, &nonce);
    Html(html).into_response()
}

pub async fn blog_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    // Strip any number of trailing ".html" and redirect to canonical
    let mut slug_clean = slug.clone();
    let mut stripped = false;
    while let Some(s) = slug_clean.strip_suffix(".html") {
        slug_clean = s.to_string();
        stripped = true;
        if slug_clean.is_empty() {
            // e.g., "html.html" -> empty; reject
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    if stripped {
        let loc = format!("/blog/{slug_clean}");
        return Redirect::permanent(&loc).into_response();
    }

    // If requested /blog/{slug}.typ, return raw Typst source
    if let Some(stripped) = slug_clean.strip_suffix(".typ") {
        return raw_typ_response(stripped).await;
    }

    // If requested /blog/{slug}.md, return generated Markdown
    if let Some(stripped) = slug_clean.strip_suffix(".md") {
        return markdown_response(&state, stripped);
    }

    let prerendered = match state.blog_pages.get(&slug_clean) {
        Some(p) => p,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(prerendered, &client_ip, &nonce);
    Html(html).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

pub async fn search_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(nonce): Extension<String>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let client_ip = addr.ip().to_string();
    let q_raw = params.q.unwrap_or_default();
    let q = q_raw.trim();
    let mut hits = Vec::new();
    if !q.is_empty() {
        let q_lc = q.to_lowercase();
        for entry in state.search_index.iter() {
            if entry.title_lc.contains(&q_lc) || entry.body_lc.contains(&q_lc) {
                let snippet = build_snippet(&entry.body_plain, &q_lc);
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
    }

    let html = render_search_page(q.to_string(), &hits, &client_ip, &nonce);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        "X-Robots-Tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    res
}

fn build_snippet(body: &str, q_lc: &str) -> String {
    let body_chars: Vec<char> = body.chars().collect();
    let body_lower: Vec<char> = body.to_lowercase().chars().collect();
    let q_chars: Vec<char> = q_lc.chars().collect();

    let hit = find_subsequence(&body_lower, &q_chars);
    let (start, end) = if let Some(pos) = hit {
        let start = pos.saturating_sub(40);
        let end = (pos + q_chars.len() + 120).min(body_chars.len());
        (start, end)
    } else {
        (0, body_chars.len().min(160))
    };

    let mut snippet: String = body_chars[start..end].iter().collect();
    if end < body_chars.len() {
        snippet.push('…');
    }
    snippet
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
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

pub async fn profile_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_profile, &client_ip, &nonce);
    Html(html).into_response()
}

// Serve raw Typst source
pub async fn raw_typ_response(slug: &str) -> Response {
    // reject directory traversal or hidden drafts
    if slug.contains('/') || slug.starts_with('_') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = PathBuf::from("content").join(format!("{slug}.typ"));
    match fs::read_to_string(&path).await {
        Ok(src) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/vnd.typst; charset=utf-8",
            )],
            src,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn markdown_response(state: &AppState, slug: &str) -> Response {
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
        return StatusCode::NOT_FOUND.into_response();
    }
    match state.blog_markdowns.get(slug) {
        Some(md) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/markdown; charset=utf-8",
            )],
            md.as_ref().to_string(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// Middleware: add Referrer-Policy, CSP nonce, and reject overly long paths (path only, no query/fragment)
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
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    res_headers.insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );
    res_headers.insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    res_headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    let csp = format!(
        "default-src 'self'; script-src 'self' 'nonce-{}' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; frame-ancestors 'self'; base-uri 'self'; require-trusted-types-for 'script'",
        nonce
    );
    if let Ok(val) = HeaderValue::from_str(&csp) {
        res_headers.insert("Content-Security-Policy", val);
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
        HeaderValue::from_static("Follow https://x.com/suzuneu_discord please!"),
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
