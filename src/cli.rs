use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "indiana_business_dir",
    about = "Scrape Indiana Secretary of State (INBiz) business entity records by county",
    after_long_help = TOP_LEVEL_AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Discover and enrich business records from the Indiana SOS website.
    ///
    /// This is the primary scraping workflow. It launches a Playwright-
    /// driven browser, searches the Indiana SOS database by ZIP code or
    /// city name, paginates through results, visits detail pages for each
    /// business, and persists everything to SQLite. Finally it exports
    /// the completed records to CSV.
    ///
    /// Because Indiana SOS is protected by Cloudflare and Google
    /// reCAPTCHA, live scraping REQUIRES --headful mode so you can
    /// solve the CAPTCHA manually in the opened browser window.
    #[command(name = "scrape", after_long_help = SCRAPE_AFTER_HELP)]
    Scrape(ScrapeArgs),

    /// Export existing database records to CSV without scraping.
    ///
    /// This command reads the local SQLite database for the specified
    /// county and writes a timestamped CSV to outputs/<county>/.
    /// No browser is launched, no network requests are made, and no
    /// CAPTCHA solving is required.
    #[command(name = "export", after_long_help = EXPORT_AFTER_HELP)]
    Export(ExportArgs),

    /// List all 92 Indiana counties available for scraping.
    ///
    /// Displays the counties loaded from the embedded Census ZIP-to-
    /// county and city-to-county mappings. You can use any of these
    /// names with the --county flag in the scrape or export commands.
    #[command(name = "list", after_long_help = LIST_AFTER_HELP)]
    List(ListArgs),
}

