# Indiana Business Directory Scraper

A Rust-based CLI tool that scrapes business entity records from the Indiana Secretary of State (INBiz) website (`bsd.sos.in.gov/publicbusinesssearch`) on a per-county basis. It discovers all registered businesses, enriches them with detailed SOS information (addresses, registered agents, governing persons), and exports the results to CSV.

## Architecture

- **Rust CLI**: Handles orchestration, SQLite state management, CSV generation, and user interaction.
- **Node.js + Playwright Stealth Browser Driver**: Bypasses Cloudflare anti-bot protections and automates DOM interactions with the Indiana SOS website.
- **JSON-RPC Protocol**: Rust communicates with the browser driver over stdin/stdout.

## Key Design Decisions

1. **Anti-Bot**: The Indiana SOS site sits behind Cloudflare and serves reCAPTCHA challenges on every new search. Direct HTTP requests are blocked. This tool uses Playwright with `puppeteer-extra-plugin-stealth` to present as a real browser.
2. **Minimize CAPTCHA Exposure**: Pagination within a single search result does **not** trigger a new CAPTCHA. The tool therefore paginates exhaustively through each search before moving to the next location.
3. **Resume Capability**: All progress is stored in SQLite (`indiana_business_dir.db`). If the process is interrupted, you can resume exactly where you left off.
4. **Two Search Modes**:
   - **ZIP mode** (default): Searches every ZIP code in the target county. Most thorough but may require more CAPTCHA solves.
   - **City mode**: Searches every incorporated city/town in the target county. Fewer searches for large counties, but may miss rural unincorporated addresses.

## Prerequisites

- **Rust** (1.85+)
- **Node.js** (18+) with `npm`
- **Playwright** (installed automatically via `npm install` in project directory)

## Installation

```bash
cd /path/to/indiana_business_dir
npm install
cargo build --release
```

Or use the Makefile:
```bash
make release
```

## Usage

The CLI is organized into three subcommands: `scrape`, `export`, and `list`.

```bash
# See top-level help
./target/release/indiana_business_dir --help

# See help for a specific subcommand
./target/release/indiana_business_dir scrape --help
./target/release/indiana_business_dir export --help
./target/release/indiana_business_dir list --help
```

### `list` — Show available counties

```bash
./target/release/indiana_business_dir list
./target/release/indiana_business_dir list --counts
```

### `scrape` — Discover and enrich records

```bash
# Scrape Grant County by ZIP code (headful required for CAPTCHA)
./target/release/indiana_business_dir scrape --county "Grant" --headful

# Scrape Marion County using City search mode
./target/release/indiana_business_dir scrape --county "Marion" --headful --search-mode city

# Scrape only Gas City within Grant County
./target/release/indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful

# Write CSV to an explicit path instead of auto-generated location
./target/release/indiana_business_dir scrape --county "Grant" --headful --csv ./grant_businesses.csv

# Test with only the first 2 locations
./target/release/indiana_business_dir scrape --county "Grant" --headful --limit 2

# Resume a previous run
./target/release/indiana_business_dir scrape --county "Grant" --headful --resume

# Skip phone enrichment and only do discovery + SOS detail
./target/release/indiana_business_dir scrape --county "Grant" --headful --skip-tertiary
```

### `export` — Export existing DB records to CSV

```bash
# Export Grant County records
./target/release/indiana_business_dir export --county "Grant"

# Export to a specific CSV file
./target/release/indiana_business_dir export --county "Grant" --csv ./my_grant_export.csv

# Export from a custom database
./target/release/indiana_business_dir export --county "Grant" --db ./backup.db
```

### Important: CAPTCHA Solving

Because the Indiana SOS site serves a **reCAPTCHA challenge on every new search**, you **must** use `--headful` when scraping so a visible browser window opens. When a CAPTCHA appears:

1. A browser window will pop up.
2. The terminal will print: `CAPTCHA detected. Please solve it now.`
3. Solve the CAPTCHA manually in the browser.
4. The scraper will automatically detect the solution and continue.

If you do not use `--headful`, the scraper will time out on the first CAPTCHA.

### Output

By default, CSV files are written to:

```
outputs/<county_name>/<county_name>_<YYYY-MM-DD>_<UNIXEPOCHTIME>.csv
```

You can override this with `--csv <PATH>` on both `scrape` and `export`.

### Wrapper Scripts

A `run.sh` script and `Makefile` are provided for convenience:

```bash
# Build release binary
make release
./run.sh build

# Scrape Gas City
make run-gas-city
./run.sh gas-city

# Resume Gas City
make resume-gas-city
./run.sh resume-gas-city

# Export Grant County
make export-grant
./run.sh export-grant

# List all counties
./run.sh list
```

## CLI Options (Scrape Subcommand)

