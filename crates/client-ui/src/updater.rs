use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde::Deserialize;

pub const UPDATE_MANIFEST_URL: &str = "https://kevindev64.ru/rsmsg-downloads/stable/manifest.json";

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub minimum_supported_version: String,
    pub mandatory: bool,
    pub url: String,
    pub sha256: String,
    pub notes_url: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    minimum_supported_version: String,
    #[serde(default)]
    mandatory: bool,
    #[serde(default)]
    notes_url: Option<String>,
    platforms: BTreeMap<String, ManifestPlatform>,
}

#[derive(Deserialize)]
struct ManifestPlatform {
    url: String,
    sha256: String,
}

pub async fn check_for_update(
    manifest_url: &str,
    current_version: &str,
) -> Result<Option<UpdateInfo>> {
    let manifest = reqwest::get(manifest_url)
        .await?
        .error_for_status()?
        .json::<Manifest>()
        .await?;
    let platform_key = platform_key();
    let platform = manifest
        .platforms
        .get(platform_key)
        .ok_or_else(|| anyhow!("update manifest has no package for {platform_key}"))?;
    let update_available = version_gt(&manifest.version, current_version);
    let unsupported = version_gt(&manifest.minimum_supported_version, current_version);
    if !update_available && !unsupported {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        version: manifest.version,
        current_version: current_version.to_string(),
        minimum_supported_version: manifest.minimum_supported_version,
        mandatory: manifest.mandatory || unsupported,
        url: platform.url.clone(),
        sha256: platform.sha256.clone(),
        notes_url: manifest.notes_url,
    }))
}

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status()?;

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status()?;

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("could not open update url"))
    }
}

fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x86_64",
        ("macos", "aarch64") => "macos-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        ("linux", "x86_64") => "linux-x86_64",
        _ => "unknown",
    }
}

fn version_gt(left: &str, right: &str) -> bool {
    parse_version(left) > parse_version(right)
}

fn parse_version(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or_default());
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}
