use crate::browser_driver::BrowserDriver;
use crate::cli::{ScrapeArgs, SearchMode};
use crate::counties::{load_city_data, load_zip_data, normalize_county_name};
use crate::db::Db;
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Primary scraper: discover business IDs by searching ZIP codes or cities.
///
/// This function loads the location list for the target county from embedded
/// Census JSON (`in_zips.json` or `in_cities.json`), optionally filters it
/// by the `--city` CLI argument, and then iterates over each location.
///
/// For every location it:
/// 1. Submits a search via the browser driver (`search_zip` or `search_city`).
/// 2. Enters a pagination loop:
///    - Extracts all rows from `table#grid_businessList`.
///    - Upserts each row into SQLite with `enrichment_status = discovered`.
///      The upsert uses `COALESCE` so existing enriched data is preserved.
///    - Reads pagination info to determine if more pages exist.
///    - Clicks "Next" and sleeps for `--page-delay-ms`.
/// 3. Sleeps for `--search-delay-ms` before the next location.
///
/// The `detail_business_id`, `detail_business_type`, and `detail_is_series`
/// values scraped from the grid link are stored in SQLite so the secondary
/// scraper can navigate directly to the correct SOS detail page later.
pub async fn scrape(
    driver: &mut BrowserDriver,
    db: &Db,
    args: &ScrapeArgs,
    county: &str,
) -> Result<()> {
    let county_norm = normalize_county_name(county);

    let mut locations = match args.search_mode {
        SearchMode::Zip => {
            let data = load_zip_data()?;
            data.get(&county_norm).cloned().unwrap_or_default()
        }
        SearchMode::City => {
            let data = load_city_data()?;
            data.get(&county_norm).cloned().unwrap_or_default()
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

    info!(
        "Found {} locations to search for {}",
        locations.len(),
        county_norm
    );

    for (idx, location) in locations.iter().enumerate() {
        info!(
            "[{}/{}] Searching location: {}",
            idx + 1,
            locations.len(),
            location
        );

        let error = match reset_search(driver, args.search_mode, location).await {
            Ok(err) => err,
            Err(e) => {
                warn!("Search failed for {}: {}", location, e);
                sleep(Duration::from_millis(args.search_delay_ms)).await;
                continue;
            }
        };

        if let Some(err) = error {
            warn!("Search error for {}: {}", location, err);
            sleep(Duration::from_millis(args.search_delay_ms)).await;
            continue;
        }

        let mut page_num = 1;
        let mut session_retries = 0;
        const MAX_SESSION_RETRIES: usize = 3;

        loop {
            let rows = match driver.extract_results().await {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "extract_results failed for {} page {}: {}",
                        location, page_num, e
                    );
                    session_retries += 1;
                    if session_retries > MAX_SESSION_RETRIES {
                        warn!(
                            "Too many errors for {}, skipping to next location",
                            location
                        );
                        break;
                    }
                    if let Err(e2) = reset_search(driver, args.search_mode, location).await {
                        warn!("Failed to reset search for {}: {}", location, e2);
                        break;
                    }
                    page_num = 1;
                    continue;
                }
            };
            info!("  Page {}: extracted {} rows", page_num, rows.len());

            for row in rows {
                // Use detail_business_id if available, else the display ID
                let bid = row
                    .detail_business_id
                    .unwrap_or_else(|| row.business_id_display.clone());
                if bid.is_empty() {
                    continue;
                }
                let name = if row.business_name.is_empty() {
                    None
                } else {
                    Some(row.business_name.as_str())
                };
                let entity_type = if row.entity_type.is_empty() {
                    None
                } else {
                    Some(row.entity_type.as_str())
                };
                let status = if row.status.is_empty() {
                    None
                } else {
                    Some(row.status.as_str())
                };
                let principal_address = if row.principal_address.is_empty() {
                    None
                } else {
                    Some(row.principal_address.as_str())
                };
                let registered_agent_name = if row.registered_agent_name.is_empty() {
                    None
                } else {
                    Some(row.registered_agent_name.as_str())
                };
                let detail_business_type = row
                    .detail_business_type
                    .as_deref()
                    .filter(|s| !s.is_empty());
                let detail_is_series = row.detail_is_series.as_deref().filter(|s| !s.is_empty());
                if let Err(e) = db.insert_discovered(
                    &county_norm,
                    &bid,
                    name,
                    entity_type,
                    status,
                    principal_address,
                    registered_agent_name,
                    detail_business_type,
                    detail_is_series,
                ) {
                    warn!("Failed to insert {}: {}", bid, e);
                }
            }

            let pagination = match driver.get_pagination_info().await {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "get_pagination_info failed for {} page {}: {}",
                        location, page_num, e
                    );
                    session_retries += 1;
                    if session_retries > MAX_SESSION_RETRIES {
                        warn!(
                            "Too many errors for {}, skipping to next location",
                            location
                        );
                        break;
                    }
                    if let Err(e2) = reset_search(driver, args.search_mode, location).await {
                        warn!("Failed to reset search for {}: {}", location, e2);
                        break;
                    }
                    page_num = 1;
                    continue;
                }
            };
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
            let clicked = match driver.click_next().await {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        "click_next failed for {} page {}: {}",
                        location, page_num, e
                    );
                    session_retries += 1;
                    if session_retries > MAX_SESSION_RETRIES {
                        warn!(
                            "Too many session errors for {}, skipping to next location",
                            location
                        );
                        break;
                    }
                    warn!(
                        "Re-searching {} to reset session timer (retry {}/{})...",
                        location, session_retries, MAX_SESSION_RETRIES
                    );
                    if let Err(e2) = reset_search(driver, args.search_mode, location).await {
                        warn!("Failed to reset search for {}: {}", location, e2);
                        break;
                    }
                    page_num = 1;
                    continue;
                }
            };
            if !clicked {
                warn!("  Pagination ended unexpectedly");
                break;
            }
            page_num += 1;
            session_retries = 0;
        }

        sleep(Duration::from_millis(args.search_delay_ms)).await;
    }

    info!("Primary scraper complete for {}", county_norm);
    Ok(())
}

async fn reset_search(
    driver: &mut BrowserDriver,
    mode: SearchMode,
    location: &str,
) -> Result<Option<String>> {
    match mode {
        SearchMode::Zip => driver.search_zip(location).await,
        SearchMode::City => driver.search_city(location).await,
    }
}
