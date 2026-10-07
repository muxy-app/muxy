use std::path::PathBuf;

pub(crate) fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_muxy"))
}
