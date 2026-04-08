use anyhow::{Context, Result};
use std::collections::HashMap;

pub fn load_zip_data() -> Result<HashMap<String, Vec<String>>> {
    let data = include_str!("../data/in_zips.json");
    let map: HashMap<String, Vec<String>> =
        serde_json::from_str(data).context("Failed to parse in_zips.json")?;
    Ok(map)
}

pub fn load_city_data() -> Result<HashMap<String, Vec<String>>> {
    let data = include_str!("../data/in_cities.json");
    let map: HashMap<String, Vec<String>> =
        serde_json::from_str(data).context("Failed to parse in_cities.json")?;
    Ok(map)
}

pub fn list_counties(map: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut counties: Vec<String> = map.keys().cloned().collect();
    counties.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    counties
}

pub fn normalize_county_name(name: &str) -> String {
    let trimmed = name.trim();
    // Title case each word, but preserve "St. Joseph"
    trimmed
        .split_whitespace()
        .map(|word| {
            let lower = word.to_lowercase();
            if lower.starts_with("st.") || lower == "st" {
                // Handle St. Joseph specially
                if lower == "st" {
                    "St.".to_string()
                } else {
                    let rest = &word[3..];
                    format!("St.{}", rest)
                }
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
