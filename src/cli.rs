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
    /// "grant", "GRANT", and "Grant" are all equivalent. Use the exact
    /// name shown by the `list` command. If the county is not found in
    /// the embedded Census data, the CLI exits with an error before
    /// launching the browser.
    #[arg(long, short, help = "County to scrape (e.g., Grant, Marion, Lake)")]
    pub county: String,

    /// Optional city filter within the selected county.
    ///
    /// When provided, only locations whose name contains this string
    /// are searched. This is useful for narrowing a large county down
    /// to a single municipality (e.g., --city "Gas City" within
    /// Grant County). The filter is case-insensitive and applies
    /// to the location list derived from --search-mode.
    ///
    /// If no locations match, the scraper exits with an error before
    /// any browser interaction occurs.
    #[arg(long, help = "Filter to a specific city/town within the county")]
    pub city: Option<String>,

    /// Base directory for CSV output files.
    ///
    /// Each run creates a subdirectory named after the county
    /// (lowercased, spaces replaced with underscores) and writes
    /// a timestamped CSV inside it:
    ///   <OUTPUT_DIR>/<county>/<county>_<YYYY-MM-DD>_<unix_epoch>.csv
    ///
    /// This option is ignored if --csv is also provided.
    #[arg(
        long,
        short,
        default_value = "outputs",
        help = "Directory for generated CSV files"
    )]
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
    #[arg(
        long,
        help = "Open a visible browser window for manual CAPTCHA solving"
    )]
    pub headful: bool,

    /// Resume a previous run using the SQLite database.
    ///
    /// Records whose enrichment_status is already 'complete' are
    /// skipped. Records in 'discovered' or 'enriched' states are
    /// re-processed starting from the next unhandled tier.
    /// This allows safe interruption and restart without losing
    /// already-paginated discovery results or detail-page data.
    ///
    /// Note: Resume works best when combined with --skip-primary
    /// if you already finished the discovery phase and only need
    /// to finish enrichment.
    #[arg(
        long,
        help = "Skip already-complete records and continue where you left off"
    )]
    pub resume: bool,

    /// Skip the primary scraper (business discovery phase).
    ///
    /// Use this when you already have business IDs in the SQLite
    /// database for the target county and only want to run
    /// enrichment or export. This is useful for resuming a run
    /// that previously completed discovery but was interrupted
    /// during detail-page enrichment.
    #[arg(long, help = "Skip ZIP/city discovery and use existing DB records")]
    pub skip_primary: bool,

    /// Skip the secondary scraper (SOS detail enrichment).
    ///
    /// The secondary phase visits each business's detail page to
    /// extract creation dates, principal addresses, registered
    /// agents, jurisdiction information, governing persons, and
    /// filing history. Skipping this produces a CSV with basic
    /// grid-level data only.
    #[arg(long, help = "Skip visiting individual business detail pages")]
    pub skip_secondary: bool,

    /// Skip the tertiary scraper (phone number enrichment).
    ///
    /// Indiana SOS does not publish phone numbers. The tertiary
    /// phase is currently a stub that marks records 'complete'
    /// without adding phone data. You may implement external
    /// lookups (Google Places, Yelp, etc.) in
    /// src/scraper/tertiary.rs later.
    ///
    /// This flag is useful when you want discovery + detail
    /// enrichment but don't need the final "complete" status
    /// bump, or when you're iterating quickly.
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
    #[arg(
        long,
        default_value = "zip",
        help = "Discovery strategy: zip (exhaustive) or city (fewer CAPTCHAs)"
    )]
    pub search_mode: SearchMode,

    /// Path to the SQLite database file.
    ///
    /// All discovered and enriched records are persisted here.
    /// The schema includes business_id (PRIMARY KEY), county,
    /// enrichment_status, addresses, agent info, governing persons,
    /// filing history, and timestamps. You can inspect it directly
    /// with the sqlite3 CLI.
    #[arg(
        long,
        default_value = "indiana_business_dir.db",
        help = "Path to the SQLite state database"
    )]
    pub db: PathBuf,

    /// Delay in milliseconds between pagination clicks.
    ///
    /// Indiana SOS uses ASP.NET postbacks for pagination.
    /// A short delay reduces the chance of stale-element or
    /// navigation-intercept errors. Default is 3000 ms.
    ///
    /// Increase this value if you see "Pagination ended unexpectedly"
    /// or if pages load slowly.
    #[arg(
        long,
        default_value_t = 3000,
        help = "Wait time between Next-page clicks (default: 3000)"
    )]
    pub page_delay_ms: u64,

    /// Delay in milliseconds between distinct searches.
    ///
    /// After finishing pagination for one ZIP/city, the scraper
    /// pauses this long before submitting the next search.
    /// This gives the browser time to settle and reduces the
    /// chance of being rate-limited by Cloudflare.
    #[arg(
        long,
        default_value_t = 5000,
        help = "Wait time between new ZIP/city searches (default: 5000)"
    )]
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
    #[arg(
        long,
        default_value_t = 120,
        help = "Seconds to wait for manual CAPTCHA solution (default: 120)"
    )]
    pub captcha_timeout: u64,

    /// Explicit CSV output file path.
    ///
    /// If provided, the final CSV is written to this exact path
    /// instead of the auto-generated
    /// outputs/<county>/<county>_<date>_<epoch>.csv location.
    /// Parent directories are created automatically.
    #[arg(
        long,
        short,
        help = "Explicit CSV file path (overrides auto-generated name)"
    )]
    pub csv: Option<PathBuf>,
}