| Flag | Description |
|------|-------------|
| `--county <NAME>` | **Required.** Target Indiana county. |
| `--city <NAME>` | Filter to a specific city/town within the county. |
| `--output-dir <PATH>` | Base directory for CSV files (default: `outputs/`). Ignored if `--csv` is given. |
| `--csv <PATH>` | Explicit CSV output file path. Overrides auto-generated filename. |
| `--headful` | Run browser in visible mode (required for CAPTCHA). |
| `--search-mode <zip\|city>` | Discovery strategy (default: `zip`). |
| `--resume` | Skip already-complete records and continue. |
| `--skip-primary` | Skip business discovery. |
| `--skip-secondary` | Skip SOS detail enrichment. |
| `--skip-tertiary` | Skip external phone enrichment stub. |
| `--db <PATH>` | SQLite database path (default: `indiana_business_dir.db`). |
| `--page-delay-ms <N>` | Delay between paginations (default: 3000). |
| `--search-delay-ms <N>` | Delay between searches (default: 5000). |
| `--limit <N>` | Only search the first N locations (for testing). |
| `--captcha-timeout <SECONDS>` | Seconds to wait for manual CAPTCHA solve (default: 120). |

## Scraping Tiers

### 1. Primary Scraper — Business Discovery
Searches the SOS database by ZIP code or city name for the target county. For every result page, it extracts:
- Business ID
- Business Name
- Entity Type
- Status
- Principal Office Address
- Registered Agent Name

These are stored in SQLite with `enrichment_status = discovered`.

### 2. Secondary Scraper — SOS Detail Enrichment
Visits each business's detail page and extracts:
- Creation Date
- Principal Office Address (full)
- Jurisdiction of Formation
- Business Status
- Inactive / Expiration Dates
- Business Entity Report Due Date
- Registered Agent Name & Address
- Governing Persons (officers/directors/members)

Updates SQLite with `enrichment_status = enriched`.

### 3. Tertiary Scraper — Phone Enrichment *(Stub)*
The Indiana SOS does **not** publish phone numbers. This tier is provided as an extensible stub. To implement it, you could integrate:
- Google Places API
- Yelp Fusion API
- DuckDuckGo/Yellow Pages scraping

By default, it marks all enriched records as `complete` without adding phone numbers.

## Database Schema

All data is stored in a local SQLite database (`indiana_business_dir.db` by default):

```sql
CREATE TABLE businesses (
    business_id TEXT PRIMARY KEY,
    county TEXT NOT NULL,
    business_name TEXT,
    entity_type TEXT,
    status TEXT,
    creation_date TEXT,
    principal_address TEXT,
    principal_city TEXT,
    principal_zip TEXT,
    jurisdiction TEXT,
    inactive_date TEXT,
    expiration_date TEXT,
    report_due_date TEXT,
    registered_agent_name TEXT,
    registered_agent_address TEXT,
    governing_persons TEXT,
    filing_history TEXT,
    phone_number TEXT,
    enrichment_status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

## Known Limitations

1. **CAPTCHA Frequency**: Indiana SOS serves a CAPTCHA on every new search submission. For a county with many ZIPs/cities, you will need to solve multiple CAPTCHAs. Pagination within a search does not require additional CAPTCHAs.
2. **No Phone Numbers from SOS**: Phone numbers are not part of the public SOS record. External enrichment is required.
3. **No Bulk API**: Indiana sells bulk data through INBiz Bulk Data Services, but there is no free public API for exhaustive enumeration.
4. **Large Counties**: Marion County (Indianapolis) has tens of thousands of businesses. A full scrape may take several hours and require solving 10–40 CAPTCHAs depending on search mode.

## Tips for Large Counties

- Use `--search-mode city` for counties like Marion, Lake, or Allen. This dramatically reduces the number of searches (and CAPTCHAs) compared to ZIP mode.
- Increase `--page-delay-ms` if you notice the site throttling pagination.
- Run with `--limit 1` first to verify CAPTCHA behavior and output format.

## Project Structure

```
indiana_business_dir/
├── Cargo.toml
├── package.json
├── Makefile
├── run.sh
├── data/
│   ├── in_zips.json      # County → ZIP codes (Census data)
│   └── in_cities.json    # County → cities/towns (Census data)
├── scripts/
│   └── browser_driver.js # Node.js Playwright stealth driver
├── src/
│   ├── main.rs           # CLI entry point
│   ├── cli.rs            # clap argument definitions
│   ├── counties.rs       # County data loading
│   ├── db.rs             # SQLite operations
│   ├── models.rs         # Rust data models
│   ├── browser_driver.rs # JSON-RPC client to Node driver
│   ├── output.rs         # CSV export
│   └── scraper/
│       ├── mod.rs        # Orchestrator
│       ├── primary.rs    # Discovery (ZIP/City search)
│       ├── secondary.rs  # SOS detail enrichment
│       └── tertiary.rs   # Phone enrichment stub
└── outputs/              # Generated CSVs
```

## Future Enhancements

- **Automatic CAPTCHA Solving**: Integrate with 2captcha / Anti-Captcha API for fully unattended operation.
- **Phone Enrichment**: Implement DuckDuckGo scraping or Google Places API integration.
- **Governing Persons Parsing**: Extract structured officer/director tables from the detail page.
- **Parallel Detail Scraping**: Add concurrency control for faster secondary enrichment.
