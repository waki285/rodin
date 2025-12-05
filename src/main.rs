mod app;
mod asset;
mod components;
mod frontmatter;
mod logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init()?;
    app::run().await
}
