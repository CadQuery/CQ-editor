//! Knowing when a newer CQ-editor is out (FEAT-E5).
//!
//! The worker asks GitHub for CQ-editor's releases (the GUI has no HTTP
//! client of its own); this decides when to ask and what an answer means.

use crate::settings::General;
use crate::worker::protocol::Release;

/// This CQ-editor's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The daily check waits this long between two checks, in seconds.
pub const CHECK_INTERVAL: u64 = 24 * 60 * 60;
/// Release pages are opened only on this site.
const RELEASES_SITE: &str = "https://github.com/";

/// `"v1.2.3-rc1"` -> `(1, 2, 3)`: the first three numbers, missing ones 0.
pub fn version_key(version: &str) -> (u64, u64, u64) {
    let mut numbers = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap_or(u64::MAX));
    let mut next = || numbers.next().unwrap_or(0);
    (next(), next(), next())
}

/// Whether the release `tag` is newer than the version `than`.
pub fn is_newer(tag: &str, than: &str) -> bool {
    version_key(tag) > version_key(than)
}

/// Whether the daily check is due `now` (seconds since the Unix epoch).
pub fn check_due(general: &General, now: u64) -> bool {
    general.check_updates && now.saturating_sub(general.last_update_check) >= CHECK_INTERVAL
}

/// The release to offer: the newest one found, while it is newer than this
/// CQ-editor (which may have been updated since) and its page is on GitHub.
pub fn available(general: &General) -> Option<&Release> {
    general
        .newer_release
        .as_ref()
        .filter(|release| is_newer(&release.tag, VERSION))
        .filter(|release| release.url.starts_with(RELEASES_SITE))
}

#[cfg(test)]
#[path = "../tests/rust/app/test_updates.rs"]
mod tests;
