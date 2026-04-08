use crate::db::Db;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;

pub fn write_csv(db: &Db, county: &str, output_dir: &Path, explicit_csv: Option<&Path>) -> Result<String> {
    let records = db.get_records_for_export(county)?;

    let path = if let Some(p) = explicit_csv {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        p.to_path_buf()
    } else {
        let county_slug = county.to_lowercase().replace(' ', "_");
        let dir = output_dir.join(&county_slug);
        fs::create_dir_all(&dir)?;
        let filename = format!(
            "{}_{}_{}.csv",
            county_slug,
            Utc::now().format("%Y-%m-%d"),
            Utc::now().timestamp()
        );
        dir.join(&filename)
    };

    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "business_id",
        "business_name",
        "entity_type",
        "status",
        "creation_date",
        "principal_address",
        "principal_city",
        "principal_zip",
        "county",
        "jurisdiction",
        "inactive_date",
        "expiration_date",
        "report_due_date",
        "registered_agent_name",
        "registered_agent_address",
        "governing_persons",
        "filing_history",
        "phone_number",
        "enrichment_status",
        "scraped_at",
    ])?;

    for r in records {
        wtr.write_record([
            &r.business_id,
            r.business_name.as_deref().unwrap_or(""),
            r.entity_type.as_deref().unwrap_or(""),
            r.status.as_deref().unwrap_or(""),
            r.creation_date.as_deref().unwrap_or(""),
            r.principal_address.as_deref().unwrap_or(""),
            r.principal_city.as_deref().unwrap_or(""),
            r.principal_zip.as_deref().unwrap_or(""),
            &r.county,
            r.jurisdiction.as_deref().unwrap_or(""),
            r.inactive_date.as_deref().unwrap_or(""),
            r.expiration_date.as_deref().unwrap_or(""),
            r.report_due_date.as_deref().unwrap_or(""),
            r.registered_agent_name.as_deref().unwrap_or(""),
            r.registered_agent_address.as_deref().unwrap_or(""),
            r.governing_persons.as_deref().unwrap_or(""),
            r.filing_history.as_deref().unwrap_or(""),
            r.phone_number.as_deref().unwrap_or(""),
            r.enrichment_status.as_str(),
            &r.updated_at.to_rfc3339(),
        ])?;
    }

    wtr.flush()?;
    Ok(path.to_string_lossy().to_string())
}
