use std::path::PathBuf;

pub(crate) fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_muxy"))
}

/// The server built beside [`binary`].
#[allow(dead_code, reason = "Not every test binary starts a server")]
pub(crate) fn server() -> PathBuf {
    muxy_core::executable::sibling(&binary(), "muxy-server")
}
