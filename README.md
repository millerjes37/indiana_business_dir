# Indiana Business Directory Scraper

[![CI](https://github.com/millerjes37/indiana_business_dir/actions/workflows/ci.yml/badge.svg)](https://github.com/millerjes37/indiana_business_dir/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/millerjes37/indiana_business_dir?include_prereleases)](https://github.com/millerjes37/indiana_business_dir/releases)

A Rust-based CLI tool that scrapes business entity records from the Indiana Secretary of State (INBiz) website (`bsd.sos.in.gov/publicbusinesssearch`) on a per-county basis. It discovers all registered businesses, enriches them with detailed SOS information (addresses, registered agents, governing persons, filing history), and exports the results to CSV.

**[Download Prebuilt Binaries →](https://github.com/millerjes37/indiana_business_dir/releases)**

## Table of Contents

- [Architecture](#architecture)
- [Why Playwright?](#why-playwright)
- [Key Design Decisions](#key-design-decisions)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Usage](#usage)
  - [`list`](#list--show-available-counties)
  - [`scrape`](#scrape--discover-and-enrich-records)
  - [`export`](#export--export-existing-db-records-to-csv)
- [Data Flow](#data-flow)
- [CAPTCHA Workflow](#captcha-workflow)
- [Output](#output)
- [County Recommendations](#county-recommendations)
- [SQLite Cookbook](#sqlite-cookbook)
- [Troubleshooting](#troubleshooting)
- [Scraping Tiers](#scraping-tiers)
- [Database Schema](#database-schema)
- [Known Limitations](#known-limitations)
- [Project Structure](#project-structure)
- [Future Enhancements](#future-enhancements)

---

## Architecture

- **Rust CLI**: Handles orchestration, SQLite state management, CSV generation, and user interaction.
- **Node.js + Playwright Stealth Browser Driver**: Bypasses Cloudflare anti-bot protections and automates DOM interactions with the Indiana SOS website.
- **JSON-RPC Protocol**: Rust communicates with the browser driver over stdin/stdout using newline-delimited JSON messages.

The browser driver (`scripts/browser_driver.js`) launches a Chromium instance via Playwright with `puppeteer-extra-plugin-stealth`. It exposes methods like `search_zip`, `extract_results`, `click_next`, and `get_detail`. The Rust side sends JSON-RPC requests and receives structured JSON responses, allowing the scraper to navigate the ASP.NET postback-driven site without direct HTTP requests.

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for a deep dive into the JSON-RPC protocol, CAPTCHA polling loop, and schema migration strategy.

## Why Playwright?

The Indiana SOS site sits behind **Cloudflare** and serves **Google reCAPTCHA** challenges on every new search submission. Direct HTTP requests (even with rotating user agents) are blocked at the TLS/JS-challenge layer. The CAPTCHA also requires a valid browser environment with consistent fingerprints, cookies, and execution context.

By using Playwright with `puppeteer-extra-plugin-stealth`, the tool:
- Presents as a genuine Chrome browser (viewport, WebGL, fonts, plugins).
- Executes the site's ASP.NET postbacks and pagination handlers in a real DOM.
- Allows the user to solve reCAPTCHA manually in a visible browser window (`--headful`).

## Key Design Decisions

1. **Anti-Bot**: The Indiana SOS site serves reCAPTCHA challenges on every new search. Direct HTTP requests are blocked. This tool uses Playwright with `puppeteer-extra-plugin-stealth` to present as a real browser.
2. **Minimize CAPTCHA Exposure**: Pagination within a single search result does **not** trigger a new CAPTCHA. The tool therefore paginates exhaustively through each search before moving to the next location.
3. **Resume Capability**: All progress is stored in SQLite (`indiana_business_dir.db`). If the process is interrupted, you can resume exactly where you left off using `--resume`.
4. **Two Search Modes**:
   - **ZIP mode** (default): Searches every ZIP code in the target county. Most thorough but may require more CAPTCHA solves.
   - **City mode**: Searches every incorporated city/town in the target county. Fewer searches for large counties, but may miss rural unincorporated addresses.
5. **Upsert Safety**: When re-processing a business ID, the primary scraper uses SQLite `ON CONFLICT DO UPDATE` with `COALESCE` to preserve any enriched fields already fetched.

## Prerequisites

- **Node.js** (18+) with `npm` *(required at runtime for the Playwright browser driver)*
- **Rust** (1.85+) *(only if building from source)*
- **Playwright** (installed automatically via `npm install` in project directory)

## Installation

### Quick Install (One-Liner)

We provide auto-install scripts that detect your platform, download the correct prebuilt binary, install Node.js dependencies, and place the binary on your PATH.

#### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.sh | bash
```

You can customize the install location:

```bash
curl -fsSL https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.sh | INSTALL_DIR=$HOME/bin bash
```

#### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.ps1 | iex
```

You can customize the install location:

```powershell
$env:INSTALL_DIR = "$env:USERPROFILE\bin"; irm https://raw.githubusercontent.com/millerjes37/indiana_business_dir/main/install.ps1 | iex
```

### Manual Download

If you prefer to download and extract the archive yourself, prebuilt binaries are available for **macOS (Intel & Apple Silicon)**, **Linux (x86_64)**, and **Windows (x86_64)** on the [Releases page](https://github.com/millerjes37/indiana_business_dir/releases).

Each release archive includes the binary, the Node.js browser driver (`scripts/browser_driver.js`), embedded county data, and `package.json` / `package-lock.json` so you can install Node dependencies.

#### macOS (Apple Silicon)

```bash
curl -LO https://github.com/millerjes37/indiana_business_dir/releases/latest/download/indiana_business_dir-v$(curl -s https://api.github.com/repos/millerjes37/indiana_business_dir/releases/latest | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')-aarch64-apple-darwin.tar.gz
tar xzf indiana_business_dir-v*-aarch64-apple-darwin.tar.gz
cd indiana_business_dir
npm install
./indiana_business_dir --help
```

#### macOS (Intel)

```bash
curl -LO https://github.com/millerjes37/indiana_business_dir/releases/latest/download/indiana_business_dir-v$(curl -s https://api.github.com/repos/millerjes37/indiana_business_dir/releases/latest | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')-x86_64-apple-darwin.tar.gz
tar xzf indiana_business_dir-v*-x86_64-apple-darwin.tar.gz
cd indiana_business_dir
npm install
./indiana_business_dir --help
```

#### Linux (x86_64)

```bash
curl -LO https://github.com/millerjes37/indiana_business_dir/releases/latest/download/indiana_business_dir-v$(curl -s https://api.github.com/repos/millerjes37/indiana_business_dir/releases/latest | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')-x86_64-unknown-linux-gnu.tar.gz
tar xzf indiana_business_dir-v*-x86_64-unknown-linux-gnu.tar.gz
cd indiana_business_dir
npm install
./indiana_business_dir --help
```

#### Windows (x86_64)

```powershell
# Download the latest release zip (replace vX.Y.Z with the actual version)
Invoke-WebRequest -Uri "https://github.com/millerjes37/indiana_business_dir/releases/download/vX.Y.Z/indiana_business_dir-vX.Y.Z-x86_64-pc-windows-msvc.zip" -OutFile "indiana_business_dir.zip"
Expand-Archive -Path "indiana_business_dir.zip" -DestinationPath "."
cd indiana_business_dir
npm install
.\indiana_business_dir.exe --help
```

> **Note:** The binary expects `scripts/browser_driver.js` and `data/` to exist in the **current working directory**. Always run it from the extracted folder (or ensure those directories are present in your working directory).

### Build from Source

```bash
cd /path/to/indiana_business_dir
npm install
cargo build --release
```

Or use the Makefile:
```bash
make release
```

## Quick Start

```bash
# 1. List available counties
./indiana_business_dir list

# 2. Scrape a small county (headful required)
./indiana_business_dir scrape --county "Grant" --headful

# 3. Export without scraping
./indiana_business_dir export --county "Grant"
```

## Usage

The CLI is organized into three subcommands: `scrape`, `export`, and `list`.

```bash
# See top-level help
./indiana_business_dir --help

# See help for a specific subcommand
./indiana_business_dir scrape --help
./indiana_business_dir export --help
./indiana_business_dir list --help
```

### `list` — Show available counties

```bash
./indiana_business_dir list
./indiana_business_dir list --counts
```

### `scrape` — Discover and enrich records

```bash
# Scrape Grant County by ZIP code (headful required for CAPTCHA)
./indiana_business_dir scrape --county "Grant" --headful

# Scrape Marion County using City search mode
./indiana_business_dir scrape --county "Marion" --headful --search-mode city

# Scrape only Gas City within Grant County
./indiana_business_dir scrape --county "Grant" --city "Gas City" --search-mode city --headful

# Write CSV to an explicit path instead of auto-generated location
./indiana_business_dir scrape --county "Grant" --headful --csv ./grant_businesses.csv

# Test with only the first 2 locations
./indiana_business_dir scrape --county "Grant" --headful --limit 2

# Resume a previous run
./indiana_business_dir scrape --county "Grant" --headful --resume

# Fast discovery-only run (skip detail and phone enrichment)
./indiana_business_dir scrape --county "Grant" --headful --skip-secondary --skip-tertiary

# Skip phone enrichment and only do discovery + SOS detail
./indiana_business_dir scrape --county "Grant" --headful --skip-tertiary
```

### `export` — Export existing DB records to CSV

```bash
# Export Grant County records
./indiana_business_dir export --county "Grant"

# Export to a specific CSV file
./indiana_business_dir export --county "Grant" --csv ./my_grant_export.csv

# Export from a custom database
./indiana_business_dir export --county "Grant" --db ./backup.db
```

## Data Flow

```
ZIP / City
    |
    v
[ Indiana SOS Search ]
    |
    v
[ Grid Results ]  ----extract---->  SQLite (enrichment_status = discovered)
    |                                    |
    |<---- pagination (no CAPTCHA) ------|
    |
    v
[ Business Detail Page ]  ----enrich-->  SQLite (enrichment_status = enriched)
    |
    v
[ Tertiary Enrichment ]  ----complete-->  SQLite (enrichment_status = complete)
    |
    v
[ CSV Export ]
```

1. **Discovery**: For each ZIP or city, submit a search, paginate through all results, and insert each business into SQLite.
2. **Enrichment**: Visit each business detail page to extract creation dates, full addresses, registered agents, governing persons, and filing history.
3. **Completion**: Mark records as complete (tertiary phone enrichment is currently a stub).
4. **Export**: Write all records for the county to CSV.

## CAPTCHA Workflow

Because the Indiana SOS site serves a **reCAPTCHA challenge on every new search**, you **must** use `--headful` when scraping so a visible browser window opens.

**What happens:**
1. Launch with `--headful`.
2. A Chromium window opens to `https://bsd.sos.in.gov/publicbusinesssearch`.
3. If a reCAPTCHA appears, solve it in the browser window.
4. The Node.js driver polls the DOM every 2 seconds for the `g-recaptcha-response` token.
5. Once detected, the terminal prints a success message and the scraper continues automatically.
6. The scraper paginates through all results for that search.
7. If another CAPTCHA appears on the next search, repeat steps 3–5.

**Important:** Pagination within a single search does **NOT** trigger new CAPTCHAs. City mode usually requires fewer CAPTCHAs than ZIP mode.

## Output

By default, CSV files are written to:

```
outputs/<county_name>/<county_name>_<YYYY-MM-DD>_<UNIXEPOCHTIME>.csv
```

You can override this with `--csv <PATH>` on both `scrape` and `export`.

**CSV Columns:**

| Column | Source | Description |
|--------|--------|-------------|
| `business_id` | Primary | SOS Business ID (e.g., `2008050100426`). |
| `business_name` | Primary/Secondary | Legal or assumed business name. |
| `entity_type` | Primary/Secondary | e.g., Domestic LLC, Nonprofit Corp. |
| `status` | Primary/Secondary | Active, Admin Dissolved, Revoked, etc. |
| `creation_date` | Secondary | Date the entity was formed. |
| `principal_address` | Primary/Secondary | Full principal office address. |
| `principal_city` | Secondary | City extracted from principal address. |
| `principal_zip` | Secondary | ZIP code extracted from principal address. |
| `county` | Primary | Target county for the search. |
| `jurisdiction` | Secondary | Jurisdiction of formation (e.g., Indiana, Delaware). |
| `inactive_date` | Secondary | Date the entity became inactive, if applicable. |
| `expiration_date` | Secondary | Entity expiration date, if applicable. |
| `report_due_date` | Secondary | Next business entity report due date. |
| `registered_agent_name` | Primary/Secondary | Name of the registered agent. |
| `registered_agent_address` | Secondary | Full address of the registered agent. |
| `governing_persons` | Secondary | JSON array of officers/directors/members. |
| `filing_history` | Secondary | JSON array of recent filings. |
| `phone_number` | Tertiary | Phone number (currently empty stub). |
| `enrichment_status` | Internal | `discovered`, `enriched`, or `complete`. |
| `scraped_at` | Internal | ISO 8601 timestamp of the last update. |

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

## County Recommendations

| County | Size | Recommended Mode | Est. Searches | Notes |
|--------|------|------------------|---------------|-------|
| Adams | Small | ZIP | ~20 ZIPs | Quick scrape. |
| Allen | Large | City | ~20 cities | ZIP mode would require ~60 CAPTCHAs. |
| Boone | Medium | ZIP | ~10 ZIPs | Manageable either way. |
| Elkhart | Medium-Large | City | ~15 cities | Use city mode for speed. |
| Grant | Medium | City | ~10 cities | Good test county. |
| Hamilton | Large | City | ~15 cities | Very active business county. |
| Lake | Large | City | ~25 cities | ZIP mode would be extremely slow. |
| Marion | Very Large | City | ~25 cities | Indianapolis metro; expect 10–40 CAPTCHAs in ZIP mode. |
| St. Joseph | Medium-Large | City | ~15 cities | South Bend metro. |
| Tippecanoe | Medium | City | ~10 cities | Lafayette / West Lafayette. |
| Vigo | Medium | City | ~10 cities | Terre Haute area. |

**Rule of thumb:**
- Counties with <15 ZIPs: ZIP mode is fine.
- Counties with 15+ ZIPs: City mode saves significant CAPTCHA time.
- If you need rural unincorporated areas: ZIP mode is more exhaustive.

## SQLite Cookbook

You can inspect and manipulate the local SQLite database directly:

```bash
sqlite3 indiana_business_dir.db
```

### Count records by enrichment status

```sql
SELECT enrichment_status, COUNT(*) FROM businesses WHERE county = 'Grant' GROUP BY enrichment_status;
```

### Find records stuck in "discovered" (haven't been enriched yet)

```sql
SELECT business_id, business_name, principal_address
FROM businesses
WHERE county = 'Grant' AND enrichment_status = 'discovered'
ORDER BY business_id
LIMIT 10;
```

### Inspect governing persons JSON

```sql
SELECT business_id, business_name, governing_persons
FROM businesses
WHERE county = 'Grant' AND governing_persons IS NOT NULL
LIMIT 5;
```

### Find businesses in a specific city (based on principal address)

```sql
SELECT business_id, business_name, principal_address
FROM businesses
WHERE county = 'Grant'
  AND principal_address LIKE '%Gas City%'
ORDER BY business_name;
```

### Manually mark a record complete (skip enrichment)

```sql
UPDATE businesses
SET enrichment_status = 'complete', updated_at = strftime('%s', 'now')
WHERE business_id = '1234567';
```

### Export your own CSV directly from SQLite

```sql
.headers on
.mode csv
.out my_export.csv
SELECT * FROM businesses WHERE county = 'Grant';
```

## Troubleshooting

### "CAPTCHA not solved within timeout"

- **Cause**: You didn't use `--headful`, or the CAPTCHA challenge was unusually difficult.
- **Fix**: Always use `--headful`. Increase the timeout with `--captcha-timeout 300` (5 minutes).

### "No locations found for county: X"

- **Cause**: The county name doesn't match the embedded Census data.
- **Fix**: Run `./indiana_business_dir list` to see exact accepted names. Try the normalized title-case version (e.g., "St. Joseph" not "St Joseph").

### "City 'X' not found in Y"

- **Cause**: The city filter string doesn't match any city in the county's city list.
- **Fix**: Use a substring that appears in the city name. The filter is case-insensitive but must match a contiguous substring.

### Empty CSV except for headers

- **Cause**: The county hasn't been scraped yet, or all records were filtered out.
- **Fix**: Run the `scrape` command first. Check the DB with the SQLite queries above.

### "Pagination ended unexpectedly"

- **Cause**: The site returned an empty page or the "Next" link became stale/stale-element.
- **Fix**: Increase `--page-delay-ms` to 5000 or 8000 to give ASP.NET postbacks more time to settle.

### Slow pagination or hanging

- **Cause**: Rate-limiting by Cloudflare or a slow network connection.
- **Fix**: Increase both `--page-delay-ms` and `--search-delay-ms`. If the site is completely unresponsive, wait a few minutes and resume with `--resume --skip-primary`.

### Records are "discovered" but never "enriched"

- **Cause**: The secondary scraper couldn't reach detail pages, or the detail page returned no data.
- **Fix**: This usually happens when `detail_business_type` / `detail_is_series` were missing from the grid link. Make sure you're on the latest version. Re-run primary discovery to backfill those parameters.

### The binary path changed / `cargo build --release` doesn't create `./target/release/indiana_business_dir`

- **Cause**: This project uses `CARGO_TARGET_DIR=/Users/jacksonmiller/.cargo/target`.
- **Fix**: Use `make release` or `./run.sh build`, which handle the symlink automatically.

### "`node: command not found`" or "Failed to spawn node browser_driver.js"

- **Cause**: The Rust binary spawns `node scripts/browser_driver.js` at runtime, but Node.js is not installed or not in your PATH.
- **Fix**: Install Node.js 18+ and ensure `node --version` works in your terminal. Then run `npm install` inside the project directory so that `playwright` and `puppeteer-extra-plugin-stealth` are available.

## Scraping Tiers

### 1. Primary Scraper — Business Discovery
Searches the SOS database by ZIP code or city name for the target county. For every result page, it extracts:
- Business ID
- Business Name
- Entity Type
- Status
- Principal Office Address
- Registered Agent Name
- Detail link parameters (`detail_business_type`, `detail_is_series`)

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
- Governing Persons (officers/directors/members) as JSON
- Filing History as JSON

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
    detail_business_type TEXT,
    detail_is_series TEXT,
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

## Project Structure

```
indiana_business_dir/
├── Cargo.toml
├── package.json
├── Makefile
├── run.sh
├── README.md
├── ARCHITECTURE.md
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
- **Parallel Detail Scraping**: Add concurrency control for faster secondary enrichment.
