use anyhow::{anyhow, Result};
#[cfg(not(debug_assertions))]
use minify_html::{minify, Cfg as HtmlMinCfg};
use regex::Regex;
use std::{collections::HashMap, fs, path::PathBuf};
use typst_as_lib::{typst_kit_options::TypstKitFontOptions, TypstEngine};
use typst_html::HtmlDocument;
use typst_library::diag::SourceDiagnostic;

use crate::frontmatter::FrontMatter;

pub fn build_posts(preamble_path: &str, generated_dir: &str) -> Result<Vec<FrontMatter>> {
    let out_dir = PathBuf::from(generated_dir);
    fs::create_dir_all(&out_dir)?;
    let binaries = load_binary_assets()?;
    let preamble = load_preamble(preamble_path);

    let mut index = Vec::new();
    for entry in fs::read_dir("content")? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("typ") {
            continue;
        }
        let slug = entry
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap()
            .to_string();
        // Files beginning with "_" are ignored (drafts/partials)
        if slug.starts_with('_') {
            continue;
        }
        let raw = fs::read_to_string(entry.path())?;
        let (meta, body) = parse_front_matter(&slug, &raw);
        let body_clean = strip_preamble_import(&body);
        let html = compile_typst(&preamble, &body_clean, &binaries)?;

        let html_path = out_dir.join(format!("{slug}.html"));
        fs::write(&html_path, maybe_minify_html(html.clone()))?;

        let mut meta_out = meta.clone();
        meta_out.html = format!("generated/{slug}.html");
        meta_out.reading_minutes = Some(estimate_reading_minutes(&html));
        index.push(meta_out);
    }

    println!("cargo:warning=generated {} posts", index.len());
    Ok(index)
}

pub fn build_home(preamble_path: &str, generated_dir: &str) -> Result<()> {
    let path = PathBuf::from("content/_home.typ");
    if !path.exists() {
        return Ok(());
    }

    let out_dir = PathBuf::from(generated_dir);
    fs::create_dir_all(&out_dir)?;

    let binaries = load_binary_assets()?;
    let preamble = load_preamble(preamble_path);
    let raw = fs::read_to_string(&path)?;
    let body_clean = strip_preamble_import(&raw);

    // Inject helpers: direct json load and pre-rendered HTML cards (built in Rust).
    let index_path = PathBuf::from(generated_dir).join("index.json");
    let index_bytes = fs::read(&index_path)?;
    let metas: Vec<FrontMatter> = serde_json::from_slice(&index_bytes)?;
    let cards_html = build_cards_html(&metas);
    let injected = format!(
        "#let __posts_items = json(\"static/generated/index.json\")\n#let __posts_list_html = raw({cards_html:?}, lang: \"html\")\n{body_clean}"
    );

    let mut html = compile_typst(&preamble, &injected, &binaries)?;
    // Replace placeholder (both bare text and wrapped in <p>) with pre-rendered cards HTML.
    html = html.replace("<p>POSTS_LIST_PLACEHOLDER</p>", &cards_html);
    html = html.replace("POSTS_LIST_PLACEHOLDER", &cards_html);

    let html_path = out_dir.join("home.html");
    fs::write(&html_path, maybe_minify_html(html))?;
    println!("cargo:warning=generated home page");
    Ok(())
}

pub fn build_profile(preamble_path: &str, generated_dir: &str) -> Result<Option<FrontMatter>> {
    let path = PathBuf::from("content/_profile.typ");
    if !path.exists() {
        return Ok(None);
    }

    let out_dir = PathBuf::from(generated_dir);
    fs::create_dir_all(&out_dir)?;

    let binaries = load_binary_assets()?;
    let preamble = load_preamble(preamble_path);
    let raw = fs::read_to_string(&path)?;
    let (mut meta, body) = parse_front_matter("_profile", &raw);
    let body_clean = strip_preamble_import(&body);

    let html = compile_typst(&preamble, &body_clean, &binaries)?;
    let html_path = out_dir.join("profile.html");
    fs::write(&html_path, maybe_minify_html(html.clone()))?;

    // enrich meta for runtime usage
    meta.html = "generated/profile.html".to_string();
    meta.reading_minutes = Some(estimate_reading_minutes(&html));

    // persist meta for runtime renderer
    let meta_path = out_dir.join("profile_meta.json");
    fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    println!("cargo:warning=generated profile page");
    Ok(Some(meta))
}

