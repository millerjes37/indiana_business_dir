use crate::models::{BusinessRecord, EnrichmentStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open SQLite database")?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory SQLite")?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS businesses (
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
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_county_status ON businesses(county, enrichment_status)",
            [],
        )?;
        // Add detail columns if they don't exist (for existing databases)
        let cols: Vec<String> = self
            .conn
            .prepare("PRAGMA table_info(businesses)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if !cols.contains(&"detail_business_type".to_string()) {
            self.conn.execute(
                "ALTER TABLE businesses ADD COLUMN detail_business_type TEXT",
                [],
            )?;
        }
        if !cols.contains(&"detail_is_series".to_string()) {
            self.conn.execute(
                "ALTER TABLE businesses ADD COLUMN detail_is_series TEXT",
                [],
            )?;
        }
        Ok(())
    }

    pub fn insert_discovered(
        &self,
        county: &str,
        business_id: &str,
        business_name: Option<&str>,
        entity_type: Option<&str>,
        status: Option<&str>,
        principal_address: Option<&str>,
        registered_agent_name: Option<&str>,
        detail_business_type: Option<&str>,
        detail_is_series: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO businesses (
                business_id, county, business_name, entity_type, status,
                principal_address, registered_agent_name, detail_business_type,
                detail_is_series, enrichment_status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(business_id) DO UPDATE SET
                business_name = COALESCE(business_name, excluded.business_name),
                entity_type = COALESCE(entity_type, excluded.entity_type),
                status = COALESCE(status, excluded.status),
                principal_address = COALESCE(principal_address, excluded.principal_address),
                registered_agent_name = COALESCE(registered_agent_name, excluded.registered_agent_name),
                detail_business_type = COALESCE(detail_business_type, excluded.detail_business_type),
                detail_is_series = COALESCE(detail_is_series, excluded.detail_is_series),
                updated_at = excluded.updated_at
            WHERE enrichment_status != 'complete'",
            params![
                business_id,
                county,
                business_name,
                entity_type,
                status,
                principal_address,
                registered_agent_name,
                detail_business_type,
                detail_is_series,
                EnrichmentStatus::Discovered.as_str(),
                now,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn update_enriched(&self, record: &BusinessRecord) -> Result<()> {
        let now = Utc::now().timestamp();
        // Only update fields that are present so we don't overwrite discovered data
        let fields: Vec<(&str, Option<&str>)> = vec![
            ("business_name", record.business_name.as_deref()),
            ("entity_type", record.entity_type.as_deref()),
            ("status", record.status.as_deref()),
            ("creation_date", record.creation_date.as_deref()),
            ("principal_address", record.principal_address.as_deref()),
            ("principal_city", record.principal_city.as_deref()),
            ("principal_zip", record.principal_zip.as_deref()),
            ("jurisdiction", record.jurisdiction.as_deref()),
            ("inactive_date", record.inactive_date.as_deref()),
            ("expiration_date", record.expiration_date.as_deref()),
            ("report_due_date", record.report_due_date.as_deref()),
            (
                "registered_agent_name",
                record.registered_agent_name.as_deref(),
            ),
            (
                "registered_agent_address",
                record.registered_agent_address.as_deref(),
            ),
            ("governing_persons", record.governing_persons.as_deref()),
            ("filing_history", record.filing_history.as_deref()),
            ("phone_number", record.phone_number.as_deref()),
            (
                "detail_business_type",
                record.detail_business_type.as_deref(),
            ),
            ("detail_is_series", record.detail_is_series.as_deref()),
        ];
        for (col, val) in fields {
            if let Some(v) = val {
                self.conn.execute(
                    &format!("UPDATE businesses SET {} = ?1 WHERE business_id = ?2", col),
                    params![v, &record.business_id],
                )?;
            }
        }
        self.conn.execute(
            "UPDATE businesses SET enrichment_status = ?1, updated_at = ?2 WHERE business_id = ?3",
            params![record.enrichment_status.as_str(), now, &record.business_id],
        )?;
        Ok(())
    }

    pub fn update_phone(&self, business_id: &str, phone: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            "UPDATE businesses SET phone_number = ?1, enrichment_status = ?2, updated_at = ?3 WHERE business_id = ?4",
            params![phone, EnrichmentStatus::Complete.as_str(), now, business_id],
        )?;
        Ok(())
    }

    pub fn get_pending_ids(&self, county: &str, status: EnrichmentStatus) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT business_id FROM businesses WHERE county = ?1 AND enrichment_status = ?2 ORDER BY business_id"
        )?;
        let rows = stmt.query_map(params![county, status.as_str()], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to collect pending IDs")
    }

    pub fn get_detail_params(
        &self,
        business_id: &str,
    ) -> Result<(String, Option<String>, Option<String>)> {
        let mut stmt = self.conn.prepare(
            "SELECT business_id, detail_business_type, detail_is_series FROM businesses WHERE business_id = ?1"
        )?;
        let row = stmt.query_row(params![business_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        Ok(row)
    }

    pub fn count_by_status(&self, county: &str) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT enrichment_status, COUNT(*) FROM businesses WHERE county = ?1 GROUP BY enrichment_status"
        )?;
        let rows = stmt.query_map(params![county], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to count by status")
    }

    pub fn get_records_for_export(&self, county: &str) -> Result<Vec<BusinessRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                business_id, county, business_name, entity_type, status,
                creation_date, principal_address, principal_city, principal_zip,
                jurisdiction, inactive_date, expiration_date, report_due_date,
                registered_agent_name, registered_agent_address,
                governing_persons, filing_history, phone_number,
                detail_business_type, detail_is_series,
                enrichment_status, created_at, updated_at
            FROM businesses WHERE county = ?1 ORDER BY business_id",
        )?;
        let rows = stmt.query_map(params![county], |row| {
            Ok(BusinessRecord {
                business_id: row.get(0)?,
                county: row.get(1)?,
                business_name: row.get(2)?,
                entity_type: row.get(3)?,
                status: row.get(4)?,
                creation_date: row.get(5)?,
                principal_address: row.get(6)?,
                principal_city: row.get(7)?,
                principal_zip: row.get(8)?,
                jurisdiction: row.get(9)?,
                inactive_date: row.get(10)?,
                expiration_date: row.get(11)?,
                report_due_date: row.get(12)?,
                registered_agent_name: row.get(13)?,
                registered_agent_address: row.get(14)?,
                governing_persons: row.get(15)?,
                filing_history: row.get(16)?,
                phone_number: row.get(17)?,
                detail_business_type: row.get(18)?,
                detail_is_series: row.get(19)?,
                enrichment_status: row
                    .get::<_, String>(20)?
                    .parse()
                    .unwrap_or(EnrichmentStatus::Discovered),
                created_at: DateTime::from_timestamp(row.get::<_, i64>(21)?, 0)
                    .unwrap_or_else(Utc::now),
                updated_at: DateTime::from_timestamp(row.get::<_, i64>(22)?, 0)
                    .unwrap_or_else(Utc::now),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to collect export records")
    }
}
