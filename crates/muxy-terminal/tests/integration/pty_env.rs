#![cfg(unix)]

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use muxy_terminal::pty::{Pty, PtyEvent, PtySize, SpawnRequest};

#[test]
fn removed_variables_are_not_inherited_from_the_servers_own_environment()
-> Result<(), Box<dyn std::error::Error>> {
    let inherited = std::env::vars_os()
        .map(|(name, _)| name)
        .find(|name| name == "HOME" || name == "PATH")
        .ok_or("the test process has neither HOME nor PATH")?;
    let pty = Pty::spawn(SpawnRequest {
        program: PathBuf::from("/usr/bin/env"),
        args: Vec::new(),
        cwd: std::env::temp_dir(),
        env: vec![("MUXY_PTY_KEPT".into(), "1".into())],
        remove: vec![inherited.clone()],
        size: PtySize {
            cols: 200,
            rows: 50,
        },
    })?;
    let (sender, receiver) = channel();
    let _reader = pty.start_reader(sender)?;
    let mut output = Vec::new();
    while let Ok(PtyEvent::Output(bytes)) = receiver.recv_timeout(Duration::from_secs(10)) {
        output.extend_from_slice(&bytes);
    }
    let output = String::from_utf8_lossy(&output);
    let prefix = format!("{}=", inherited.to_string_lossy());
    assert!(
        output
            .lines()
            .any(|line| line.trim_end() == "MUXY_PTY_KEPT=1"),
        "{output}"
    );
    assert!(
        !output.lines().any(|line| line.starts_with(&prefix)),
        "{output}"
    );
    Ok(())
}
