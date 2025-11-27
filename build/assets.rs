use anyhow::Result;
use minifier::{css::minify as minify_css, js::minify as minify_js};
use std::{fs, path::PathBuf};

pub fn minify_assets() -> Result<()> {
    let out_dir = PathBuf::from("static/build");
    fs::create_dir_all(&out_dir)?;

    let css_files = ["custom.css", "tailwind-fallback.css"];
    for file in css_files {
        let src = PathBuf::from("static").join(file);
        let dst = out_dir.join(file);
        if src.exists() {
            let content = fs::read_to_string(&src)?;
            let min = minify_css(&content)
                .map(|m| m.to_string())
                .unwrap_or_else(|_| content.clone());
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
