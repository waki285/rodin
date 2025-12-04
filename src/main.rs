mod app;
mod components;
mod frontmatter;
mod asset;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
