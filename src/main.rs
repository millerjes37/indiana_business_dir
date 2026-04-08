//! Indiana Business Directory Scraper — CLI entry point.
//!
//! This binary orchestrates three subcommands:
//! - `list`: Display available Indiana counties.
//! - `scrape`: Launch a Playwright browser, discover businesses by ZIP/city,
//!   enrich them from SOS detail pages, and export to CSV.
//! - `export`: Read existing SQLite records and write CSV without scraping.
//!
//! The scrape workflow delegates to modules in `src/scraper/`:
//! 1. `primary::scrape`   — Search SOS grid and insert discovered records.
//! 2. `secondary::scrape` — Visit detail pages and enrich discovered records.
//! 3. `tertiary::scrape`  — Mark enriched records complete (phone stub).

mod browser_driver;
mod cli;
mod counties;
mod db;
mod models;
mod output;
mod scraper;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use counties::{load_city_data, load_zip_data, normalize_county_name};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    match args.command {
        Commands::Scrape(scrape_args) => {
            let county = normalize_county_name(&scrape_args.county);
            let data = load_zip_data()?;
            if !data.contains_key(&county) {
                anyhow::bail!("County '{}' not found in Indiana data", county);
            }
            info!("Target county: {}", county);

            let db = db::Db::open(&scrape_args.db).context("Failed to open database")?;
            let mut driver = browser_driver::BrowserDriver::spawn(!scrape_args.headful).await?;

            let result = scraper::run(&scrape_args, &db, &mut driver).await;

            if let Err(e) = driver.close().await {
                warn!("Error closing browser driver: {}", e);
            }

            result?;

            let csv_path = output::write_csv(
                &db,
                &county,
                &scrape_args.output_dir,
                scrape_args.csv.as_deref(),
            )?;
            info!("CSV exported to: {}", csv_path);
            println!("\nDone! Output saved to: {}", csv_path);
            Ok(())
        }

        Commands::Export(export_args) => {
            let county = normalize_county_name(&export_args.county);
            let db = db::Db::open(&export_args.db).context("Failed to open database")?;
            let csv_path = output::write_csv(
                &db,
                &county,
                &export_args.output_dir,
                export_args.csv.as_deref(),
            )?;
            info!("CSV exported to: {}", csv_path);
            println!("\nExported! Output saved to: {}", csv_path);
            Ok(())
        }

        Commands::List(list_args) => {
            let zip_data = load_zip_data()?;
            let city_data = load_city_data()?;
            let mut counties: Vec<String> = zip_data.keys().cloned().collect();
            counties.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

            println!("Available Indiana counties ({} total):", counties.len());
            for c in counties {
                if list_args.counts {
                    let zips = zip_data.get(&c).map(|v| v.len()).unwrap_or(0);
                    let cities = city_data.get(&c).map(|v| v.len()).unwrap_or(0);
                    println!("  {:20}  {} ZIPs, {} cities", c, zips, cities);
                } else {
                    println!("  {}", c);
                }
            }
            Ok(())
        }
    }
}
