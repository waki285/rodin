use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub genre: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub breadcrumbs: Vec<String>,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub slug: String,
    pub html: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
    #[serde(default)]
    pub markdown: Option<String>,
    #[serde(default)]
    pub reading_minutes: Option<u32>,
}
