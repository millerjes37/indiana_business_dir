use crate::browser_driver::BrowserDriver;
use crate::db::Db;
use crate::models::{BusinessRecord, EnrichmentStatus};
use anyhow::Result;
use serde_json::Value;
use tracing::{info, warn};

pub async fn scrape(driver: &mut BrowserDriver, db: &Db, county: &str) -> Result<()> {
    let ids = db.get_pending_ids(county, EnrichmentStatus::Discovered)?;
    info!("Secondary scraper: {} businesses to enrich", ids.len());

    for (idx, business_id) in ids.iter().enumerate() {
        info!("[{}/{}] Enriching {}", idx + 1, ids.len(), business_id);

        let (bid, business_type, is_series) = match db.get_detail_params(business_id) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to get detail params for {}: {}", business_id, e);
                continue;
            }
        };

        let detail = match driver.get_detail(&bid, business_type.as_deref(), is_series.as_deref()).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to get detail for {}: {}", business_id, e);
                continue;
            }
        };

        let record = parse_detail(business_id, county, &detail);

        // Only update if we found at least one meaningful field to avoid overwriting discovered data
        if !has_meaningful_data(&record) {
            warn!("No meaningful detail data found for {}, skipping update", business_id);
            continue;
        }

        if let Err(e) = db.update_enriched(&record) {
            warn!("Failed to update enriched record {}: {}", business_id, e);
        }
    }

    info!("Secondary scraper complete");
    Ok(())
}

fn has_meaningful_data(record: &BusinessRecord) -> bool {
    record.business_name.is_some()
        || record.entity_type.is_some()
        || record.principal_address.is_some()
        || record.registered_agent_name.is_some()
        || record.status.is_some()
        || record.creation_date.is_some()
}

fn parse_detail(business_id: &str, county: &str, detail: &Value) -> BusinessRecord {
    use chrono::Utc;

    let kvs = detail
        .get("kvs")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let get = |key: &str| -> Option<String> {
        kvs.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    };

    let principal_address = get("principal office address");

    let mut principal_city = None;
    let mut principal_zip = None;
    if let Some(ref addr) = principal_address {
        let parts: Vec<&str> = addr.split(',').collect();
        if parts.len() >= 2 {
            principal_city = Some(parts[parts.len() - 2].trim().to_string());
        }
        if let Some(last) = parts.last() {
            let trimmed = last.trim();
            let zip_re = regex_extract_zip(trimmed);
            if let Some(z) = zip_re {
                principal_zip = Some(z);
            }
        }
    }

    let sections = detail
        .get("sections")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let governing_persons = extract_section_json(&sections, "governing");
    let filing_history = extract_section_json(&sections, "filing");

    BusinessRecord {
        business_id: business_id.to_string(),
        county: county.to_string(),
        business_name: get("business name"),
        entity_type: get("entity type"),
        status: get("business status"),
        creation_date: get("creation date"),
        principal_address,
        principal_city,
        principal_zip,
        jurisdiction: get("jurisdiction of formation"),
        inactive_date: get("inactive date"),
        expiration_date: get("expiration date"),
        report_due_date: get("business entity report due date"),
        registered_agent_name: get("registered agent name"),
        registered_agent_address: get("registered agent address"),
        governing_persons,
        filing_history,
        phone_number: None,
        detail_business_type: None,
        detail_is_series: None,
        enrichment_status: EnrichmentStatus::Enriched,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn extract_section_json(sections: &[Value], keyword: &str) -> Option<String> {
    let sec = sections.iter().find(|s| {
        s.get("heading")
            .and_then(|h| h.as_str())
            .map(|h| h.to_lowercase().contains(keyword))
            .unwrap_or(false)
    })?;
    let tables = sec.get("tables")?;
    serde_json::to_string(tables).ok()
}

fn regex_extract_zip(text: &str) -> Option<String> {
    // Simple heuristic: find first 5-digit sequence
    for word in text.split_whitespace() {
        let digits: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 5 {
            return Some(digits);
        }
    }
    None
}
