mod app;
mod asset;
mod components;
mod frontmatter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