/// Arguments for the `export` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ExportArgs {
    /// County whose records should be exported.
    ///
    /// The name is normalized the same way as in the scrape
    /// command, so "grant" and "Grant" are equivalent.
    #[arg(long, short, help = "County to export (e.g., Grant, Marion)")]
    pub county: String,

    /// Output directory for the CSV file.
    ///
    /// Ignored if --csv is provided. A subdirectory named after
    /// the county is created inside this directory.
    #[arg(
        long,
        short,
        default_value = "outputs",
        help = "Directory for generated CSV"
    )]
    pub output_dir: PathBuf,

    /// Explicit CSV output file path.
    ///
    /// If provided, the CSV is written to this exact path instead
    /// of the auto-generated location under --output-dir.
    /// Parent directories are created automatically.
    #[arg(
        long,
        short,
        help = "Explicit CSV file path (overrides auto-generated name)"
    )]
    pub csv: Option<PathBuf>,

    /// SQLite database path.
    ///
    /// Defaults to indiana_business_dir.db in the current working
    /// directory. Use this to export from a backup or an alternate
    /// run location.
    #[arg(
        long,
        default_value = "indiana_business_dir.db",
        help = "Path to the SQLite state database"
    )]
    pub db: PathBuf,
}

/// Arguments for the `list` subcommand.
#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    /// Also print the number of ZIP codes and cities mapped to each county.
    ///
    /// This can help you decide whether to use --search-mode zip
    /// or --search-mode city for a given county. Counties with
    /// many ZIPs but few cities are good candidates for city mode.
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

QUICK-START FLOW
    1. List counties:
       ./indiana_business_dir list

    2. Scrape a county (headful mode is REQUIRED):
       ./indiana_business_dir scrape --county "Grant" --headful

    3. Export without scraping:
       ./indiana_business_dir export --county "Grant"

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

For full usage examples, CAPTCHA workflow details, and troubleshooting, see README.md.
"#;

const SCRAPE_AFTER_HELP: &str = r#"
EXAMPLES

  Scrape Grant County exhaustively by ZIP code:
    indiana_business_dir scrape --county "Grant" --headful

  Scrape Marion County using City mode (fewer CAPTCHAs):
    indiana_business_dir scrape --county "Marion" --headful --search-mode city

  Scrape only Gas City within Grant County:
    indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful

  Resume an interrupted Gas City run:
    indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful --resume

  Quick dry-run: search only the first ZIP:
    indiana_business_dir scrape --county "Grant" --headful --limit 1

  Skip discovery and only enrich existing records:
    indiana_business_dir scrape --county "Grant" --headful --skip-primary

  Discovery-only run (fast, skips detail + phone enrichment):
    indiana_business_dir scrape --county "Grant" --headful --skip-secondary --skip-tertiary

  Skip phone enrichment stub and just discover + detail:
    indiana_business_dir scrape --county "Grant" --headful --skip-tertiary

  Slow connection? Increase delays:
    indiana_business_dir scrape --county "Grant" --headful --page-delay-ms 5000 --search-delay-ms 8000

