mod app;
mod admin_tasks;
mod asset;
mod components;
mod frontmatter;
mod logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logging::init()?;
    app::run().await
}