fn parse_front_matter(slug: &str, source: &str) -> (FrontMatter, String) {
    let mut fm = FrontMatter {
        slug: slug.to_string(),
        ..Default::default()
    };
    let mut meta_map: HashMap<String, String> = HashMap::new();
    let mut body_lines = Vec::new();
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("//:") {
            let trimmed = rest.trim();
            if let Some(val) = trimmed.strip_prefix("title:") {
                fm.title = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("subtitle:") {
                fm.subtitle = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("genre:") {
                fm.genre = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("tags:") {
                fm.tags = val
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("date:") {
                fm.published_at = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("updated:") {
                fm.updated_at = Some(val.trim().to_string());
                continue;
            }
            if let Some(val) = trimmed.strip_prefix("meta>") {
                // format: meta>key: value
                if let Some((k, v)) = val.split_once(':') {
                    let key = k.trim().to_string();
                    let val = v.trim().to_string();
                    if !key.is_empty() && !val.is_empty() {
                        meta_map.insert(key, val);
                    }
                    continue;
                }
            }
        }
        body_lines.push(line);
    }
    // Fill missing meta defaults from front matter and site-wide defaults.
    if let Some(title) = fm.title.as_ref() {
        // Provide both standard and OG titles automatically when omitted.
        meta_map
            .entry("title".to_string())
            .or_insert_with(|| title.clone());
        meta_map
            .entry("og:title".to_string())
            .or_insert_with(|| title.clone());
    }

    // Site-wide defaults (apply only when absent).
    meta_map
        .entry("author".to_string())
        .or_insert_with(|| "すずねーう".to_string());
    meta_map
        .entry("link:author".to_string())
        .or_insert_with(|| "https://twitter.com/suzuneu_discord".to_string());
    meta_map
        .entry("referrer".to_string())
        .or_insert_with(|| "strict-origin-when-cross-origin".to_string());
    meta_map
        .entry("og:site_name".to_string())
        .or_insert_with(|| "すずねーうのウェブサイト".to_string());
    meta_map
        .entry("og:locale".to_string())
        .or_insert_with(|| "ja_JP".to_string());
    meta_map
        .entry("og:type".to_string())
        .or_insert_with(|| "article".to_string());

    // Derive article:published_time from `date:` when provided and not explicitly set.
    if !meta_map.contains_key("article:published_time") {
        if let Some(date) = fm.published_at.as_ref() {
            let derived = if date.contains('T') {
                date.clone()
            } else {
                format!("{date}T00:00:00.000Z")
            };
            meta_map.insert("article:published_time".to_string(), derived);
        }
    }

    if !meta_map.is_empty() {
        fm.meta = meta_map;
    }
    (fm, body_lines.join("\n"))
}

fn compile_typst(
    preamble: &str,
    typst_source: &str,
    binaries: &[(String, Vec<u8>)],
) -> Result<String> {
    let combined = format!("{preamble}\n{typst_source}");
    dbg!(
        "Binary assets loaded:",
        binaries.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
    let engine = TypstEngine::builder()
        .search_fonts_with(TypstKitFontOptions::default())
        .with_static_file_resolver(
            binaries
                .iter()
                .map(|(p, b)| (p.as_str(), b.as_slice()))
                .collect::<Vec<_>>(),
        )
        .main_file(combined)
        .build();

    let result = engine.compile::<HtmlDocument>();
    if !result.warnings.is_empty() {
        for w in &result.warnings {
            println!("cargo:warning=Typst warning: {}", w.message);
        }
    }
    let doc = result.output.map_err(|e| anyhow!(e.to_string()))?;
    let html = typst_html::html(&doc).map_err(|diags| anyhow!(format_diagnostics(&diags)))?;
    Ok(postprocess_typst_html(&html))
}

fn format_diagnostics(diags: &[SourceDiagnostic]) -> String {
    diags
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn postprocess_typst_html(raw: &str) -> String {
    let meta_re = Regex::new("(?is)<meta[^>]*>").expect("valid regex");
    let cleaned = meta_re.replace_all(raw, "");
    let body_re = Regex::new("(?is)<body[^>]*>(.*?)</body>").expect("valid regex");
    let content = body_re
        .captures(&cleaned)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .unwrap_or_else(|| cleaned.trim());
    content.trim().to_string()
}

fn estimate_reading_minutes(html: &str) -> u32 {
    // crude: strip tags, count non-whitespace chars; assume 500 chars/min, min 1 min
    let tag_re = Regex::new("<[^>]+>").expect("valid regex");
    let text = tag_re.replace_all(html, " ");
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    let per_min = 500usize;
    let mins = chars.div_ceil(per_min);
    mins.max(1) as u32
}

#[cfg(not(debug_assertions))]
fn maybe_minify_html(html: String) -> String {
    let cfg = HtmlMinCfg {
        minify_js: false,
        minify_css: false,
        keep_closing_tags: true,
        ..Default::default()
    };
    let min = minify(html.as_bytes(), &cfg);
    String::from_utf8(min).unwrap_or(html)
}

#[cfg(debug_assertions)]
fn maybe_minify_html(html: String) -> String {
    html
}

fn load_binary_assets() -> Result<Vec<(String, Vec<u8>)>> {
    let mut bins = Vec::new();
    let images_dir = PathBuf::from("static/images");
    if images_dir.exists() {
        for entry in fs::read_dir(&images_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(
                        ext.to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "svg" | "gif" | "webp"
                    ) {
                        let bytes = fs::read(&path)?;
                        let fname = path.file_name().unwrap().to_string_lossy().to_string();
                        let variants = [
                            fname.clone(),
                            format!("/{fname}"),
                            format!("/images/{fname}"),
                            format!("static/images/{fname}"),
                            format!("./static/images/{fname}"),
                        ];
                        for v in variants {
                            bins.push((v, bytes.clone()));
                        }
                    }
                }
            }
        }
    }

    // Add other assets
    let other_assets = vec![
        (
            "github-light.tmTheme".to_string(),
            include_bytes!("../static/github-light.tmTheme").to_vec(),
        ),
        (
            "github-dark.tmTheme".to_string(),
            include_bytes!("../static/github-dark.tmTheme").to_vec(),
        ),
    ];

    bins.extend(other_assets);

    // Allow Typst to read generated index.json when building _home.typ.
    let gen_index = PathBuf::from("static/generated/index.json");
    if gen_index.exists() {
        if let Ok(bytes) = fs::read(&gen_index) {
            let variants = [
                "static/generated/index.json".to_string(),
                "/static/generated/index.json".to_string(),
                "./static/generated/index.json".to_string(),
                "generated/index.json".to_string(),
            ];
            for v in variants {
                bins.push((v, bytes.clone()));
            }
        }
    }

    Ok(bins)
}

fn load_preamble(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| {
        r#"// Global defaults; each article can override below.
"#
        .to_string()
    })
}

fn strip_preamble_import(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let l = line.trim_start();
            !(l.starts_with("#import") && l.contains("preamble.typ"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_cards_html(metas: &[FrontMatter]) -> String {
    metas
        .iter()
        .take(5)
        .map(|post| {
            let title = post.title.as_deref().unwrap_or(&post.slug);
            let slug = &post.slug;
            let date = post.published_at.as_deref().unwrap_or("");
            let updated = post.updated_at.as_deref().unwrap_or(date);
            let tags = &post.tags;
            let description = post
                .meta
                .get("description")
                .or_else(|| post.meta.get("og:description"))
                .map(String::as_str)
                .unwrap_or("");

            let tags_html = if !tags.is_empty() {
                let chips: String = tags
                    .iter()
                    .map(|t| format!("<span class=\"px-2 py-1 text-xs rounded-full bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-200\">#{}</span>", t))
                    .collect::<Vec<_>>()
                    .join("");
                format!("<div class=\"flex flex-wrap gap-2 pt-1\">{chips}</div>")
            } else {
                String::new()
            };

            let updated_html = if !updated.is_empty() {
                format!("Updated: {}", updated)
            } else {
                String::new()
            };
            let published_html = if !date.is_empty() {
                format!("Published: {}", date)
            } else {
                String::new()
            };

                format!(
                "<article class=\"post-card space-y-2 border border-slate-200 dark:border-slate-700 rounded-xl p-4 bg-white/90 dark:bg-slate-900/70 shadow-sm backdrop-blur\">\
                    <div class=\"flex items-start justify-between gap-3\">\
                        <a href=\"/blog/{slug}\" class=\"text-lg font-semibold text-slate-900 dark:text-slate-100 hover:text-sky-600 dark:hover:text-sky-400\">{title}</a>\
                        <div class=\"text-xs text-slate-500 dark:text-slate-400 text-right\">{updated_html}</div>\
                    </div>\
                    <div class=\"text-sm text-slate-700 dark:text-slate-300 line-clamp-2\">{description}</div>\
                    <div class=\"text-sm text-slate-600 dark:text-slate-300\">{published_html}</div>\
                    {tags_html}\
                </article>"
            )
        })
        .collect::<Vec<_>>()
        .join("")
}
