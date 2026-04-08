# Architecture Deep Dive

This document explains the internals of `indiana_business_dir`: how the Rust CLI communicates with the Node.js browser driver, how data flows through the system, and how the SQLite schema evolves over time.

## Table of Contents

- [Overview](#overview)
- [JSON-RPC Protocol](#json-rpc-protocol)
- [Browser Driver Methods](#browser-driver-methods)
- [CAPTCHA Polling Loop](#captcha-polling-loop)
- [Primary Scraper Data Flow](#primary-scraper-data-flow)
- [Secondary Scraper Data Flow](#secondary-scraper-data-flow)
- [Grid Link Parameters](#grid-link-parameters)
- [SQLite Schema Migrations](#sqlite-schema-migrations)
- [Why Not Direct HTTP?](#why-not-direct-http)

---

## Overview

```
┌─────────────────┐         stdin/stdout (JSON-RPC)         ┌──────────────────────┐
│   Rust CLI      │  ◄────────────────────────────────────►  │  Node.js Driver      │
│  (orchestrator) │                                          │  (Playwright + Stealth)
└─────────────────┘                                          └──────────────────────┘
         │                                                            │
         │                                                            │
    SQLite DB                                                   Chromium Browser
         │                                                            │
         └──────────────────── CSV Export ◄───────────────────────────┘
```

The Rust side is responsible for:
- Parsing CLI arguments (`clap`).
- Loading county-to-location mappings from embedded JSON.
- Managing SQLite state (inserts, updates, queries).
- Sending commands to the browser driver and processing responses.
- Writing the final CSV.

The Node.js side is responsible for:
- Launching a stealth-enabled Chromium browser.
- Navigating the Indiana SOS ASP.NET site.
- Solving (or waiting for the user to solve) reCAPTCHA challenges.
- Extracting DOM data and returning structured JSON.

## JSON-RPC Protocol

Communication happens over **stdin/stdout** using **newline-delimited JSON**.

### Request Format

```json
{"id": 1, "method": "search_zip", "params": {"zip": "46933"}}
```

### Response Format

```json
{"id": 1, "result": {"status": "ok", "url": "...", "error": null}, "error": null}
```

If an error occurs on the Node side, the response looks like:

```json
{"id": 1, "result": null, "error": "CAPTCHA not solved within timeout"}
```

The Rust `browser_driver.rs` serializes requests with `serde_json`, writes them followed by `\n`, flushes stdin, and then blocks on a single line from stdout. The `id` field is auto-incremented per request to allow future async correlation, though the current implementation is fully synchronous.

## Browser Driver Methods

All methods are implemented in `scripts/browser_driver.js`.

### `launch`

**Request:**
```json
{"id": 1, "method": "launch", "params": {"headless": false}}
```

**Response:**
```json
{"status": "launched", "headless": false}
```

Launches Chromium with `puppeteer-extra-plugin-stealth` and creates a 1920x1080 viewport context.

---

### `navigate_search`

**Request:**
```json
{"id": 2, "method": "navigate_search", "params": null}
```

**Response:**
```json
{"status": "ok", "url": "https://bsd.sos.in.gov/publicbusinesssearch"}
```

Navigates to the Indiana SOS search page and waits for CAPTCHA resolution.

---

### `search_zip`

**Request:**
```json
{"id": 3, "method": "search_zip", "params": {"zip": "46933"}}
```

**Response:**
```json
{"status": "ok", "url": "...", "error": null}
```

Fills in the ZIP code search form, clicks Search, waits for postback, and checks for error dialogs (e.g., "You have entered an invalid search"). The `error` field is `null` on success.

---

### `search_city`

**Request:**
```json
{"id": 4, "method": "search_city", "params": {"city": "Gas City"}}
```

**Response:**
```json
{"status": "ok", "url": "...", "error": null}
```

Same as `search_zip`, but fills the City field. Common Census suffixes like `" city"`, `" town"`, `" CDP"`, `" village"` are stripped automatically before filling the input.

---

### `extract_results`

**Request:**
```json
{"id": 5, "method": "extract_results", "params": null}
```

**Response:**
```json
{
  "rows": [
    {
      "business_id_display": "2008050100426",
      "business_name": ""THE HUB"",
      "name_type": "ASSUMED BUSINESS NAME",
      "entity_type": "Domestic Limited Liability Company",
      "principal_address": "331 MASSACHUSETTS AVE., INDIANAPOLIS, IN, 46204, USA",
      "registered_agent_name": "TREVOR J BELDEN",
      "status": "Active",
      "detail_business_id": "953250",
      "detail_business_type": "Domestic Limited Liability Company",
      "detail_is_series": "False",
      "detail_link_id": "ui+Y6g6aPyU="
    }
  ]
}
```

Parses `table#grid_businessList` row by row. The `detail_business_id` is extracted from the `businessid` attribute of the `onclick="BusinessInformation(this)"` link. This internal ID is what the SOS site uses for detail-page navigation, and it can differ from the human-readable display ID in `business_id_display`.

---

### `get_pagination_info`

**Request:**
```json
{"id": 6, "method": "get_pagination_info", "params": null}
```

**Response:**
```json
{
  "text": "Page 1 of 5, records 1 to 10 of 48",
  "current_page": 1,
  "total_pages": 5,
  "record_start": 1,
  "record_end": 10,
  "total_records": 48
}
```

Scrapes the `.borderFooter` text and parses the `Page X of Y, records A to B of C` pattern.

---

### `click_next`

**Request:**
```json
{"id": 7, "method": "click_next", "params": null}
```

**Response:**
```json
{"clicked": true}
```

Finds the "Next" pagination link, verifies it isn't `disabled`, clicks it, and waits for the ASP.NET postback to settle.

---

### `get_detail`

**Request:**
```json
{
  "id": 8,
  "method": "get_detail",
  "params": {
    "business_id": "953250",
    "business_type": "Domestic Limited Liability Company",
    "is_series": "False"
  }
}
```

**Response:**
```json
{
  "url": "https://bsd.sos.in.gov/PublicBusinessSearch/BusinessInformation?businessId=953250&businessType=Domestic%20Limited%20Liability%20Company&isSeries=False",
  "kvs": {
    "business name": ""THE HUB"",
    "entity type": "Domestic Limited Liability Company",
    "business status": "Active",
    "creation date": "08/01/2008",
    "principal office address": "331 MASSACHUSETTS AVE., INDIANAPOLIS, IN, 46204, USA",
    "jurisdiction of formation": "Indiana",
    "registered agent name": "TREVOR J BELDEN",
    "registered agent address": "331 MASSACHUSETTS AVE., INDIANAPOLIS, IN, 46204, USA"
  },
  "sections": [
    {
      "heading": "Business Details",
      "kvs": { ... },
      "tables": []
    },
    {
      "heading": "Governing Person Information",
      "kvs": {},
      "tables": [
        {
          "headers": ["title", "name", "address"],
          "rows": [
            {"title": "Manager", "name": "Trevor Belden", "address": "331 Massachusetts Ave, Indianapolis, IN 46204"}
          ]
        }
      ]
    }
  ]
}
```

This is the most complex method. It:
1. Builds the correct detail-page URL with `businessId`, `businessType`, and `isSeries` query parameters.
2. Navigates to the page.
3. Waits for and verifies CAPTCHA resolution.
4. Walks the DOM, grouping tables under their preceding heading.
5. Distinguishes **key-value tables** (2 columns, first cell ends with `:`) from **data tables** (multi-column grids like Governing Persons or Filing History).
6. Returns both a flat `kvs` map and a structured `sections` array.

**Why the URL parameters matter:**
The Indiana SOS detail page does **not** work with `?id=XXX` alone. It requires `?businessId=XXX&businessType=YYY&isSeries=ZZZ`. These three values are scraped from the grid result link's HTML attributes during `extract_results`.

---

### `close`

**Request:**
```json
{"id": 9, "method": "close", "params": null}
```

**Response:**
```json
{"status": "closed"}
```

Closes the browser and cleans up resources.

## CAPTCHA Polling Loop

```javascript
async function waitForCaptcha(maxSeconds) {
  const limit = maxSeconds || config.captchaTimeout || 120;
  for (let i = 0; i < limit / 2; i++) {
    const hasCaptcha = await page.locator('iframe[src*="recaptcha"], .g-recaptcha').count() > 0;
    if (!hasCaptcha) {
      return { solved: true, waited: (i + 1) * 2 };
    }
    const token = await page.evaluate(() => {
      const el = document.querySelector('#g-recaptcha-response');
      return el ? el.value : '';
    });
    if (token && token.length > 10) {
      return { solved: true, waited: (i + 1) * 2 };
    }
    await page.waitForTimeout(2000);
  }
  return { solved: false, waited: limit };
}
```

The loop runs every 2 seconds. It detects a solved CAPTCHA in two ways:
1. The reCAPTCHA iframe/widget disappears from the DOM.
2. The `#g-recaptcha-response` textarea contains a non-empty token.

Either condition causes an immediate return. If the timeout expires, the method returns `solved: false`, which propagates back to Rust as an error.

## Primary Scraper Data Flow

1. Load `data/in_zips.json` or `data/in_cities.json` for the target county.
2. If `--city` is provided, filter the location list to matching entries.
3. For each location:
   - Send `search_zip` or `search_city`.
   - Loop:
     - Send `extract_results`.
     - For each row, call `db.insert_discovered(...)`.
     - Send `get_pagination_info`.
     - If `current_page >= total_pages`, break.
     - Send `click_next` and sleep for `--page-delay-ms`.
   - Sleep for `--search-delay-ms` before the next location.

The `insert_discovered` function uses an **UPSERT**:

```sql
INSERT INTO businesses (...) VALUES (...)
ON CONFLICT(business_id) DO UPDATE SET
    business_name = COALESCE(business_name, excluded.business_name),
    entity_type = COALESCE(entity_type, excluded.entity_type),
    ...
WHERE enrichment_status != 'complete'
```

This ensures that:
- Re-running primary discovery on the same county doesn't create duplicate rows.
- Existing records keep their enriched data.
- Completed records are left untouched.

## Secondary Scraper Data Flow

1. Query SQLite for all records in the target county where `enrichment_status = 'discovered'`.
2. For each `business_id`:
   - Look up the stored `detail_business_type` and `detail_is_series` from the primary phase.
   - Send `get_detail(business_id, business_type, is_series)`.
   - Parse the JSON response into a `BusinessRecord`.
   - Check `has_meaningful_data(&record)` — if the detail page returned nothing (e.g., wrong URL params), skip the update to avoid wiping discovered data.
   - Call `db.update_enriched(&record)`.

The `update_enriched` method performs field-level updates:

```rust
for (col, val) in fields {
    if let Some(v) = val {
        conn.execute("UPDATE businesses SET col = ?1 WHERE business_id = ?2", params![v, id])?;
    }
}
conn.execute("UPDATE businesses SET enrichment_status = 'enriched', updated_at = ?1 WHERE business_id = ?2", ...)?;
```

This guarantees that a partially successful detail parse (e.g., only business name and status found) doesn't overwrite an already-populated principal address from the primary grid.

## Grid Link Parameters

The Indiana SOS search result grid contains links like this:

```html
<a href="#"
   businessid="953250"
   businesstype="Domestic Limited Liability Company"
   isseries="False"
   id="ui+Y6g6aPyU="
   onclick="BusinessInformation(this)">
   2008050100426
</a>
```

When the user (or our scraper) clicks this link, the page calls:

```javascript
function BusinessInformation(obj) {
    var BusinessID = obj.attributes.businessid.value;
    var BusinessType = obj.attributes.businesstype.value;
    var IsSeries = obj.attributes.isseries.value;
    // submits POST to /PublicBusinessSearch/BusinessInformationFromIndex
}
```

Our detail-page scraper bypasses the POST form submission by directly constructing the equivalent GET URL:

```
https://bsd.sos.in.gov/PublicBusinessSearch/BusinessInformation?businessId=953250&businessType=Domestic%20Limited%20Liability%20Company&isSeries=False
```

Without all three query parameters, the server either returns a generic error page or a CAPTCHA loop with no actual business data.

## SQLite Schema Migrations

Because this tool uses a local SQLite file that persists across versions, new columns must be added safely.

In `db.rs::init()`:

```rust
let cols: Vec<String> = self.conn.prepare("PRAGMA table_info(businesses)")?
    .query_map([], |row| row.get::<_, String>(1))?
    .collect::<Result<Vec<_>, _>>()?;

if !cols.contains(&"detail_business_type".to_string()) {
    self.conn.execute("ALTER TABLE businesses ADD COLUMN detail_business_type TEXT", [])?;
}
if !cols.contains(&"detail_is_series".to_string()) {
    self.conn.execute("ALTER TABLE businesses ADD COLUMN detail_is_series TEXT", [])?;
}
```

This pattern:
1. Lists existing columns via `PRAGMA table_info`.
2. Conditionally runs `ALTER TABLE ADD COLUMN` only if the column is missing.
3. Allows old databases to be opened by newer binaries without manual migration.

## Why Not Direct HTTP?

You might wonder why we don't just reverse-engineer the ASP.NET POST endpoints and call them with `reqwest`.

**Reasons:**
1. **Cloudflare JS Challenge**: The initial TLS handshake is followed by a JavaScript challenge that evaluates browser fingerprints. `reqwest` cannot execute this JavaScript.
2. **reCAPTCHA v2**: Every new search requires a valid `g-recaptcha-response` token tied to the current browser session and domain. This token can only be obtained inside a real browser context.
3. **ViewState / RequestVerificationToken**: The ASP.NET forms carry anti-CSRF tokens and ViewState that are session-bound. While these *could* be extracted and replayed, the reCAPTCHA token makes it impossible to automate without a browser.
4. **Dynamic DOM**: Pagination and detail navigation rely on client-side JavaScript (`XHtmlGrid`, `BusinessInformation`) that submits dynamically constructed forms. Replaying these precisely in raw HTTP is brittle and error-prone.

Playwright solves all of these problems by operating a real browser that executes the site's own JavaScript, handles cookies automatically, and allows the user to solve CAPTCHAs in situ.
