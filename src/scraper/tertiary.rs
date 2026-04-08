use crate::browser_driver::BrowserDriver;
use crate::db::Db;
use crate::models::EnrichmentStatus;
use anyhow::Result;
use tracing::{info, warn};

pub async fn scrape(_driver: &mut BrowserDriver, db: &Db, county: &str) -> Result<()> {
    let ids = db.get_pending_ids(county, EnrichmentStatus::Enriched)?;
    info!(
        "Tertiary scraper: {} businesses to enrich with phone numbers",
        ids.len()
    );

    // Phone enrichment is left as a stub for external APIs or manual lookup.
    // To implement, you could:
    // 1. Search DuckDuckGo/Google for "{business_name} {city} Indiana phone"
    // 2. Use a paid API like Google Places, Yelp Fusion, or Whitepages
    // 3. Scrape yellow pages directories
    //
    // For now, we mark all enriched records as complete without phone numbers.
    for (idx, business_id) in ids.iter().enumerate() {
        info!(
            "[{}/{}] Phone enrichment stub for {}",
            idx + 1,
            ids.len(),
            business_id
        );
        if let Err(e) = db.update_phone(business_id, None) {
            warn!("Failed to update phone for {}: {}", business_id, e);
        }
    }

    info!("Tertiary scraper complete");
    Ok(())
}
