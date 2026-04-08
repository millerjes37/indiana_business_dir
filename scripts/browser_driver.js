const { chromium } = require('playwright-extra');
const stealth = require('puppeteer-extra-plugin-stealth')();

chromium.use(stealth);

let browser = null;
let context = null;
let page = null;
let config = { headless: true, captchaTimeout: 120 };

const SEARCH_URL = 'https://bsd.sos.in.gov/publicbusinesssearch';

function sendResponse(id, result, error) {
  const msg = JSON.stringify({ id, result, error });
  process.stdout.write(msg + '\n');
}

async function waitForCaptcha(maxSeconds) {
  const limit = maxSeconds || config.captchaTimeout || 120;
  for (let i = 0; i < limit / 2; i++) {
    const hasCaptcha = await page.locator('iframe[src*="recaptcha"], .g-recaptcha').count() > 0;
    if (!hasCaptcha) {
      // Also check token exists if it was there
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

async function ensureSearchPage() {
  await page.goto(SEARCH_URL, { waitUntil: 'networkidle', timeout: 60000 });
  await page.waitForTimeout(1500);
  const captcha = await waitForCaptcha(120);
  if (!captcha.solved) {
    throw new Error('CAPTCHA not solved within timeout');
  }
}

async function handleCommand(cmd) {
  const { id, method, params } = cmd;

  try {
    switch (method) {
      case 'launch': {
        config = { headless: params.headless !== false, captchaTimeout: params.captchaTimeout || 120, ...params };
        browser = await chromium.launch({
          headless: config.headless,
          args: ['--disable-blink-features=AutomationControlled']
        });
        context = await browser.newContext({
          viewport: { width: 1920, height: 1080 },
          locale: 'en-US',
          timezoneId: 'America/Indianapolis',
          userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36'
        });
        page = await context.newPage();
        sendResponse(id, { status: 'launched', headless: config.headless });
        break;
      }

      case 'navigate_search': {
        await ensureSearchPage();
        sendResponse(id, { status: 'ok', url: page.url() });
        break;
      }

      case 'search_zip': {
        await ensureSearchPage();
        await page.locator('#rdContains').check();
        await page.locator('#rdBusinessName').check();
        await page.locator('#txtZipCode').fill(String(params.zip));
        await page.locator('#btnSearch').click({ force: true });
        await page.waitForTimeout(3000);
        const error = await page.locator('#errorDialog').textContent().catch(() => '');
        sendResponse(id, { status: 'ok', url: page.url(), error: error.trim() || null });
        break;
      }

      case 'search_city': {
        await ensureSearchPage();
        await page.locator('#rdContains').check();
        await page.locator('#rdBusinessName').check();
        // Strip common Census suffixes like " city", " town", " CDP", " village"
        let city = String(params.city).replace(/\s+(city|town|cdp|village)$/i, '');
        await page.locator('#txtCity').fill(city);
        await page.locator('#btnSearch').click({ force: true });
        await page.waitForTimeout(3000);
        const error = await page.locator('#errorDialog').textContent().catch(() => '');
        sendResponse(id, { status: 'ok', url: page.url(), error: error.trim() || null });
        break;
      }

      case 'search_name_city': {
        await ensureSearchPage();
        await page.locator('#rdContains').check();
        await page.locator('#rdBusinessName').check();
        await page.locator('#txtBusinessName').fill(String(params.name));
        await page.locator('#txtCity').fill(String(params.city));
        await page.locator('#btnSearch').click({ force: true });
        await page.waitForTimeout(3000);
        const error = await page.locator('#errorDialog').textContent().catch(() => '');
        sendResponse(id, { status: 'ok', url: page.url(), error: error.trim() || null });
        break;
      }

      case 'extract_results': {
        const tables = await page.locator('table#grid_businessList').evaluateAll(tbs => {
          if (tbs.length === 0) return { rows: [] };
          const tb = tbs[0];
          const rows = [];
          for (let i = 1; i < tb.rows.length; i++) {
            const cells = Array.from(tb.rows[i].querySelectorAll('td')).map(td => td.innerText.trim());
            const link = tb.rows[i].querySelector('a[onclick*="BusinessInformation"]');
            rows.push({
              business_id_display: cells[0] || '',
              business_name: cells[1] || '',
              name_type: cells[2] || '',
              entity_type: cells[3] || '',
              principal_address: cells[4] || '',
              registered_agent_name: cells[5] || '',
              status: cells[6] || '',
              detail_business_id: link ? link.getAttribute('businessid') : null,
              detail_business_type: link ? link.getAttribute('businesstype') : null,
              detail_is_series: link ? link.getAttribute('isseries') : null,
              detail_link_id: link ? link.getAttribute('id') : null
            });
          }
          return { rows };
        });
        sendResponse(id, tables);
        break;
      }

      case 'get_pagination_info': {
        const footerText = await page.locator('.borderFooter').textContent().catch(() => '');
        const match = footerText.match(/Page\s+(\d+)\s+of\s+([\d,]+),\s+records\s+([\d,]+)\s+to\s+([\d,]+)\s+of\s+([\d,]+)/i);
        sendResponse(id, {
          text: footerText.trim(),
          current_page: match ? parseInt(match[1].replace(/,/g, ''), 10) : null,
          total_pages: match ? parseInt(match[2].replace(/,/g, ''), 10) : null,
          record_start: match ? parseInt(match[3].replace(/,/g, ''), 10) : null,
          record_end: match ? parseInt(match[4].replace(/,/g, ''), 10) : null,
          total_records: match ? parseInt(match[5].replace(/,/g, ''), 10) : null
        });
        break;
      }

      case 'click_next': {
        const nextLink = page.locator('a:has-text("Next >"), a:has-text("Next")').first();
        if (await nextLink.count() === 0) {
          sendResponse(id, { clicked: false, reason: 'no_next_link' });
          break;
        }
        const isDisabled = await nextLink.evaluate(el =>
          el.classList.contains('disabled') || el.getAttribute('aria-disabled') === 'true'
        );
        if (isDisabled) {
          sendResponse(id, { clicked: false, reason: 'disabled' });
          break;
        }
        await nextLink.click();
        await page.waitForTimeout(3000);
        sendResponse(id, { clicked: true });
        break;
      }

      case 'get_detail': {
        const detailUrl = new URL('https://bsd.sos.in.gov/PublicBusinessSearch/BusinessInformation');
        detailUrl.searchParams.set('businessId', String(params.business_id));
        if (params.business_type) {
          detailUrl.searchParams.set('businessType', String(params.business_type));
        }
        if (params.is_series !== undefined && params.is_series !== null) {
          detailUrl.searchParams.set('isSeries', String(params.is_series));
        }
        await page.goto(detailUrl.toString(), { waitUntil: 'networkidle', timeout: 60000 });
        await page.waitForTimeout(2500);

        // Check if we hit CAPTCHA on detail page
        const captcha = await waitForCaptcha(120);
        if (!captcha.solved) {
          sendResponse(id, { error: 'CAPTCHA timeout on detail page' });
          break;
        }

        // Extra wait for any AJAX content
        await page.waitForTimeout(2000);

        // Extract all tables, preserving section structure
        const extracted = await page.evaluate(() => {
          const isHeading = (node) => {
            if (!node) return false;
            if (/^H[1-6]$/i.test(node.tagName)) return true;
            const cls = (node.className || '').toLowerCase();
            if (cls.includes('title') || cls.includes('header') || cls.includes('section')) return true;
            return false;
          };

          const sections = [];
          let current = { heading: 'General', kvs: {}, tables: [] };

          const flush = () => {
            if (Object.keys(current.kvs).length > 0 || current.tables.length > 0) {
              sections.push(current);
            }
          };

          const processTable = (tb) => {
            const rows = Array.from(tb.rows);
            if (rows.length === 0) return;

            // Determine if this is a key-value table (2 columns, first cell ends with colon)
            const kvRows = rows.filter(r => r.cells.length === 2);
            const isKeyValue = kvRows.length >= Math.max(1, rows.length - 1) &&
                               kvRows.every(r => r.cells[0].innerText.trim().endsWith(':'));

            if (isKeyValue) {
              kvRows.forEach(r => {
                const key = r.cells[0].innerText.trim().toLowerCase().replace(/:$/, '');
                const val = r.cells[1].innerText.trim();
                if (key && val) current.kvs[key] = val;
              });
            } else {
              const headers = Array.from(rows[0].cells).map(c => c.innerText.trim().toLowerCase());
              const data = [];
              for (let i = 1; i < rows.length; i++) {
                const cells = Array.from(rows[i].cells).map(c => c.innerText.trim());
                if (cells.some(c => c.length > 0)) {
                  const obj = {};
                  headers.forEach((h, idx) => { if (h) obj[h] = cells[idx] || ''; });
                  data.push(obj);
                }
              }
              if (data.length > 0) {
                current.tables.push({ headers, rows: data });
              }
            }
          };

          const walk = (node) => {
            if (node.tagName === 'TABLE') {
              processTable(node);
              return;
            }
            if (isHeading(node)) {
              flush();
              current = { heading: node.innerText.trim(), kvs: {}, tables: [] };
              return;
            }
            for (const child of node.children) {
              walk(child);
            }
          };

          walk(document.body);
          flush();

          const allKvs = {};
          sections.forEach(s => Object.assign(allKvs, s.kvs));

          return { sections, kvs: allKvs };
        });

        sendResponse(id, {
          url: page.url(),
          ...extracted
        });
        break;
      }

      case 'close': {
        if (browser) await browser.close();
        browser = null; context = null; page = null;
        sendResponse(id, { status: 'closed' });
        break;
      }

      default: {
        sendResponse(id, null, `Unknown method: ${method}`);
      }
    }
  } catch (err) {
    sendResponse(id, null, err.message || String(err));
  }
}

process.stdin.setEncoding('utf8');
let buffer = '';
process.stdin.on('data', (chunk) => {
  buffer += chunk;
  let lines = buffer.split('\n');
  buffer = lines.pop();
  for (const line of lines) {
    if (!line.trim()) continue;
    try {
      const cmd = JSON.parse(line);
      handleCommand(cmd);
    } catch (e) {
      sendResponse(null, null, 'Invalid JSON: ' + e.message);
    }
  }
});

process.stdin.on('end', () => {
  if (browser) browser.close().catch(() => {});
});

// Keep alive
setInterval(() => {}, 1000);
