use anyhow::{anyhow, Result};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Targets};
use minifier::js::minify as minify_js;
use std::{fs, path::PathBuf};

pub fn minify_assets() -> Result<()> {
    let out_dir = PathBuf::from("static/build");
    fs::create_dir_all(&out_dir)?;

    let targets = browserslist_targets()?;

    let css_files = [
        "critical.css",
        "lazy.css",
        "post-card.css",
        "prose.css",
        "search.css",
        "test.css",
        "tailwind-fallback.css",
    ];
    for file in css_files {
        let src = PathBuf::from("static").join(file);
        let dst = out_dir.join(file);
        if src.exists() {
            let content = fs::read_to_string(&src)?;
            let min = minify_css_with_prefix(&content, targets).unwrap_or_else(|err| {
                eprintln!("warning: failed to minify {file} with lightningcss: {err}");
                content.clone()
            });
            fs::write(dst, min)?;
        }
    }

    let js_files = ["app.js", "home.js"];
    for file in js_files {
        let src = PathBuf::from("static").join(file);
        let dst = out_dir.join(file);
        if src.exists() {
            let content = fs::read_to_string(&src)?;
            let min = minify_js(&content).to_string();
            fs::write(dst, min)?;
        }
    }

    Ok(())
}

fn browserslist_targets() -> Result<Targets> {
    let browsers = Browsers::from_browserslist(vec!["> 0.5%", "not dead"])?.unwrap_or_default();
    Ok(browsers.into())
}

fn minify_css_with_prefix(content: &str, targets: Targets) -> Result<String> {
    let mut sheet =
        StyleSheet::parse(content, ParserOptions::default()).map_err(|e| anyhow!(e.to_string()))?;

    sheet
        .minify(MinifyOptions {
            targets,
            ..Default::default()
        })
        .map_err(|e| anyhow!(e.to_string()))?;

    let res = sheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            ..Default::default()
        })
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(res.code)
}
