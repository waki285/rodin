mod handlers;
pub mod render;
mod state;

// Re-export for use in logging
pub use handlers::get_client_ip;

use axum::http::HeaderValue;
use std::{convert::Infallible, env, sync::OnceLock};

use axum::routing::get_service;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tower::service_fn;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::logging;

const RODIN_MARKDOWN_ENABLED: &str = env!("RODIN_MARKDOWN_ENABLED");
const GIT_HASH: &str = env!("GIT_HASH");

pub(crate) fn markdown_enabled() -> bool {
    matches!(
        RODIN_MARKDOWN_ENABLED,
        "true" | "1" | "yes" | "on" | "TRUE" | "True" | "ON"
    )
}

fn env_flag(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|v| {
            matches!(
                v.as_str(),
                "1" | "true" | "TRUE" | "on" | "ON" | "yes" | "YES"
            )
        })
        .unwrap_or(default)
}

pub async fn run() -> anyhow::Result<()> {
    let app_state = state::build_shared_state().await?;

    let compression_enabled = env_flag("COMPRESSION_ENABLED", true);

    let static_root = ServeDir::new("static/root").fallback(service_fn(|_req| async move {
        let res = handlers::not_found_response().await;
        Ok::<_, Infallible>(res)
    }));

    let mut app = Router::new()
        .route("/", get(handlers::index_handler))
        .route("/profile", get(handlers::profile_handler))
        .route("/pgp", get(handlers::pgp_handler))
        .route("/blog/{slug}", get(handlers::blog_handler))
        .route("/search", get(handlers::search_handler))
        .route("/__admin/reload", post(handlers::reload_handler))
        .route_service(
            "/sitemap.xml",
            ServeFile::new("static/generated/sitemap.xml"),
        )
        .nest_service("/assets", ServeDir::new("static"))
        .fallback_service(get_service(static_root))
        .with_state(app_state);

    if compression_enabled {
        app = app.layer(CompressionLayer::new());
    }
    app = app.layer(middleware::from_fn(handlers::security_middleware));
    app = app.layer(middleware::from_fn(cache_headers_middleware));
    app = app.layer(middleware::from_fn(logging::access_log_middleware));

    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let listener = TcpListener::bind(format!("{}:{}", bind, port)).await?;
    tracing::info!("Server running on http://{}:{}", bind, port);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn cache_headers_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> axum::http::Response<axum::body::Body> {
    static CACHE_ENABLED: OnceLock<bool> = OnceLock::new();
    let cache_enabled = *CACHE_ENABLED.get_or_init(|| env_flag("CACHE_ENABLED", false));
    if !cache_enabled {
        return next.run(req).await;
    }

    let path_owned = req.uri().path().to_string();
    let ext_owned = path_owned.rsplit('.').next().map(str::to_string);
    let ext = ext_owned.as_deref();
    let mut res = next.run(req).await;

    let is_asset = path_owned.starts_with("/assets/")
        || ext
            .map(|e| {
                e.eq_ignore_ascii_case("css")
                    || e.eq_ignore_ascii_case("js")
                    || e.eq_ignore_ascii_case("png")
                    || e.eq_ignore_ascii_case("jpg")
                    || e.eq_ignore_ascii_case("jpeg")
                    || e.eq_ignore_ascii_case("webp")
                    || e.eq_ignore_ascii_case("avif")
                    || e.eq_ignore_ascii_case("svg")
                    || e.eq_ignore_ascii_case("ico")
                    || e.eq_ignore_ascii_case("woff")
                    || e.eq_ignore_ascii_case("woff2")
                    || e.eq_ignore_ascii_case("typ")
                    || e.eq_ignore_ascii_case("md")
            })
            .unwrap_or(false);

    // Check if path contains content hash (e.g., app-a1b2c3d4.js)
    let is_hashed_asset = is_asset && is_hashed_filename(&path_owned);
    let is_image = ext
        .map(|e| {
            e.eq_ignore_ascii_case("png")
                || e.eq_ignore_ascii_case("jpg")
                || e.eq_ignore_ascii_case("jpeg")
                || e.eq_ignore_ascii_case("webp")
                || e.eq_ignore_ascii_case("avif")
                || e.eq_ignore_ascii_case("svg")
                || e.eq_ignore_ascii_case("ico")
        })
        .unwrap_or(false);

    if is_asset {
        // ダウンロードされるのを直す
        let need_ct = res
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(|v| v == "application/octet-stream")
            .unwrap_or(true);
        if need_ct {
            if let Some(ext) = ext {
                if let Some(mime) = guess_mime(ext) {
                    if let Ok(val) = HeaderValue::from_str(mime) {
                        res.headers_mut()
                            .insert(axum::http::header::CONTENT_TYPE, val);
                    }
                    // inline で扱えるものは Content-Disposition を明示
                    if mime.starts_with("text/")
                        || mime.starts_with("image/")
                        || mime == "application/javascript"
                        || mime == "application/json"
                        || mime == "application/pgp-keys"
                    {
                        res.headers_mut().insert(
                            axum::http::header::CONTENT_DISPOSITION,
                            HeaderValue::from_static("inline"),
                        );
                    }
                }
            }
        }
        let cc = if is_hashed_asset {
            // Hashed assets: immutable, 1 year
            "public, max-age=31536000, immutable"
        } else if is_image {
            // Images without hash: medium cache (1 day) with revalidation
            "public, max-age=86400, stale-while-revalidate=604800"
        } else {
            // Other assets: short cache with revalidation
            "public, max-age=300, stale-while-revalidate=604800"
        };
        if let Ok(val) = HeaderValue::from_str(cc) {
            res.headers_mut()
                .insert(axum::http::header::CACHE_CONTROL, val);
        }
        // ETag only for non-hashed assets (hashed ones don't need it)
        if !is_hashed_asset {
            if let Ok(val) = HeaderValue::from_str(&format!("W/\"{}\"", GIT_HASH)) {
                res.headers_mut().insert(axum::http::header::ETAG, val);
            }
        }
        res.headers_mut().insert(
            axum::http::header::VARY,
            HeaderValue::from_static("Accept-Encoding, User-Agent"),
        );
    } else {
        let cc = "no-cache, must-revalidate";
        if let Ok(val) = HeaderValue::from_str(cc) {
            res.headers_mut()
                .insert(axum::http::header::CACHE_CONTROL, val);
        }
    }
    res
}

/// Check if filename contains a content hash (e.g., app-a1b2c3d4.js or font.subset-0ae176d131d7.woff2)
fn is_hashed_filename(path: &str) -> bool {
    // Extract filename from path
    let filename = path.rsplit('/').next().unwrap_or(path);
    // Pattern: name-HASH.ext where HASH is 8 or 12 hex chars
    if let Some(dot_pos) = filename.rfind('.') {
        let name_part = &filename[..dot_pos];
        if let Some(dash_pos) = name_part.rfind('-') {
            let potential_hash = &name_part[dash_pos + 1..];
            let len = potential_hash.len();
            return (len == 8 || len == 12)
                && potential_hash.chars().all(|c| c.is_ascii_hexdigit());
        }
    }
    false
}

fn guess_mime(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => Some("text/html; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "js" => Some("application/javascript"),
        "json" => Some("application/json"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        "md" => Some("text/markdown; charset=utf-8"),
        "typ" => Some("text/vnd.typst; charset=utf-8"),
        "txt" => Some("text/plain; charset=utf-8"),
        "pub" => Some("text/plain; charset=utf-8"),
        "asc" => Some("application/pgp-keys"),
        "gpg" => Some("application/pgp-signature"),
        _ => None,
    }
}
