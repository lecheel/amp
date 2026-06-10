use crate::errors::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_SOCKET_PATH: &str = "/tmp/.ctagd.sock";
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Normalize a repo root path: strip trailing slashes so
/// /opt/ai/gh/amp and /opt/ai/gh/amp/ are treated identically.
fn normalize_repo_root(path: &Path) -> String {
    path.to_string_lossy().trim_end_matches('/').to_string()
}

#[derive(Debug, Clone, Serialize)]
struct Request {
    id: String,
    method: String,
    #[serde(rename = "repo_root")]
    repo_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    #[allow(dead_code)]
    id: String,
    result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefinitionResult {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub display: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SymbolResult {
    pub name: String,
    pub kind: Option<String>,
    pub relative_path: String,
    pub line: usize,
    // pub column: usize,
    // pub detail: Option<String>,
}

/// Check if the ctagd socket exists (quick availability check).
pub fn is_available() -> bool {
    PathBuf::from(DEFAULT_SOCKET_PATH).exists()
}

/// Notify ctagd about a repository so it can start indexing immediately.
pub fn register_repo(repo_root: &Path) {
    let req = Request {
        id: format!("register-{}", std::process::id()),
        method: "saved".to_string(),
        repo_root: normalize_repo_root(repo_root),
        file: None,
        content: None,
        line: None,
        column: None,
        symbol: None,
        query: None,
    };

    if let Err(e) = send_fire_and_forget(&req) {
        log::debug!("ctagd register_repo failed: {}", e);
    }
}

/// Notify ctagd that a file has been saved. Fire-and-forget.
pub fn notify_saved(repo_root: &Path, relative_path: &str, content: &str) {
    let req = Request {
        id: format!("save-{}", std::process::id()),
        method: "saved".to_string(),
        repo_root: normalize_repo_root(repo_root),
        file: Some(relative_path.to_string()),
        content: Some(content.to_string()),
        line: None,
        column: None,
        symbol: None,
        query: None,
    };

    if let Err(e) = send_fire_and_forget(&req) {
        log::debug!("ctagd notify_saved failed: {}", e);
    }
}

/// Request the definition location of a symbol from ctagd.
pub fn definition(
    repo_root: &Path,
    relative_path: &str,
    line: usize,
    column: usize,
    symbol: &str,
) -> anyhow::Result<Vec<DefinitionResult>> {
    let req = Request {
        id: format!("def-{}", std::process::id()),
        method: "definition".to_string(),
        repo_root: normalize_repo_root(repo_root),
        file: Some(relative_path.to_string()),
        content: None,
        line: Some(line),
        column: Some(column),
        symbol: Some(symbol.to_string()),
        query: None,
    };

    let response = send_request(&req)?;

    match response.result {
        Some(val) if !val.is_null() => {
            if val.is_array() {
                let defs: Vec<DefinitionResult> = serde_json::from_value(val)
                    .context("Failed to parse ctagd definition array")?;
                Ok(defs)
            } else {
                let def: DefinitionResult = serde_json::from_value(val.clone())
                    .context("Failed to parse ctagd definition response")?;
                Ok(vec![def])
            }
        }
        _ => Ok(Vec::new()),
    }
}

/// Search for symbols across the workspace by query.
pub fn workspace_symbols(repo_root: &Path, query: &str) -> anyhow::Result<Vec<SymbolResult>> {
    let req = Request {
        id: format!("sym-{}", std::process::id()),
        method: "workspace_symbols".to_string(),
        repo_root: normalize_repo_root(repo_root),
        file: None,
        content: None,
        line: None,
        column: None,
        symbol: None,
        query: Some(query.to_string()),
    };

    let response = send_request(&req)?;

    match response.result {
        Some(val) if !val.is_null() => {
            let symbols: Vec<SymbolResult> = serde_json::from_value(val)
                .context("Failed to parse ctagd workspace_symbols response")?;
            Ok(symbols)
        }
        _ => Ok(Vec::new()),
    }
}

fn send_request(req: &Request) -> anyhow::Result<Response> {
    let socket_path = PathBuf::from(DEFAULT_SOCKET_PATH);
    let stream = std::os::unix::net::UnixStream::connect(&socket_path)
        .with_context(|| format!("Failed to connect to ctagd at {}", socket_path.display()))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .context("Failed to set read timeout on ctagd socket")?;

    let mut json = serde_json::to_string(req).context("Failed to serialize ctagd request")?;
    json.push('\n');

    let mut writer = &stream;
    writer
        .write_all(json.as_bytes())
        .context("Failed to write to ctagd socket")?;
    writer.flush().context("Failed to flush ctagd socket")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("Failed to read from ctagd socket")?;

    let response: Response =
        serde_json::from_str(&line).context("Failed to parse ctagd response")?;
    Ok(response)
}

fn send_fire_and_forget(req: &Request) -> anyhow::Result<()> {
    let socket_path = PathBuf::from(DEFAULT_SOCKET_PATH);
    let stream = std::os::unix::net::UnixStream::connect(&socket_path)
        .with_context(|| format!("Failed to connect to ctagd at {}", socket_path.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .context("Failed to set read timeout on ctagd socket")?;

    let mut json = serde_json::to_string(req).context("Failed to serialize ctagd request")?;
    json.push('\n');

    let mut writer = &stream;
    writer
        .write_all(json.as_bytes())
        .context("Failed to write to ctagd socket")?;
    writer.flush().context("Failed to flush ctagd socket")?;
    Ok(())
}
