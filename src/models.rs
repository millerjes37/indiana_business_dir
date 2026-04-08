use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRecord {
    pub business_id: String,
    pub county: String,
    pub business_name: Option<String>,
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub creation_date: Option<String>,
    pub principal_address: Option<String>,
    pub principal_city: Option<String>,
    pub principal_zip: Option<String>,
    pub jurisdiction: Option<String>,
    pub inactive_date: Option<String>,
    pub expiration_date: Option<String>,
    pub report_due_date: Option<String>,
    pub registered_agent_name: Option<String>,
    pub registered_agent_address: Option<String>,
    pub governing_persons: Option<String>, // JSON
    pub filing_history: Option<String>,    // JSON
    pub phone_number: Option<String>,
    pub detail_business_type: Option<String>,
    pub detail_is_series: Option<String>,
    pub enrichment_status: EnrichmentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnrichmentStatus {
    Discovered,
    Enriched,
    Complete,
    Failed,
}

impl std::fmt::Display for EnrichmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnrichmentStatus::Discovered => write!(f, "discovered"),
            EnrichmentStatus::Enriched => write!(f, "enriched"),
            EnrichmentStatus::Complete => write!(f, "complete"),
            EnrichmentStatus::Failed => write!(f, "failed"),
        }
    }
}

impl EnrichmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EnrichmentStatus::Discovered => "discovered",
            EnrichmentStatus::Enriched => "enriched",
            EnrichmentStatus::Complete => "complete",
            EnrichmentStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for EnrichmentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discovered" => Ok(EnrichmentStatus::Discovered),
            "enriched" => Ok(EnrichmentStatus::Enriched),
            "complete" => Ok(EnrichmentStatus::Complete),
            "failed" => Ok(EnrichmentStatus::Failed),
            _ => Err(format!("Unknown status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultRow {
    pub business_id_display: String,
    pub business_name: String,
    pub name_type: String,
    pub entity_type: String,
    pub principal_address: String,
    pub registered_agent_name: String,
    pub status: String,
    pub detail_business_id: Option<String>,
    pub detail_business_type: Option<String>,
    pub detail_is_series: Option<String>,
    pub detail_link_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub text: String,
    pub current_page: Option<i64>,
    pub total_pages: Option<i64>,
    pub record_start: Option<i64>,
    pub record_end: Option<i64>,
    pub total_records: Option<i64>,
}
