mod admin_tasks;
mod app;
mod asset;
mod components;
pub mod constants;
mod fonts;
mod frontmatter;
mod logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logging::init()?;
    app::run().await
}
