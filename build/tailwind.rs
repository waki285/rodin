use std::{fs, path::Path, process::Command};

pub fn build_tailwind() {
    let output = Command::new("node_modules/.bin/tailwindcss")
        .args([
            "-i",
            "static/input.css",
            "-o",
            "static/build/tailwind.css",
            "--minify",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            println!(
                "cargo:warning=tailwindcss generation failed (status {}), using fallback: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            write_fallback();
        }
        Err(err) => {
            println!(
                "cargo:warning=tailwindcss generation not run ({}), using fallback",
                err
            );
            write_fallback();
        }
    }
}

fn write_fallback() {
    let dst = Path::new("static/build/tailwind.css");
    if let Some(parent) = dst.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let css = include_str!("../static/tailwind-fallback.css");
    fs::write(dst, css).expect("write fallback css");
}
