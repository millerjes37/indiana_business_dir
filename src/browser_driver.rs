use crate::models::{PaginationInfo, SearchResultRow};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, info, warn};

pub struct BrowserDriver {
    child: Child,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
    next_id: i64,
}

#[derive(Debug, Serialize)]
struct RpcRequest {
    id: i64,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RpcResponse {
    id: i64,
    result: Option<Value>,
    error: Option<String>,
}

/// Resolves the path to `scripts/browser_driver.js` relative to the running
/// binary so the tool works regardless of the user's current working directory.
fn resolve_browser_driver_path() -> Result<std::path::PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::canonicalize(p).ok())
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let candidates: Vec<std::path::PathBuf> = vec![
        // Primary: directory containing the resolved executable (follows symlinks)
        exe_dir,
        // Fallback: current working directory (useful during local development)
        std::env::current_dir().ok(),
    ]
    .into_iter()
    .flatten()
    .map(|dir| dir.join("scripts").join("browser_driver.js"))
    .collect();

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    anyhow::bail!(
        "Could not find scripts/browser_driver.js. \
         Searched: {:?}. \
         Make sure you run the binary from the installation directory \
         or that the 'scripts' folder is next to the executable.",
        candidates
    )
}

#[allow(dead_code)]
impl BrowserDriver {
    pub async fn spawn(headless: bool) -> Result<Self> {
        let script_path = resolve_browser_driver_path()?;

        info!("Spawning browser driver: {}", script_path.display());

        let mut child = Command::new("node")
            .arg(&script_path)
            .arg(headless.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn node browser_driver.js")?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to take stdin of browser driver")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to take stdout of browser driver")?;

        // Spawn stderr logger
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    eprintln!("[browser-driver] {}", line);
                }
            });
        }

        let mut driver = Self {
            child,
            stdin,
            stdout_reader: BufReader::new(stdout),
            next_id: 1,
        };

        driver
            .send_cmd("launch", serde_json::json!({"headless": headless}))
            .await?;
        Ok(driver)
    }

    async fn send_cmd_raw(&mut self, req: RpcRequest) -> Result<Value> {
        let line = serde_json::to_string(&req)?;
        debug!("-> {}", line);
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        let mut buf = String::new();
        self.stdout_reader
            .read_line(&mut buf)
            .await
            .context("Browser driver stdout closed")?;
        debug!("<- {}", buf.trim());

        let resp: RpcResponse = serde_json::from_str(&buf)
            .with_context(|| format!("Invalid JSON from browser driver: {}", buf.trim()))?;

        if let Some(err) = resp.error {
            anyhow::bail!("Browser driver error: {}", err);
        }

        resp.result.context("Browser driver returned empty result")
    }

    async fn send_cmd(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_cmd_raw(RpcRequest {
            id,
            method: method.to_string(),
            params,
        })
        .await
    }

    pub async fn navigate_search(&mut self) -> Result<()> {
        info!("Navigating to search page...");
        let _ = self.send_cmd("navigate_search", Value::Null).await?;
        Ok(())
    }

    pub async fn search_zip(&mut self, zip: &str) -> Result<Option<String>> {
        info!("Searching ZIP: {}", zip);
        let res = self
            .send_cmd("search_zip", serde_json::json!({"zip": zip}))
            .await?;
        let error = res
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(err) = error {
            if !err.is_empty() {
                warn!("Search ZIP error: {}", err);
                return Ok(Some(err));
            }
        }
        Ok(None)
    }

    pub async fn search_city(&mut self, city: &str) -> Result<Option<String>> {
        info!("Searching City: {}", city);
        let res = self
            .send_cmd("search_city", serde_json::json!({"city": city}))
            .await?;
        let error = res
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(err) = error {
            if !err.is_empty() {
                warn!("Search City error: {}", err);
                return Ok(Some(err));
            }
        }
        Ok(None)
    }

    pub async fn search_name_city(&mut self, name: &str, city: &str) -> Result<Option<String>> {
        info!("Searching Name: '{}' + City: '{}'", name, city);
        let res = self
            .send_cmd(
                "search_name_city",
                serde_json::json!({"name": name, "city": city}),
            )
            .await?;
        let error = res
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(err) = error {
            if !err.is_empty() {
                warn!("Search Name+City error: {}", err);
                return Ok(Some(err));
            }
        }
        Ok(None)
    }

    pub async fn extract_results(&mut self) -> Result<Vec<SearchResultRow>> {
        let res = self.send_cmd("extract_results", Value::Null).await?;
        let rows: Vec<SearchResultRow> =
            serde_json::from_value(res.get("rows").cloned().unwrap_or(Value::Array(vec![])))
                .context("Failed to parse search results")?;
        Ok(rows)
    }

    pub async fn get_pagination_info(&mut self) -> Result<PaginationInfo> {
        let res = self.send_cmd("get_pagination_info", Value::Null).await?;
        let info: PaginationInfo =
            serde_json::from_value(res).context("Failed to parse pagination info")?;
        Ok(info)
    }

    pub async fn click_next(&mut self) -> Result<bool> {
        let res = self.send_cmd("click_next", Value::Null).await?;
        let clicked = res
            .get("clicked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(clicked)
    }

    pub async fn get_detail(
        &mut self,
        business_id: &str,
        business_type: Option<&str>,
        is_series: Option<&str>,
    ) -> Result<Value> {
        info!("Fetching detail for business_id: {}", business_id);
        let mut params = serde_json::json!({"business_id": business_id});
        if let Some(bt) = business_type {
            params["business_type"] = serde_json::json!(bt);
        }
        if let Some(is) = is_series {
            params["is_series"] = serde_json::json!(is);
        }
        let res = self.send_cmd("get_detail", params).await?;
        Ok(res)
    }

    pub async fn close(mut self) -> Result<()> {
        let _ = self.send_cmd("close", Value::Null).await;
        let _ = self.child.kill().await;
        Ok(())
    }
}
