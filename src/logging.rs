use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, Response},
    middleware::Next,
};
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

use crate::app::get_client_ip;

/// Environment: dev or prod
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Dev,
    Prod,
}

impl Environment {
    pub fn from_env() -> Self {
        match env::var("RODIN_ENV")
            .or_else(|_| env::var("RUST_ENV"))
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "prod" | "production" => Self::Prod,
            _ => Self::Dev,
        }
    }
}

/// Log file path
fn log_file_path() -> PathBuf {
    env::var("LOG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs/access.log"))
}

/// Buffered file writer for production (batched writes)
struct BufferedFileWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
    tx: mpsc::UnboundedSender<()>,
}

impl BufferedFileWriter {
    fn new(path: PathBuf, flush_interval: Duration) -> io::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let buffer = Arc::new(Mutex::new(Vec::with_capacity(8192)));
        let (tx, mut rx) = mpsc::unbounded_channel::<()>();

        let buffer_clone = Arc::clone(&buffer);
        let path_clone = path.clone();

        // Spawn background task for periodic flushing
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(flush_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::flush_to_file(&buffer_clone, &path_clone);
                    }
                    result = rx.recv() => {
                        if result.is_none() {
                            // Channel closed, do final flush
                            Self::flush_to_file(&buffer_clone, &path_clone);
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self { buffer, tx })
    }

    fn flush_to_file(buffer: &Arc<Mutex<Vec<u8>>>, path: &PathBuf) {
        let data = {
            let mut buf = buffer.lock().unwrap();
            if buf.is_empty() {
                return;
            }
            std::mem::take(&mut *buf)
        };

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(&data);
        }
    }
}

impl Write for BufferedFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Notify the background task to consider flushing
        let _ = self.tx.send(());
        Ok(())
    }
}

impl Clone for BufferedFileWriter {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            tx: self.tx.clone(),
        }
    }
}

/// Immediate file writer for development
#[derive(Clone)]
struct ImmediateFileWriter {
    path: PathBuf,
}

impl ImmediateFileWriter {
    fn new(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }
}

impl Write for ImmediateFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// MakeWriter wrapper for our file writers
#[derive(Clone)]
enum FileWriter {
    Immediate(ImmediateFileWriter),
    Buffered(BufferedFileWriter),
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            FileWriter::Immediate(w) => w.write(buf),
            FileWriter::Buffered(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            FileWriter::Immediate(w) => w.flush(),
            FileWriter::Buffered(w) => w.flush(),
        }
    }
}

impl<'a> MakeWriter<'a> for FileWriter {
    type Writer = FileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Initialize logging based on environment
pub fn init() -> anyhow::Result<()> {
    let env = Environment::from_env();
    let log_path = log_file_path();

    match env {
        Environment::Dev => init_dev(log_path),
        Environment::Prod => init_prod(log_path),
    }
}

/// Dev: Console (all logs) + File (immediate write)
fn init_dev(log_path: PathBuf) -> anyhow::Result<()> {
    let file_writer = FileWriter::Immediate(ImmediateFileWriter::new(log_path)?);

    // Console layer: all levels, pretty format
    let console_layer = fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .with_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("access_log=info,rodin=debug,info")),
        );

    // File layer: all access logs
    let file_layer = fmt::layer()
        .with_target(false)
        .with_level(false)
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(EnvFilter::new("access_log=info"));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Logging initialized (dev mode)");
    Ok(())
}

/// Prod: Console (errors only) + File (batched write every 5 seconds)
fn init_prod(log_path: PathBuf) -> anyhow::Result<()> {
    let flush_interval = Duration::from_secs(
        env::var("LOG_FLUSH_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5),
    );

    let file_writer = FileWriter::Buffered(BufferedFileWriter::new(log_path, flush_interval)?);

    // Console layer: errors only
    let console_layer = fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .with_filter(EnvFilter::new("error"));

    // File layer: all access logs, batched
    let file_layer = fmt::layer()
        .with_target(false)
        .with_level(false)
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(EnvFilter::new("access_log=info"));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Logging initialized (prod mode)");
    Ok(())
}

/// Access log middleware
/// Logs in format: "METHOD /path HTTP/1.1" STATUS CONTENT_LENGTH IP "User-Agent"
pub async fn access_log_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start = Instant::now();

    // Extract request info before consuming the request
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let user_agent = request
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();

    // Get real IP (supports proxy headers when TRUST_PROXY=true)
    let ip = get_client_ip(request.headers(), &addr);

    // Process the request
    let response = next.run(request).await;

    let latency = start.elapsed();
    let status = response.status().as_u16();
    let content_length = response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    // Format HTTP version
    let version_str = match version {
        axum::http::Version::HTTP_09 => "HTTP/0.9",
        axum::http::Version::HTTP_10 => "HTTP/1.0",
        axum::http::Version::HTTP_11 => "HTTP/1.1",
        axum::http::Version::HTTP_2 => "HTTP/2.0",
        axum::http::Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/?",
    };

    // Log in Apache-like format
    tracing::info!(
        target: "access_log",
        "\"{} {} {}\" {} {} {} \"{}\" {}ms",
        method,
        uri.path(),
        version_str,
        status,
        content_length,
        ip,
        user_agent,
        latency.as_millis()
    );

    response
}
