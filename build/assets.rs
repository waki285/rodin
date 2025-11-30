use anyhow::{anyhow, Result};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Targets};
use std::{fs, path::PathBuf, process::Command};

pub fn minify_assets() -> Result<()> {
    let out_dir = PathBuf::from("static/build");
    fs::create_dir_all(&out_dir)?;

    let targets = browserslist_targets()?;

    let css_dir = PathBuf::from("static/css");
    fs::create_dir_all(&css_dir)?;
    for entry in fs::read_dir(&css_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("css") {
            continue;
        }
        let file_name = match path.file_name() {
            Some(name) => name.to_owned(),
            None => continue,
        };
        let file_str = file_name.to_string_lossy();
        let content = fs::read_to_string(&path)?;
        let min = minify_css_with_prefix(&content, targets).unwrap_or_else(|err| {
            eprintln!(
                "warning: failed to minify {} with lightningcss: {err}",
                file_str
            );
            content.clone()
        });
        fs::write(out_dir.join(file_name), min)?;
    }

    let js_files = ["app.js", "home.js"];
    for file in js_files {
        let src = PathBuf::from("static").join(file);
        let dst = out_dir.join(file);
        if !src.exists() {
            continue;
        }

        let esbuild = Command::new("node_modules/.bin/esbuild")
            .args([
                src.to_string_lossy().as_ref(),
                "--platform=browser",
                "--charset=utf8",
                "--minify",
                "--legal-comments=none",
                "--drop:console",
                "--tree-shaking=true",
                format!("--outfile={}", dst.to_string_lossy()).as_ref(),
            ])
            .output();

        let out = esbuild.map_err(|err| {
            anyhow!("esbuild not run for {file}: {err}. Ensure node_modules/.bin/esbuild exists.")
        })?;

        if !out.status.success() {
            return Err(anyhow!(
                "esbuild failed for {file} (status {}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            ));
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