/// Arguments for the `scrape` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ScrapeArgs {
    /// Target Indiana county (e.g., "Marion", "Grant", "St. Joseph").
    ///
    /// The county name is normalized to title case automatically, so
    /// "grant", "GRANT", and "Grant" are all equivalent.
    #[arg(long, short, help = "County to scrape (e.g., Grant, Marion, Lake)")]
    pub county: String,

    /// Optional city filter within the selected county.
    ///
    /// When provided, only locations whose name contains this string
    /// are searched. This is useful for narrowing a large county down
    /// to a single municipality (e.g., --city "Gas City" within
    /// Grant County). The filter is case-insensitive and applies
    /// to the location list derived from --search-mode.
    #[arg(long, help = "Filter to a specific city/town within the county")]
    pub city: Option<String>,

    /// Base directory for CSV output files.
    ///
    /// Each run creates a subdirectory named after the county
    /// (lowercased, spaces replaced with underscores) and writes
    /// a timestamped CSV inside it:
    ///   <OUTPUT_DIR>/<county>/<county>_<YYYY-MM-DD>_<unix_epoch>.csv
    #[arg(long, short, default_value = "outputs", help = "Directory for generated CSV files")]
    pub output_dir: PathBuf,

    /// Run the Playwright browser in visible (headful) mode.
    ///
    /// REQUIRED FOR LIVE SCRAPING. The Indiana SOS website
    /// (bsd.sos.in.gov) is protected by Cloudflare and serves a
    /// Google reCAPTCHA challenge on every new search submission.
    /// In headful mode a browser window opens, allowing you to
    /// solve the CAPTCHA manually. The Node.js driver polls the
    /// DOM for the g-recaptcha-response token and resumes
    /// automatically once the challenge is cleared.
    ///
    /// If you omit this flag, the driver will time out on the
    /// first CAPTCHA and the scrape will fail.
    #[arg(long, help = "Open a visible browser window for manual CAPTCHA solving")]
    pub headful: bool,

    /// Resume a previous run using the SQLite database.
    ///
    /// Records whose enrichment_status is already 'complete' are
    /// skipped. Records in 'discovered' or 'enriched' states are
    /// re-processed starting from the next unhandled tier.
    /// This allows safe interruption and restart without losing
    /// already-paginated discovery results or detail-page data.
    #[arg(long, help = "Skip already-complete records and continue where you left off")]
    pub resume: bool,

    /// Skip the primary scraper (business discovery phase).
    ///
    /// Use this when you already have business IDs in the SQLite
    /// database for the target county and only want to run
    /// enrichment or export.
    #[arg(long, help = "Skip ZIP/city discovery and use existing DB records")]
    pub skip_primary: bool,

    /// Skip the secondary scraper (SOS detail enrichment).
    ///
    /// The secondary phase visits each business's detail page to
    /// extract creation dates, principal addresses, registered
    /// agents, and jurisdiction information.
    #[arg(long, help = "Skip visiting individual business detail pages")]
    pub skip_secondary: bool,

    /// Skip the tertiary scraper (phone number enrichment).
    ///
    /// Indiana SOS does not publish phone numbers. The tertiary
    /// phase is currently a stub that marks records 'complete'.
    /// You may implement external lookups (Google Places, Yelp,
    /// etc.) in src/scraper/tertiary.rs later.
    #[arg(long, help = "Skip external phone enrichment (currently a no-op stub)")]
    pub skip_tertiary: bool,

    /// Discovery strategy: zip or city.
    ///
    /// ZIP mode (default) iterates over every USPS ZIP code
    /// mapped to the target county via Census ZCTA data.
    /// This is the most thorough strategy but generates the
    /// largest number of searches (and therefore CAPTCHAs).
    ///
    /// City mode iterates over incorporated cities and towns
    /// within the county. For dense urban counties this often
    /// yields fewer searches than ZIP mode while still capturing
    /// the majority of businesses. Rural unincorporated areas
    /// may be missed.
    #[arg(long, default_value = "zip", help = "Discovery strategy: zip (exhaustive) or city (fewer CAPTCHAs)")]
    pub search_mode: SearchMode,

    /// Path to the SQLite database file.
    ///
    /// All discovered and enriched records are persisted here.
    /// The schema includes business_id (PRIMARY KEY), county,
    /// enrichment_status, addresses, agent info, and timestamps.
    /// You can inspect it directly with the sqlite3 CLI.
    #[arg(long, default_value = "indiana_business_dir.db", help = "Path to the SQLite state database")]
    pub db: PathBuf,

    /// Delay in milliseconds between pagination clicks.
    ///
    /// Indiana SOS uses ASP.NET postbacks for pagination.
    /// A short delay reduces the chance of stale-element or
    /// navigation-intercept errors. Default is 3000 ms.
    #[arg(long, default_value_t = 3000, help = "Wait time between Next-page clicks (default: 3000)")]
    pub page_delay_ms: u64,

    /// Delay in milliseconds between distinct searches.
    ///
    /// After finishing pagination for one ZIP/city, the scraper
    /// pauses this long before submitting the next search.
    /// This gives the browser time to settle and reduces the
    /// chance of being rate-limited by Cloudflare.
    #[arg(long, default_value_t = 5000, help = "Wait time between new ZIP/city searches (default: 5000)")]
    pub search_delay_ms: u64,

    /// Cap the number of locations to search.
    ///
    /// Useful for dry-runs or spot-checking a county. For
    /// example, --limit 1 will search only the first ZIP
    /// (or first city) and then stop.
    #[arg(long, help = "Only search the first N locations (useful for testing)")]
    pub limit: Option<usize>,

    /// Maximum seconds to wait for a manual CAPTCHA solve.
    ///
    /// The Playwright driver polls the page every 2 seconds
    /// for the g-recaptcha-response token. If the token does
    /// not appear within this window, the driver errors out
    /// and the Rust process exits. Increase this if you need
    /// more time to solve difficult image challenges.
    #[arg(long, default_value_t = 120, help = "Seconds to wait for manual CAPTCHA solution (default: 120)")]
    pub captcha_timeout: u64,

    /// Explicit CSV output file path.
    ///
    /// If provided, the final CSV is written to this exact path
    /// instead of the auto-generated
    /// outputs/<county>/<county>_<date>_<epoch>.csv location.
    /// Parent directories are created automatically.
    #[arg(long, short, help = "Explicit CSV file path (overrides auto-generated name)")]
    pub csv: Option<PathBuf>,
}

/// Arguments for the `export` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ExportArgs {
    /// County whose records should be exported.
    #[arg(long, short, help = "County to export (e.g., Grant, Marion)")]
    pub county: String,

    /// Output directory for the CSV file.
    ///
    /// Ignored if --csv is provided.
    #[arg(long, short, default_value = "outputs", help = "Directory for generated CSV")]
    pub output_dir: PathBuf,

    /// Explicit CSV output file path.
    ///
    /// If provided, the CSV is written to this exact path instead
    /// of the auto-generated location under --output-dir.
    #[arg(long, short, help = "Explicit CSV file path (overrides auto-generated name)")]
    pub csv: Option<PathBuf>,

    /// SQLite database path.
    #[arg(long, default_value = "indiana_business_dir.db", help = "Path to the SQLite state database")]
    pub db: PathBuf,
}

