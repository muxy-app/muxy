use muxy_core::resources::{ProcessIdentity, ProcessResourceRecord};
use std::io;

pub fn current_process_identity() -> io::Result<ProcessIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "resource monitoring is unavailable",
    ))
}

pub fn process_records() -> io::Result<Vec<ProcessResourceRecord>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "resource monitoring is unavailable",
    ))
}
