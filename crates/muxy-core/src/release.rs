//! Which release a build is, read from its version.
//!
//! Stable releases are `X.Y.Z`. Betas are `X.Y.Z-beta.N`, where `N` is the
//! commit count that orders them; betas published before `BETA_VERSION` were
//! `2.0.0-beta-N` on the same count. Any other version, such as an unstamped
//! checkout's `2.0.0-beta-0`, is a local build.

/// Where a release is published and how it installs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Beta,
    Stable,
}

impl Channel {
    /// This build's channel. Local builds install like betas.
    pub fn current() -> Self {
        Version::current().map_or(Self::Beta, Version::channel)
    }

    pub const fn app_name(self) -> &'static str {
        match self {
            Self::Beta => "Muxy Beta",
            Self::Stable => "Muxy",
        }
    }

    pub const fn bundle_identifier(self) -> &'static str {
        match self {
            Self::Beta => "com.muxy-beta.app",
            Self::Stable => "com.muxy.app",
        }
    }
}

/// A released version, in the order releases of its channel follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Beta { build: u64 },
    Stable([u64; 3]),
}

impl Version {
    pub fn parse(version: &str) -> Option<Self> {
        if let Some(build) = version.strip_prefix("2.0.0-beta-") {
            return beta(build);
        }
        let (base, build) = match version.split_once("-beta.") {
            Some((base, build)) => (base, Some(build)),
            None => (version, None),
        };
        let mut parts = [0; 3];
        let mut numbers = base.split('.');
        for part in &mut parts {
            *part = number(numbers.next()?)?;
        }
        if numbers.next().is_some() {
            return None;
        }
        match build {
            None => Some(Self::Stable(parts)),
            Some(build) => beta(build),
        }
    }

    /// This build's release, or `None` for a local build.
    pub fn current() -> Option<Self> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }

    pub const fn channel(self) -> Channel {
        match self {
            Self::Beta { .. } => Channel::Beta,
            Self::Stable(_) => Channel::Stable,
        }
    }

    /// Whether this release replaces `other`. Releases of different channels never replace each other.
    pub fn is_newer_than(self, other: Self) -> bool {
        match (self, other) {
            (Self::Beta { build }, Self::Beta { build: other }) => build > other,
            (Self::Stable(version), Self::Stable(other)) => version > other,
            _ => false,
        }
    }
}

/// Whether `target` is a newer release than `current` on the same channel.
pub fn is_newer(target: &str, current: &str) -> bool {
    Version::parse(target)
        .zip(Version::parse(current))
        .is_some_and(|(target, current)| target.is_newer_than(current))
}

fn beta(build: &str) -> Option<Version> {
    if build.starts_with('0') {
        return None;
    }
    Some(Version::Beta {
        build: number(build)?,
    })
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_replace_only_older_releases_of_their_channel() {
        assert!(is_newer("2.0.0-beta.10", "2.0.0-beta.9"));
        assert!(is_newer("2.1.0-beta.10", "2.0.0-beta.9"));
        assert!(!is_newer("2.0.0-beta.9", "2.0.0-beta.9"));
        assert!(is_newer("2.0.10", "2.0.9"));
        assert!(is_newer("3.0.0", "2.9.9"));
        assert!(!is_newer("2.0.0", "2.0.1"));
        assert!(!is_newer("2.0.1", "2.0.0-beta.9"));
        assert!(!is_newer("2.0.0-beta.10", "2.0.0"));
        assert!(!is_newer("2.0.0-beta.10", "2.0.0-beta-0"));
        assert!(is_newer("2.0.0-beta.1123", "2.0.0-beta-1110"));
    }
}
