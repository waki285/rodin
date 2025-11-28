use axum::{
    body::Body,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::{env, net::SocketAddr};

use super::{markdown_enabled, render::inject_runtime_tokens, state::AppState};
use crate::app::render::{render_search_page, SearchHit};

const CSP_PREFIX: &str = "default-src 'self'; script-src 'self' 'nonce-";
const CSP_SUFFIX: &str = "' static.cloudflareinsights.com 'strict-dynamic'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' cloudflareinsights.com; object-src 'none'; frame-ancestors 'self'; base-uri 'self'; require-trusted-types-for 'script'";

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
        return raw_typ_response(&state, stripped).await;
    }
    if is_curl && !slug.contains('.') {
        return raw_typ_response(&state, &slug_clean).await;
    }

    // /blog/{slug}.md にリクエストしたら Markdown ソースを返す
    if let Some(stripped) = slug_clean.strip_suffix(".md") {
        return markdown_response(&state, stripped).await;
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
    if env::var("TRUST_PROXY").is_err() || env::var("TRUST_PROXY").unwrap() != "true" {
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

pub async fn pgp_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let client_ip = client_ip_from_headers(&headers).unwrap_or_else(|| addr.ip().to_string());
    let html = inject_runtime_tokens(&state.prerender_pgp, &client_ip, &nonce);
    Html(html).into_response()
}

pub async fn raw_typ_response(state: &AppState, slug: &str) -> Response {
    // 悪意駆動型人生を送っている人を防ぐ
    if slug.contains('/') || slug.starts_with('_') {
        return not_found_response().await;
    }
    match state.blog_typs.get(slug) {
        Some(src) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/vnd.typst; charset=utf-8",
            )],
            src.as_ref().to_string(),
        )
            .into_response(),
        None => not_found_response().await,
    }
}

pub async fn markdown_response(state: &AppState, slug: &str) -> Response {
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
        Some(md) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/markdown; charset=utf-8",
            )],
            md.as_ref().to_string(),
        )
            .into_response(),
        None => not_found_response().await,
    }
}

pub async fn not_found_response() -> Response {
    let html = r#"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
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
