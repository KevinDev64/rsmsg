use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Clone)]
pub struct Localization {
    fallback: BTreeMap<String, String>,
    current: BTreeMap<String, String>,
}

impl Localization {
    pub fn load(language: &str) -> Self {
        let language = normalize_language(language);
        let fallback = load_language_file("en").unwrap_or_else(embedded_en);
        let current = if language == "en" {
            fallback.clone()
        } else if language == "ru" {
            load_language_file(&language).unwrap_or_else(embedded_ru)
        } else {
            load_language_file(&language).unwrap_or_default()
        };
        Self { fallback, current }
    }

    pub fn text(&self, key: &str) -> String {
        self.current
            .get(key)
            .or_else(|| self.fallback.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

fn normalize_language(language: &str) -> String {
    match language {
        "ru" => "ru".to_string(),
        _ => "en".to_string(),
    }
}

fn load_language_file(language: &str) -> Option<BTreeMap<String, String>> {
    for path in locale_paths(language) {
        let Ok(raw) = fs::read_to_string(path) else {
            continue;
        };
        if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&raw) {
            return Some(map);
        }
    }
    None
}

fn locale_paths(language: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("locales/{language}.json")),
        PathBuf::from(format!("crates/client-ui/locales/{language}.json")),
    ]
}

fn embedded_en() -> BTreeMap<String, String> {
    serde_json::from_str(include_str!("../locales/en.json")).unwrap_or_default()
}

fn embedded_ru() -> BTreeMap<String, String> {
    serde_json::from_str(include_str!("../locales/ru.json")).unwrap_or_default()
}
