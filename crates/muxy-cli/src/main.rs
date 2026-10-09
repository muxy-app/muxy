//! Command-line and terminal client for the server on this computer or, with
//! `--host`, on another one; the server runs as a separate executable.
mod args;
mod clipboard;
mod input;
mod manage;
mod mobile;
mod open;
mod projects;
mod scroll;
mod selection;
mod skills;
mod state;
mod target;
mod terminal;
mod tui;
mod ui;
mod worker;

use args::Command;
use muxy_client::Start;
use std::io::{self, Write};
use std::process::ExitCode;
use target::Target;

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
    let (host, command) = args::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    match command {
        Command::Help => writeln!(io::stdout(), "{}", manage::help::ROOT)?,
        Command::Usage(usage) => writeln!(io::stdout(), "{usage}")?,
        Command::Version => writeln!(io::stdout(), "muxy {}", env!("CARGO_PKG_VERSION"))?,
        Command::BuildInfo => {
            serde_json::to_writer(io::stdout().lock(), &muxy_protocol::BuildInfo::current())?;
        }
        Command::Interactive => tui::run(host, None).map_err(io::Error::other)?,
        Command::Manage(command) => manage::run(*command, host)?,
        Command::Open(folder) => open::run(&folder)?,
        Command::InstallSkills(directories) => {
            let home = std::env::home_dir();
            let mut stdout = io::stdout().lock();
            for file in skills::install(home.as_deref(), &directories)? {
                writeln!(stdout, "{}", file.display())?;
            }
        }
        Command::Mobile(command) => {
            let _lease = muxy_client::local::bundle::acquire_runtime(
                &muxy_core::executable::current_path()?,
            )?;
            let target = Target::new(host)?;
            let client = target
                .connect(Start::IfNeeded)
                .map_err(|error| target.explain(error))?;
            mobile::run(command, &client).map_err(|error| target.explain(error))?;
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
