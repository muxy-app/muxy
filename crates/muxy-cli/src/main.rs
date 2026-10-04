//! Local command-line and terminal client; the server runs as a separate executable.
mod args;
mod input;
mod manage;
mod mobile;
mod render;
mod state;
mod terminal;
mod tui;
mod worker;

use args::Command;
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "muxy: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let command = args::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    match command {
        Command::Help => writeln!(io::stdout(), "{}", manage::help::ROOT)?,
        Command::Version => writeln!(io::stdout(), "muxy {}", env!("CARGO_PKG_VERSION"))?,
        Command::BuildInfo => {
            serde_json::to_writer(io::stdout().lock(), &muxy_protocol::BuildInfo::current())?;
        }
        Command::Interactive => tui::run().map_err(io::Error::other)?,
        Command::Manage(command) => manage::run(*command)?,
        Command::Mobile(command) => {
            let _lease = muxy_client::local::bundle::acquire_runtime(
                &muxy_core::executable::current_path()?,
            )?;
            mobile::run(command, &client()?)?;
        }
        Command::Stdio(start) => {
            let _lease = muxy_client::local::bundle::acquire_runtime(
                &muxy_core::executable::current_path()?,
            )?;
            muxy_client::bridge::serve(
                &muxy_core::dirs::muxy_dir()?.join("server.sock"),
                &muxy_client::local::server_executable()?,
                start,
            )?;
        }
    }
    Ok(())
}

fn client() -> Result<muxy_client::Client, Box<dyn std::error::Error>> {
    let socket = muxy_core::dirs::muxy_dir()?.join("server.sock");
    Ok(muxy_client::local::ensure_running(
        &socket,
        &muxy_client::local::server_executable()?,
    )?)
}
