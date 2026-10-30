//! The tokens this computer signs in with on servers it paired with. They
//! live in `paired-servers.toml`, apart from `settings.toml`: only this user
//! can read it, and backups leave it out.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ServerId;
use crate::settings::{Error, Result};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingTokens {
    #[serde(default, rename = "servers", skip_serializing_if = "Vec::is_empty")]
    tokens: Vec<Token>,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Token {
    id: ServerId,
    token: String,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Token")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl PairingTokens {
    /// No file means no tokens.
    pub fn load(path: &Path) -> Result<Self> {
        let context = || path.display().to_string();
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(Error::new(context(), error)),
        };
        let tokens: Self = toml::from_str(&source)
            .map_err(|_| Error::new(context(), "invalid paired server tokens"))?;
        if tokens
            .tokens
            .iter()
            .any(|token| unhex::<32>(&token.token).is_none())
        {
            return Err(Error::new(context(), "invalid paired server tokens"));
        }
        Ok(tokens)
    }

    pub fn token(&self, server: ServerId) -> Option<[u8; 32]> {
        self.tokens
            .iter()
            .find(|token| token.id == server)
            .and_then(|token| unhex(&token.token))
    }

    /// Saves `server`'s token, replacing the one it had.
    pub fn set(path: &Path, server: ServerId, token: &[u8; 32]) -> Result<()> {
        Self::edit(path, |tokens| {
            tokens.retain(|stored| stored.id != server);
            tokens.push(Token {
                id: server,
                token: hex(token),
            });
        })
    }

    pub fn remove(path: &Path, servers: &[ServerId]) -> Result<()> {
        Self::edit(path, |tokens| {
            tokens.retain(|stored| !servers.contains(&stored.id));
        })
    }

    /// Changes the tokens as the file holds them now, while no other
    /// process can, so a token another app saved meanwhile is kept.
    fn edit(path: &Path, change: impl FnOnce(&mut Vec<Token>)) -> Result<()> {
        let context = || path.display().to_string();
        let _lock =
            super::private_file::lock(path).map_err(|error| Error::new(context(), error))?;
        let mut stored = Self::load(path)?;
        let before = stored.clone();
        change(&mut stored.tokens);
        if stored == before {
            return Ok(());
        }
        let source =
            toml::to_string_pretty(&stored).map_err(|error| Error::new(context(), error))?;
        super::private_file::write(path, source.as_bytes())
            .map_err(|error| Error::new(context(), error))
    }
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

pub(super) fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return None;
    }
    let mut bytes = [0; N];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        let high = char::from(pair[0]).to_digit(16)?;
        let low = char::from(pair[1]).to_digit(16)?;
        *byte = u8::try_from(high * 16 + low).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn tokens_are_private_and_kept_per_server() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("paired-servers.toml");
        let (studio, laptop) = (ServerId::new(), ServerId::new());
        assert_eq!(PairingTokens::load(&path)?.token(studio), None);
        PairingTokens::set(&path, studio, &[1; 32])?;
        PairingTokens::set(&path, laptop, &[2; 32])?;
        PairingTokens::set(&path, studio, &[3; 32])?;
        let mode = fs::metadata(&path)
            .map_err(|error| Error::new("test", error))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let tokens = PairingTokens::load(&path)?;
        assert_eq!(tokens.token(studio), Some([3; 32]));
        assert_eq!(tokens.token(laptop), Some([2; 32]));
        PairingTokens::remove(&path, &[studio])?;
        let tokens = PairingTokens::load(&path)?;
        assert_eq!(tokens.token(studio), None);
        assert_eq!(tokens.token(laptop), Some([2; 32]));
        Ok(())
    }

    #[test]
    fn tokens_saved_at_the_same_time_are_all_kept() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("paired-servers.toml");
        let servers: Vec<ServerId> = (0..8).map(|_| ServerId::new()).collect();
        std::thread::scope(|scope| {
            for (index, server) in servers.iter().enumerate() {
                let path = &path;
                scope.spawn(move || {
                    for _ in 0..10 {
                        PairingTokens::set(path, *server, &[u8::try_from(index).unwrap_or(0); 32])?;
                    }
                    Ok::<(), Error>(())
                });
            }
        });
        let tokens = PairingTokens::load(&path)?;
        for (index, server) in servers.iter().enumerate() {
            assert_eq!(
                tokens.token(*server),
                Some([u8::try_from(index).unwrap_or(0); 32])
            );
        }
        let leftovers = fs::read_dir(directory.path())
            .map_err(|error| Error::new("test", error))?
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        Ok(())
    }

    #[test]
    fn damaged_files_are_reported_without_their_contents() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("paired-servers.toml");
        fs::write(&path, "[[servers]]\nid = \"6b1f\"\ntoken = \"5ecre7\"\n")
            .map_err(|error| Error::new("test", error))?;
        let error = PairingTokens::load(&path)
            .err()
            .ok_or_else(|| Error::new("test", "a damaged file loaded"))?;
        assert!(!error.to_string().contains("5ecre7"), "{error}");
        Ok(())
    }
}
