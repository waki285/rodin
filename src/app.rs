mod handlers;
pub mod render;
mod state;

use axum::http::HeaderValue;
use std::{convert::Infallible, env};

use axum::routing::get_service;
use axum::{middleware, routing::get, Router};
use tokio::net::TcpListener;
use tower::service_fn;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

const RODIN_MARKDOWN_ENABLED: &str = env!("RODIN_MARKDOWN_ENABLED");
const GIT_HASH: &str = env!("GIT_HASH");

pub(crate) fn markdown_enabled() -> bool {
    matches!(
        RODIN_MARKDOWN_ENABLED,
        "true" | "1" | "yes" | "on" | "TRUE" | "True" | "ON"
    )
}

pub async fn run() -> anyhow::Result<()> {
    let app_state = state::build_prerendered_state().await?;

    let static_root = ServeDir::new("static/root").fallback(service_fn(|_req| async move {
        let res = handlers::not_found_response().await;
        Ok::<_, Infallible>(res)
    }));

    let app = Router::new()
        .route("/", get(handlers::index_handler))
        .route("/profile", get(handlers::profile_handler))
        .route("/pgp", get(handlers::pgp_handler))
        .route("/blog/{slug}", get(handlers::blog_handler))
        .route("/search", get(handlers::search_handler))
        .route_service(
            "/sitemap.xml",
            ServeFile::new("static/generated/sitemap.xml"),
        )
        .nest_service("/assets", ServeDir::new("static"))
        .fallback_service(get_service(static_root))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(handlers::security_middleware))
        .layer(middleware::from_fn(cache_headers_middleware))
        .with_state(app_state);

    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let listener = TcpListener::bind(format!("{}:{}", bind, port)).await?;
    println!("Server running on http://{}:{}", bind, port);

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
    let cache_enabled = env::var("CACHE_ENABLED")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false);
    let path = req.uri().path().to_ascii_lowercase();
    let mut res = next.run(req).await;
    if !cache_enabled {
        return res;
    }

    let is_asset = path.starts_with("/assets/")
        || path.ends_with(".css")
        || path.ends_with(".js")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".webp")
        || path.ends_with(".avif")
        || path.ends_with(".svg")
        || path.ends_with(".ico")
        || path.ends_with(".woff")
        || path.ends_with(".woff2")
        || path.ends_with(".typ")
        || path.ends_with(".md");

    if is_asset {
        // fix missing/incorrect content-type (e.g., root-served files)
        let need_ct = res
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(|v| v == "application/octet-stream")
            .unwrap_or(true);
        if need_ct {
            if let Some(ext) = path.rsplit('.').next() {
                if let Some(mime) = guess_mime(ext) {
                    if let Ok(val) = HeaderValue::from_str(mime) {
                        res.headers_mut()
                            .insert(axum::http::header::CONTENT_TYPE, val);
                    }
                    // inlineで扱えるものは Content-Disposition を明示
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
        let cc = "public, max-age=300, stale-while-revalidate=604800";
        if let Ok(val) = HeaderValue::from_str(cc) {
            res.headers_mut()
                .insert(axum::http::header::CACHE_CONTROL, val);
        }
        if let Ok(val) = HeaderValue::from_str(&format!("W/\"{}\"", GIT_HASH)) {
            res.headers_mut().insert(axum::http::header::ETAG, val);
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
        "typ" => Some("text/plain; charset=utf-8"),
        "txt" => Some("text/plain; charset=utf-8"),
        "pub" => Some("text/plain; charset=utf-8"),
        "asc" => Some("application/pgp-keys"),
        "gpg" => Some("application/pgp-signature"),
        _ => None,
    }
}
