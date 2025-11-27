use crate::frontmatter::FrontMatter;
use anyhow::Result;
use std::{fs, path::Path};

pub fn write_sitemap(metas: &[FrontMatter], site_url: &str, output_path: &str) -> Result<()> {
    let homepage_lastmod = latest_lastmod(metas);
    let mut urls = Vec::with_capacity(metas.len() + 2);
    urls.push(SitemapEntry {
        loc: format!("{site_url}/"),
        lastmod: homepage_lastmod.clone(),
    });
    urls.push(SitemapEntry {
        loc: format!("{site_url}/profile"),
        lastmod: homepage_lastmod.clone(),
    });

    for meta in metas {
        let loc = format!("{site_url}/blog/{}", meta.slug);
        let lastmod = meta
            .updated_at
            .as_ref()
            .or(meta.published_at.as_ref())
            .map(|s| s.trim().to_string());
        urls.push(SitemapEntry { loc, lastmod });
    }

    let xml = render_xml(&urls);
    if let Some(dir) = Path::new(output_path).parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(output_path, xml)?;
    println!("cargo:warning=generated sitemap with {} urls", urls.len());
    Ok(())
}

fn latest_lastmod(metas: &[FrontMatter]) -> Option<String> {
    metas
        .iter()
        .filter_map(|m| m.updated_at.as_ref().or(m.published_at.as_ref()))
        .map(|s| s.trim().to_string())
        .max()
}

#[derive(Clone)]
struct SitemapEntry {
    loc: String,
    lastmod: Option<String>,
}

fn render_xml(urls: &[SitemapEntry]) -> String {
    let mut body = String::new();
    for entry in urls {
        body.push_str("  <url>\n");
        body.push_str(&format!("    <loc>{}</loc>\n", &entry.loc));
        if let Some(lastmod) = entry.lastmod.as_ref() {
            body.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod));
        }
        body.push_str("  </url>\n");
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{body}</urlset>
"#
    )
}
