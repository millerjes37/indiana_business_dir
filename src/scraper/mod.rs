use crate::browser_driver::BrowserDriver;
use crate::cli::ScrapeArgs;
use crate::db::Db;
use anyhow::Result;
use tracing::info;

pub mod primary;
pub mod secondary;
pub mod tertiary;

pub async fn run(args: &ScrapeArgs, db: &Db, driver: &mut BrowserDriver) -> Result<()> {
    let county = args.county.clone();

    if !args.skip_primary {
        info!("Starting primary scraper for {}", county);
        primary::scrape(driver, db, args, &county).await?;
    }

    if !args.skip_secondary {
        info!("Starting secondary scraper for {}", county);
        secondary::scrape(driver, db, &county).await?;
    }

    if !args.skip_tertiary {
        info!("Starting tertiary scraper for {}", county);
        tertiary::scrape(driver, db, &county).await?;
    }

    Ok(())
}
