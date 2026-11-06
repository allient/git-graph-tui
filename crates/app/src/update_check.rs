//! Update check policy, independent of transport and UI.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const CHECK_INTERVAL: Duration = Duration::from_hours(24);

/// The last attempted check and the newest stable version it returned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cache {
    pub checked_at: u64,
    pub latest: Option<String>,
    pub dismissed: Option<String>,
}

impl Cache {
    /// Corrupt or missing cache files never prevent the app from starting.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// The cache is optional; inability to write it never interrupts the UI.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, contents)
    }

    pub fn due(&self, now: SystemTime) -> bool {
        let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        seconds.saturating_sub(self.checked_at) >= CHECK_INTERVAL.as_secs()
    }

    pub fn available(&self, current: &str) -> Option<&str> {
        let latest = self.latest.as_deref()?;
        if self.dismissed.as_deref() == Some(latest) {
            return None;
        }
        (stable_version(latest)? > stable_version(current)?).then_some(latest)
    }
}

/// Parses release versions strictly and excludes prereleases.
fn stable_version(value: &str) -> Option<semver::Version> {
    let version = semver::Version::parse(value.strip_prefix('v').unwrap_or(value)).ok()?;
    version.pre.is_empty().then_some(version)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

/// Fixed origin, no redirects, verified TLS, bounded request and response.
fn fetch_latest() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .max_redirects(0)
        .https_only(true)
        .build()
        .into();
    let mut response = agent
        .get("https://api.github.com/repos/allient/git-graph-tui/releases/latest")
        .header(
            "User-Agent",
            concat!("git-graph-tui/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let release: Release = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json()
        .ok()?;
    (!release.draft && !release.prerelease && stable_version(&release.tag_name).is_some())
        .then_some(release.tag_name)
}

/// Does all file and network work on a worker; failures are deliberately quiet.
pub fn check(path: Option<&Path>, force: bool) -> Cache {
    let mut cache = path.map_or_else(Cache::default, Cache::load);
    let now = SystemTime::now();
    if force || cache.due(now) {
        cache.checked_at = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        if let Some(latest) = fetch_latest() {
            cache.latest = Some(latest);
        }
        if let Some(path) = path {
            let _ = cache.save(path);
        }
    }
    cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_versions_compare_numerically_and_ignore_prereleases() {
        let mut cache = Cache {
            latest: Some("v0.10.0".into()),
            ..Cache::default()
        };
        assert_eq!(cache.available("0.9.9"), Some("v0.10.0"));
        assert_eq!(cache.available("0.10.0"), None);
        cache.latest = Some("v1.0.0-rc.1".into());
        assert_eq!(cache.available("0.9.9"), None);
    }

    #[test]
    fn dismissing_a_version_does_not_hide_a_later_release() {
        let mut cache = Cache {
            latest: Some("v0.4.0".into()),
            dismissed: Some("v0.4.0".into()),
            ..Cache::default()
        };
        assert_eq!(cache.available("0.3.5"), None);
        cache.latest = Some("v0.5.0".into());
        assert_eq!(cache.available("0.3.5"), Some("v0.5.0"));
    }

    #[test]
    fn cache_round_trips_and_corruption_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("updates").join("cache.toml");
        let cache = Cache {
            checked_at: 42,
            latest: Some("v1.2.3".into()),
            dismissed: None,
        };
        cache.save(&path).unwrap();
        assert_eq!(Cache::load(&path).available("1.2.2"), Some("v1.2.3"));
        std::fs::write(&path, "not valid toml =").unwrap();
        assert_eq!(Cache::load(&path).checked_at, 0);
    }

    #[test]
    fn checks_are_limited_to_daily_and_future_timestamps_are_safe() {
        let cache = Cache {
            checked_at: 100_000,
            ..Cache::default()
        };
        assert!(!cache.due(UNIX_EPOCH + Duration::from_secs(100_001)));
        assert!(!cache.due(UNIX_EPOCH + Duration::from_secs(1)));
        assert!(cache.due(UNIX_EPOCH + Duration::from_secs(186_400)));
    }
}
