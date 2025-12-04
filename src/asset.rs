use std::{collections::HashMap, fs, path::Path, sync::LazyLock};

static MANIFEST: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let path = Path::new("static/generated/assets-manifest.json");
    if let Ok(s) = fs::read_to_string(path) {
        if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(&s) {
            return m;
        }
    }
    HashMap::new()
});

/// Resolve an asset path using generated manifest. If manifest missing or key not found,
/// returns the original `path`.
pub fn asset_url(path: &str) -> String {
    MANIFEST.get(path).cloned().unwrap_or_else(|| path.to_string())
}
