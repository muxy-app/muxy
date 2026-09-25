//! Server executable composition root and process lifecycle.

mod args;
mod legacy;
mod logging;
mod remote_listener;
mod run;
mod settings_file;

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error}");
            let _ = writeln!(io::stderr(), "muxy-server: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [flag] if flag == "--build-info" => {
            serde_json::to_writer(io::stdout().lock(), &muxy_protocol::BuildInfo::current())?;
            Ok(())
        }
        [flag] if flag == "--version" || flag == "-V" => {
            writeln!(io::stdout(), "muxy-server {}", env!("CARGO_PKG_VERSION"))
        }
        [flag] if flag == "--help" || flag == "-h" => writeln!(
            io::stdout(),
            "Usage: muxy-server [--socket PATH --settings PATH --log PATH]\n       muxy-server --help | --version | --build-info"
        ),
        _ => {
            let args = args::Args::parse(arguments)?;
            let _lease = muxy_core::bundle::acquire_runtime(
                &muxy_core::executable::current_path()?,
                &serde_json::to_vec(&muxy_protocol::BuildInfo::current())?,
            )?;
            run::run(&args)
        }
    }
}
