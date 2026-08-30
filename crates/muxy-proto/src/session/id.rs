use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, SessionIdError> {
        if value.len() != 36
            || value.as_bytes().get(8) != Some(&b'-')
            || value.as_bytes().get(13) != Some(&b'-')
            || value.as_bytes().get(18) != Some(&b'-')
            || value.as_bytes().get(23) != Some(&b'-')
            || value.bytes().enumerate().any(|(index, byte)| {
                !matches!(index, 8 | 13 | 18 | 23) && !byte.is_ascii_hexdigit()
            })
        {
            return Err(SessionIdError);
        }
        Uuid::parse_str(value).map(Self).map_err(|_| SessionIdError)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }

    pub fn uppercase(self) -> String {
        self.0.hyphenated().to_string().to_ascii_uppercase()
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.uppercase())
    }
}

impl FromStr for SessionId {
    type Err = SessionIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.uppercase())
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionIdError;

impl Display for SessionIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid session ID")
    }
}

impl std::error::Error for SessionIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_accepts_canonical_case_and_persists_uppercase() {
        let lower = "123e4567-e89b-12d3-a456-426614174000";
        let id = SessionId::parse(lower).unwrap();
        assert_eq!(id.to_string(), "123E4567-E89B-12D3-A456-426614174000");
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"123E4567-E89B-12D3-A456-426614174000\""
        );
        assert_eq!(
            serde_json::from_str::<SessionId>(&serde_json::to_string(&id).unwrap()).unwrap(),
            id
        );
    }

    #[test]
    fn session_id_rejects_noncanonical_or_invalid_values() {
        for value in [
            "",
            "123e4567e89b12d3a456426614174000",
            "123e4567-e89b-12d3-a456-42661417400",
            "123e4567-e89b-12d3-a456-42661417400z",
        ] {
            assert!(SessionId::parse(value).is_err(), "{value}");
        }
    }
}
