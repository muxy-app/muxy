//! Local command-line and terminal client; the server runs as a separate executable.
mod args;
mod input;
mod render;
mod state;
mod terminal;
mod tui;
mod worker;

use args::Command;
use muxy_protocol::{
    OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation, ServerPath,
};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
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
    let _lease = if matches!(command, Command::Projects | Command::AddProject { .. }) {
        muxy_client::local::bundle::acquire_runtime(&muxy_core::executable::current_path()?)?
    } else {
        None
    };
    match command {
        Command::Help => writeln!(
            io::stdout(),
            "Muxy — local terminal sessions\n\nUsage: muxy [COMMAND]\n\n  (no command)  Open the terminal client; Ctrl-B ? shows help\n  project list\n  project add <directory> [--name NAME]\n  --help | --version | --build-info"
        )?,
        Command::Version => writeln!(io::stdout(), "muxy {}", env!("CARGO_PKG_VERSION"))?,
        Command::BuildInfo => {
            serde_json::to_writer(io::stdout().lock(), &muxy_protocol::BuildInfo::current())?;
        }
        Command::Interactive => tui::run().map_err(io::Error::other)?,
        Command::Projects => {
            for project in client()?.catalog()?.projects {
                writeln!(
                    io::stdout(),
                    "{}\t{}\t{}",
                    project.id,
                    project.name.escape_debug(),
                    String::from_utf8_lossy(&project.directory.0).escape_debug()
                )?;
            }
        }
        Command::AddProject { directory, name } => add_project(&directory, name)?,
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

fn add_project(directory: &Path, name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = if directory.is_absolute() {
        directory.to_owned()
    } else {
        std::env::current_dir()?.join(directory)
    };
    if !directory.is_dir() {
        return Err("project directory must exist".into());
    }
    let name = name.unwrap_or_else(|| {
        directory
            .file_name()
            .unwrap_or(directory.as_os_str())
            .to_string_lossy()
            .into_owned()
    });
    let project = ProjectDescriptor {
        id: ProjectId::new(),
        home: false,
        name,
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: ServerPath(directory.as_os_str().as_bytes().into()),
        kind: None,
        parent_id: None,
    };
    project
        .validate()
        .map_err(|code| format!("invalid project: {code:?}"))?;
    client()?.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(project.clone()),
    })?;
    writeln!(
        io::stdout(),
        "{}\t{}",
        project.id,
        project.name.escape_debug()
    )?;
    Ok(())
}