/// Arguments for the `list` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    /// Also print the number of ZIP codes and cities mapped to each county.
    #[arg(long, help = "Show ZIP and city counts per county")]
    pub counts: bool,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum SearchMode {
    /// Search every ZIP code in the county (most thorough, more CAPTCHAs).
    #[default]
    Zip,
    /// Search every incorporated city/town in the county (fewer CAPTCHAs).
    City,
}

const TOP_LEVEL_AFTER_HELP: &str = r#"
SYNOPSIS
    indiana_business_dir <COMMAND> [OPTIONS]

COMMANDS
    scrape    Discover and enrich business records from Indiana SOS
    export    Write CSV from existing SQLite data without scraping
    list      Display all 92 Indiana counties available for scraping

GETTING STARTED
    1. Install Node dependencies:
       npm install

    2. Build the release binary:
       cargo build --release
       # or: make release

    3. Scrape a county (headful mode is REQUIRED for CAPTCHA):
       ./indiana_business_dir scrape --county "Grant" --headful

    4. Export the results without scraping:
       ./indiana_business_dir export --county "Grant"

HELP PER SUBCOMMAND
    ./indiana_business_dir scrape --help
    ./indiana_business_dir export --help
    ./indiana_business_dir list --help

For full usage examples and CAPTCHA workflow details, see:
    ./indiana_business_dir scrape --help
"#;

const SCRAPE_AFTER_HELP: &str = r#"
EXAMPLES

  Scrape Grant County exhaustively by ZIP code:
    indiana_business_dir scrape --county "Grant" --headful

  Scrape only Gas City within Grant County (fewer CAPTCHAs):
    indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful

  Resume an interrupted Gas City run:
    indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful --resume

  Quick dry-run: search only the first ZIP:
    indiana_business_dir scrape --county "Grant" --headful --limit 1

  Skip discovery and only enrich existing records:
    indiana_business_dir scrape --county "Grant" --headful --skip-primary

  Skip phone enrichment stub and just discover + detail:
    indiana_business_dir scrape --county "Grant" --headful --skip-tertiary

CAPTCHA WORKFLOW

  1. Launch with --headful.
  2. A Chromium window opens to https://bsd.sos.in.gov/publicbusinesssearch.
  3. If a reCAPTCHA appears, solve it in the browser window.
  4. The terminal prints "CAPTCHA solved after X s" and continues.
  5. The scraper paginates through all results for that search.
  6. If another CAPTCHA appears on the next search, repeat step 3–5.

  Pagination within a single search does NOT trigger new CAPTCHAs.
  City mode usually requires fewer CAPTCHAs than ZIP mode.

OUTPUT FORMAT

  CSV files are written to:
    outputs/<county_name>/<county_name>_<YYYY-MM-DD>_<unix_epoch>.csv

  Columns:
    business_id, business_name, entity_type, status, creation_date,
    principal_address, principal_city, principal_zip, county,
    jurisdiction, inactive_date, expiration_date, report_due_date,
    registered_agent_name, registered_agent_address,
    governing_persons, filing_history, phone_number,
    enrichment_status, scraped_at
"#;

const EXPORT_AFTER_HELP: &str = r#"
EXAMPLES

  Export Grant County records to CSV:
    indiana_business_dir export --county "Grant"

  Export to a custom directory:
    indiana_business_dir export --county "Grant" --output-dir ./my_exports

  Use a non-default database:
    indiana_business_dir export --county "Grant" --db ./backups/old_run.db

NOTES

  - This command does not launch a browser.
  - No network requests are made.
  - No CAPTCHA solving is required.
  - Only records for the specified county are exported.
"#;

const LIST_AFTER_HELP: &str = r#"
EXAMPLES

  List all counties:
    indiana_business_dir list

  List counties with ZIP/city counts:
    indiana_business_dir list --counts

NOTES

  The county names printed here are the exact values accepted by
  the --county flag in the scrape and export commands.
"#;
