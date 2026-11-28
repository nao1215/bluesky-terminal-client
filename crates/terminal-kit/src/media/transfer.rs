//! Bounded HTTP transfers shared by attachment uploads and downloads.
use std::io::Read;
use std::time::Duration;
/// Upload a byte body, refusing redirects so authorization cannot cross servers.
pub fn put(
    url: &str,
    mime: &str,
    authorization: &str,
    data: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut response = agent
        .put(url)
        .header("Content-Type", mime)
        .header("Authorization", authorization)
        .send(data)
        .map_err(|e| e.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(65536)
        .read_to_vec()
        .map_err(|e| e.to_string())
}
/// Download at most `limit` bytes; fail instead of returning a truncated file.
pub fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut response = super::agent().get(url).call().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download HTTP {}", response.status()));
    }
    let mut data = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > limit {
        return Err("download exceeds size limit".into());
    }
    Ok(data)
}
