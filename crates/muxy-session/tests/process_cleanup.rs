pub mod support;

use muxy_proto::session::SessionStatus;
use muxy_session::{identity_is_alive, process_identity};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command;
use support::{TestDaemon, read_until, renderer};

#[test]
fn end_session_terminates_background_grandchildren_after_process_group_changes() {
    let daemon = TestDaemon::start();
    let mut control = daemon.client();
    let descriptor = control
        .create_session(daemon.request("process-cleanup"))
        .unwrap();
    let mut renderer = renderer(&daemon.socket, descriptor.session_id, 1);
    let helper = std::env::current_exe().unwrap();
    let command = format!(
        "P8_ISOLATED_PROCESS_HELPER=1 {} --exact isolated_changed_process_group_helper --nocapture\n",
        shell_word(&helper.to_string_lossy())
    );
    renderer.send_input(command.as_bytes()).unwrap();
    let output = read_until(&mut renderer, b"P8_GRANDCHILD_SESSION:");
    let text = String::from_utf8_lossy(&output);
    let child = tagged_process_id(&text, "P8_CHILD:");
    let child_group = tagged_process_id(&text, "P8_CHILD_GROUP:");
    let grandchild = tagged_process_id(&text, "P8_GRANDCHILD:");
    let grandchild_session = tagged_process_id(&text, "P8_GRANDCHILD_SESSION:");
    assert_eq!(child, child_group);
    assert_eq!(grandchild, grandchild_session);
    let child_identity = process_identity(child).unwrap();
    let grandchild_identity = process_identity(grandchild).unwrap();
    assert!(identity_is_alive(child_identity));
    assert!(identity_is_alive(grandchild_identity));

    std::thread::sleep(std::time::Duration::from_millis(150));
    control.end_session(descriptor.session_id).unwrap();
    assert!(!identity_is_alive(descriptor.shell));
    assert!(!identity_is_alive(child_identity));
    assert!(!identity_is_alive(grandchild_identity));
    drop(renderer);
    drop(control);
    daemon.finish(false);
}

#[test]
fn natural_shell_exit_terminates_tracked_background_descendants() {
    let daemon = TestDaemon::start();
    let mut control = daemon.client();
    let descriptor = control
        .create_session(daemon.request("natural-exit-cleanup"))
        .unwrap();
    let mut renderer = renderer(&daemon.socket, descriptor.session_id, 1);
    let helper = std::env::current_exe().unwrap();
    let marker = daemon.root.path().join("p8-natural-exit-child");
    let command = format!(
        "P8_ISOLATED_NATURAL_EXIT_HELPER={} {} --exact isolated_natural_exit_helper --nocapture; sleep 0.2; exit\n",
        shell_word(&marker.to_string_lossy()),
        shell_word(&helper.to_string_lossy())
    );
    renderer.send_input(command.as_bytes()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !marker.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "natural-exit helper did not record its child"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let child = std::fs::read_to_string(&marker)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let child_identity = process_identity(child).unwrap();
    loop {
        let session = control.get_session(descriptor.session_id).unwrap().unwrap();
        if session.status == (SessionStatus::Exited { status: Some(0) }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "session did not finish natural-exit cleanup"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!identity_is_alive(child_identity));
    drop(renderer);
    drop(control);
    daemon.finish(false);
}

#[test]
fn isolated_natural_exit_helper() {
    let Some(marker) = std::env::var_os("P8_ISOLATED_NATURAL_EXIT_HELPER") else {
        return;
    };
    let child = unsafe { libc::fork() };
    assert!(child >= 0);
    if child == 0 {
        let executable = std::ffi::CString::new("/bin/sleep").unwrap();
        let duration = std::ffi::CString::new("30").unwrap();
        unsafe {
            if libc::setsid() < 0 {
                libc::_exit(126);
            }
            libc::execl(
                executable.as_ptr(),
                executable.as_ptr(),
                duration.as_ptr(),
                std::ptr::null::<libc::c_char>(),
            );
            libc::_exit(127);
        }
    }
    std::fs::write(marker, format!("{child}\n")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(150));
}

#[test]
fn isolated_changed_process_group_helper() {
    if std::env::var_os("P8_ISOLATED_PROCESS_HELPER").is_none() {
        return;
    }
    assert_eq!(unsafe { libc::setpgid(0, 0) }, 0);
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut grandchild = command.spawn().unwrap();
    println!("P8_CHILD:{}", std::process::id());
    println!("P8_CHILD_GROUP:{}", unsafe { libc::getpgrp() });
    println!("P8_GRANDCHILD:{}", grandchild.id());
    println!("P8_GRANDCHILD_SESSION:{}", unsafe {
        libc::getsid(grandchild.id() as libc::pid_t)
    });
    std::io::stdout().flush().unwrap();
    grandchild.wait().unwrap();
}

fn shell_word(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn tagged_process_id(output: &str, tag: &str) -> u32 {
    output
        .split(|character: char| character.is_ascii_whitespace())
        .find_map(|field| field.strip_prefix(tag))
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("missing {tag} in {output:?}"))
}
