use crate::browser_driver::BrowserDriver;
use crate::cli::{ScrapeArgs, SearchMode};
use crate::counties::{load_city_data, load_zip_data, normalize_county_name};
use crate::db::Db;
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

pub async fn scrape(driver: &mut BrowserDriver, db: &Db, args: &ScrapeArgs, county: &str) -> Result<()> {
    let county_norm = normalize_county_name(county);

    let mut locations = match args.search_mode {
        SearchMode::Zip => {
            let data = load_zip_data()?;
            data.get(&county_norm)
                .cloned()
                .unwrap_or_default()
        }
        SearchMode::City => {
            let data = load_city_data()?;
            data.get(&county_norm)
                .cloned()
                .unwrap_or_default()
        }
    };

    // Filter to specific city if requested
    if let Some(ref target_city) = args.city {
        let target_lower = target_city.to_lowercase();
        locations.retain(|loc| loc.to_lowercase().contains(&target_lower));
        if locations.is_empty() {
            anyhow::bail!("City '{}' not found in {}", target_city, county_norm);
        }
    }

    if locations.is_empty() {
        anyhow::bail!("No locations found for county: {}", county_norm);
    }

    let locations: Vec<String> = if let Some(limit) = args.limit {
        locations.into_iter().take(limit).collect()
    } else {
        locations
    };

    info!("Found {} locations to search for {}", locations.len(), county_norm);

    for (idx, location) in locations.iter().enumerate() {
        info!("[{}/{}] Searching location: {}", idx + 1, locations.len(), location);

        let error = match args.search_mode {
            SearchMode::Zip => driver.search_zip(location).await?,
            SearchMode::City => driver.search_city(location).await?,
        };

        if let Some(err) = error {
            warn!("Search error for {}: {}", location, err);
            sleep(Duration::from_millis(args.search_delay_ms)).await;
            continue;
        }

        let mut page_num = 1;
        loop {
            let rows = driver.extract_results().await?;
            info!("  Page {}: extracted {} rows", page_num, rows.len());

            for row in rows {
                // Use detail_business_id if available, else the display ID
                let bid = row.detail_business_id.unwrap_or_else(|| row.business_id_display.clone());
                if bid.is_empty() {
                    continue;
                }
                let name = if row.business_name.is_empty() { None } else { Some(row.business_name.as_str()) };
                let entity_type = if row.entity_type.is_empty() { None } else { Some(row.entity_type.as_str()) };
                let status = if row.status.is_empty() { None } else { Some(row.status.as_str()) };
                let principal_address = if row.principal_address.is_empty() { None } else { Some(row.principal_address.as_str()) };
                let registered_agent_name = if row.registered_agent_name.is_empty() { None } else { Some(row.registered_agent_name.as_str()) };
                let detail_business_type = row.detail_business_type.as_deref().filter(|s| !s.is_empty());
                let detail_is_series = row.detail_is_series.as_deref().filter(|s| !s.is_empty());
                if let Err(e) = db.insert_discovered(
                    &county_norm, &bid, name, entity_type, status,
                    principal_address, registered_agent_name,
                    detail_business_type, detail_is_series
                ) {
                    warn!("Failed to insert {}: {}", bid, e);
                }
            }

            let pagination = driver.get_pagination_info().await?;
            info!("  Pagination: {}", pagination.text);

            // Check if there are more pages
            let has_more = match (pagination.current_page, pagination.total_pages) {
                (Some(cur), Some(total)) => cur < total,
                _ => false,
            };

            if !has_more {
                break;
            }

            // Click next
            sleep(Duration::from_millis(args.page_delay_ms)).await;
            let clicked = driver.click_next().await?;
            if !clicked {
                warn!("  Pagination ended unexpectedly");
                break;
            }
            page_num += 1;
        }

        sleep(Duration::from_millis(args.search_delay_ms)).await;
    }

    info!("Primary scraper complete for {}", county_norm);
    Ok(())
}