CAPTCHA WORKFLOW

  1. Launch with --headful.
  2. A Chromium window opens to https://bsd.sos.in.gov/publicbusinesssearch.
  3. If a reCAPTCHA appears, solve it in the browser window.
  4. The Node.js driver polls the DOM every 2 seconds for the
     g-recaptcha-response token. Once detected, it prints a
     success message and continues automatically.
  5. The scraper paginates through all results for that search.
  6. If another CAPTCHA appears on the next search, repeat step 3–5.

  Pagination within a single search does NOT trigger new CAPTCHAs.
  City mode usually requires fewer CAPTCHAs than ZIP mode.

CAPTCHA TROUBLESHOOTING

  - "CAPTCHA not solved within timeout"
    → Always use --headful. Increase timeout with --captcha-timeout 300.

  - Browser window doesn't appear
    → Make sure your display / X11 / Wayland session is active.
      On macOS this should just work. On Linux over SSH you may
      need X11 forwarding or a local display.

  - The challenge is extremely slow or images don't load
    → Your IP may be partially rate-limited. Wait a minute and
      resume with --resume.

ENRICHMENT TIERS

  Tier 1 (Primary)  — Business Discovery
    Searches by ZIP or city. Extracts:
      business_id, business_name, entity_type, status,
      principal_address, registered_agent_name
    Stores with enrichment_status = discovered.

  Tier 2 (Secondary) — SOS Detail Enrichment
    Visits each detail page. Extracts:
      creation_date, principal_address, principal_city,
      principal_zip, jurisdiction, inactive_date,
      expiration_date, report_due_date,
      registered_agent_name, registered_agent_address,
      governing_persons (JSON), filing_history (JSON)
    Stores with enrichment_status = enriched.

  Tier 3 (Tertiary) — Phone Enrichment (stub)
    Indiana SOS does not publish phone numbers. This tier
    currently marks records as complete without adding phones.
    You can extend src/scraper/tertiary.rs to call external APIs.
    Stores with enrichment_status = complete.

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

  Column glossary:
    business_id            — SOS internal Business ID.
    business_name          — Legal or assumed business name.
    entity_type            — e.g. Domestic LLC, Nonprofit Corporation.
    status                 — Active, Admin Dissolved, Revoked, etc.
    creation_date          — Formation date from the detail page.
    principal_address      — Full principal office address.
    principal_city         — City extracted from principal_address.
    principal_zip          — ZIP code extracted from principal_address.
    county                 — The target county you searched.
    jurisdiction           — Jurisdiction of formation.
    inactive_date          — Inactive date if applicable.
    expiration_date        — Expiration date if applicable.
    report_due_date        — Next business entity report due date.
    registered_agent_name  — Name of the registered agent.
    registered_agent_address — Full address of the registered agent.
    governing_persons      — JSON array of officers/directors/members.
    filing_history         — JSON array of recent filings.
    phone_number           — Empty stub (Indiana SOS has no phone data).
    enrichment_status      — discovered | enriched | complete.
    scraped_at             — ISO 8601 timestamp of last update.
"#;

const EXPORT_AFTER_HELP: &str = r#"
EXAMPLES

  Export Grant County records to CSV:
    indiana_business_dir export --county "Grant"

  Export to a custom directory:
    indiana_business_dir export --county "Grant" --output-dir ./my_exports

  Export to an explicit file path:
    indiana_business_dir export --county "Grant" --csv ./grant_export.csv

  Use a non-default database:
    indiana_business_dir export --county "Grant" --db ./backups/old_run.db

SQLITE TIPS

  You can query the database directly before exporting:

    sqlite3 indiana_business_dir.db \
      "SELECT enrichment_status, COUNT(*) FROM businesses WHERE county = 'Grant' GROUP BY enrichment_status;"

  To export your own CSV directly from SQLite:

    sqlite3 indiana_business_dir.db
    > .headers on
    > .mode csv
    > .out my_export.csv
    > SELECT * FROM businesses WHERE county = 'Grant';

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

COUNTY NAME NORMALIZATION

  The --county argument accepts any casing (e.g., "grant", "GRANT",
  "Grant"). It is normalized to title case before matching against
  the embedded Census data. Names with periods or spaces (e.g.,
  "St. Joseph", "LaPorte") must be passed with quotes on the shell:

    ./indiana_business_dir scrape --county "St. Joseph" --headful

NOTES

  The county names printed here are the exact values accepted by
  the --county flag in the scrape and export commands.
"#;
