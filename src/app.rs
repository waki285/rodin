mod handlers;
pub mod render;
mod state;

use std::env;

use axum::{middleware, routing::get, Router};
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

const RODIN_MARKDOWN_ENABLED: &str = env!("RODIN_MARKDOWN_ENABLED");

pub(crate) fn markdown_enabled() -> bool {
    matches!(
        RODIN_MARKDOWN_ENABLED,
        "true" | "1" | "yes" | "on" | "TRUE" | "True" | "ON"
    )
}

pub async fn run() -> anyhow::Result<()> {
    let app_state = state::build_prerendered_state().await?;

    let app = Router::new()
        .route("/", get(handlers::index_handler))
        .route("/profile", get(handlers::profile_handler))
        .route("/blog/{slug}", get(handlers::blog_handler))
        .route("/search", get(handlers::search_handler))
        .route_service("/favicon.ico", ServeFile::new("static/favicon.ico"))
        .route_service("/favicon.svg", ServeFile::new("static/favicon.svg"))
        .route_service(
            "/apple-touch-icon.png",
            ServeFile::new("static/apple-touch-icon.png"),
        )
        .route_service(
            "/android-chrome-192x192.png",
            ServeFile::new("static/android-chrome-192x192.png"),
        )
        .route_service(
            "/android-chrome-512x512.png",
            ServeFile::new("static/android-chrome-512x512.png"),
        )
        .route_service("/robots.txt", ServeFile::new("static/robots.txt"))
        .route_service(
            "/sitemap.xml",
            ServeFile::new("static/generated/sitemap.xml"),
        )
        .nest_service("/assets", ServeDir::new("static"))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(handlers::security_middleware))
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
