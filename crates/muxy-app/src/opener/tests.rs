#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

use super::*;

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("muxy-openers-{}", muxy_app_core::PaneId::new()));
        std::fs::create_dir(&root).expect("temp directory");
        Self(root)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn context() -> OpenContext {
    OpenContext {
        project: muxy_app_core::ProjectId::new(),
        pane: muxy_app_core::PaneId::new(),
        server: muxy_app_core::ServerId::local(),
        directory: "/project/sub directory".into(),
        project_directory: "/project".into(),
    }
}

fn file(path: impl Into<PathBuf>) -> FileLocation {
    FileLocation {
        path: path.into(),
        line: Some(12),
        column: Some(3),
    }
}

#[cfg(target_os = "macos")]
#[test]
fn editor_selection_and_failed_launch_fall_back_without_losing_context() {
    let editors = [
        Editor {
            bundle: "first".into(),
            path: "/Applications/First.app".into(),
            rank: 0,
        },
        Editor {
            bundle: "preferred".into(),
            path: "/Applications/Preferred.app".into(),
            rank: 1,
        },
    ];
    let request = OpenRequest {
        target: Target::File(file("/tmp/file.rs")),
        context: context(),
    };
    for (preference, application) in [
        ("preferred", "/Applications/Preferred.app"),
        ("missing", "/Applications/First.app"),
    ] {
        let settings = muxy_app_core::settings::OpenerSettings {
            project_target: Some(preference.into()),
            ..muxy_app_core::settings::OpenerSettings::default()
        };
        let mut calls = 0;
        open_with(
            &request,
            &settings,
            || &editors,
            |command| {
                calls += 1;
                if calls == 1 {
                    assert_eq!(
                        command,
                        &Launch::system([
                            "-a".into(),
                            application.into(),
                            "/project".into(),
                            "/tmp/file.rs".into()
                        ])
                    );
                    Err(io::Error::other("editor failed"))
                } else {
                    assert_eq!(command, &finder(Path::new("/tmp/file.rs")));
                    Ok(())
                }
            },
        )
        .expect("Finder fallback");
        assert_eq!(calls, 2);
        assert_eq!(settings.project_target.as_deref(), Some(preference));
    }
    let mut remote = request;
    remote.context.server = muxy_app_core::ServerId::new();
    assert!(
        open_with(
            &remote,
            &muxy_app_core::settings::OpenerSettings::default(),
            || &editors,
            |_| panic!("remote file must not launch")
        )
        .is_err()
    );
    assert!(resolve("/etc/hosts", &remote.context).is_none());
    assert!(resolve("https://example.com", &remote.context).is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn editor_cli_arguments_are_lossless_and_directories_are_not_line_locations() {
    let temp = Temp::new();
    let directory = temp.0.join("project space");
    std::fs::create_dir(&directory).expect("project");
    for (bundle, relative, goto) in [
        (
            "com.microsoft.VSCode",
            "Contents/Resources/app/bin/code",
            true,
        ),
        (
            "com.todesktop.230313mzl4w4u92",
            "Contents/Resources/app/bin/cursor",
            true,
        ),
        ("dev.zed.Zed", "Contents/MacOS/cli", false),
    ] {
        let editor = Editor {
            bundle: bundle.into(),
            path: temp.0.join(bundle),
            rank: 0,
        };
        let cli = editor.path.join(relative);
        std::fs::create_dir_all(cli.parent().expect("parent")).expect("app");
        std::fs::write(&cli, b"#!/bin/sh\nexit 0\n").expect("stub");
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).expect("executable");
        let location = file(PathBuf::from(OsString::from_vec(
            b"/tmp/file '\xff.rs".to_vec(),
        )));
        let command = editor.command(&location, &directory);
        let mut args = vec![directory.as_os_str().to_owned()];
        if goto {
            args.push("--goto".into());
        }
        args.push(OsString::from_vec(b"/tmp/file '\xff.rs:12:3".to_vec()));
        assert_eq!(
            command,
            Launch {
                program: cli.clone(),
                args
            }
        );
        launch(&command).expect("execute stub, not a shell command string");
        let command = editor.command(&file(&directory), &directory);
        assert_eq!(
            command,
            Launch {
                program: cli,
                args: vec![directory.as_os_str().to_owned()]
            }
        );
    }
    assert_eq!(
        finder(&directory),
        Launch::system(["-a".into(), "Finder".into(), directory.into_os_string()])
    );
}

#[cfg(unix)]
#[test]
fn a_timed_out_opener_is_reaped_and_the_worker_accepts_later_work() {
    let temp = Temp::new();
    let pid_file = temp.0.join("pid");
    let pid_path = pid_file.clone();
    let pool = WorkerPool::new("opener-timeout-test", 1, 4).expect("pool");
    let (send, receive) = std::sync::mpsc::channel();
    let finished = send.clone();
    pool.try_spawn(move || {
        let error = process::run(
            Command::new("/bin/sh")
                .args(["-c", "echo $$ > \"$1\"; while :; do :; done", "stub"])
                .arg(pid_path)
                .stdout(Stdio::null()),
            Duration::from_millis(150),
        )
        .expect_err("must time out");
        finished.send(Err(error.kind())).expect("result");
    })
    .expect("hang job");
    pool.try_spawn(move || {
        let command = Launch::program("/usr/bin/true", []);
        send.send(launch(&command).map_err(|error| error.kind()))
            .expect("result");
    })
    .expect("later job");
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("timeout released worker"),
        Err(io::ErrorKind::TimedOut)
    );
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("later work ran"),
        Ok(())
    );
    let pid = std::fs::read_to_string(pid_file).expect("pid");
    assert!(
        !Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("probe")
            .success()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_openers_preserve_literal_paths_and_use_the_file_manager_for_reveal() {
    let temp = Temp::new();
    let directory = temp.0.join("project space");
    std::fs::create_dir(&directory).expect("project");
    let location = file(PathBuf::from(OsString::from_vec(
        b"/tmp/$(touch injected) '\xff.rs".to_vec(),
    )));
    let editor = Editor {
        bundle: "com.microsoft.VSCode".into(),
        path: "/usr/bin/code".into(),
        rank: 0,
    };
    let command = editor.command(&location, &directory);
    assert_eq!(command.program, Path::new("/usr/bin/code"));
    assert_eq!(
        command.args,
        [
            directory.as_os_str().to_owned(),
            "--goto".into(),
            OsString::from_vec(b"/tmp/$(touch injected) '\xff.rs:12:3".to_vec())
        ]
    );
    assert_eq!(
        Launch::system([location.path.as_os_str().to_owned()]).program,
        Path::new("xdg-open")
    );
    let reveal = finder(&location.path);
    assert_eq!(reveal.select.as_deref(), Some(location.path.as_path()));
    assert_eq!(
        (reveal.program.as_path(), reveal.args.as_slice()),
        (Path::new("xdg-open"), [OsString::from("/tmp")].as_slice()),
        "without a file manager that selects, the folder opens"
    );
    let folder = finder(&directory);
    assert_eq!(folder.select, None);
    assert_eq!(folder.args, [directory.into_os_string()]);
}

#[test]
fn a_missing_program_is_named_and_other_failures_are_kept() {
    let program = if cfg!(windows) {
        r"C:\nonexistent\muxy-opener.exe"
    } else {
        "/nonexistent/muxy-opener"
    };
    let missing = Launch::program(program, ["a file".into()]);
    let error = launch(&missing).expect_err("nothing to run");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(error.to_string().contains(program), "{error}");
    let failing = if cfg!(windows) {
        Launch::program("cmd.exe", ["/c".into(), "exit 1".into()])
    } else {
        Launch::program("/usr/bin/false", [])
    };
    let error = launch(&failing).expect_err("exits with failure");
    assert_ne!(error.kind(), io::ErrorKind::NotFound);
}

#[cfg(windows)]
#[test]
fn windows_openers_hand_files_to_the_shell_and_editors_to_their_command_shims() {
    let temp = Temp::new();
    let directory = temp.0.join("project space");
    std::fs::create_dir(&directory).expect("project");
    let bin = temp.0.join("bin");
    std::fs::create_dir(&bin).expect("bin");
    std::fs::write(bin.join("code.cmd"), "").expect("shim");
    let path = muxy_core::executable::find_with_extensions(
        std::slice::from_ref(&bin),
        "code",
        ".EXE;.CMD",
    )
    .expect("editor");
    let location = file(directory.join("main.rs"));
    let editor = Editor {
        bundle: "com.microsoft.VSCode".into(),
        path,
        rank: 0,
    };
    let command = editor.command(&location, &directory);
    assert_eq!(command.program, bin.join("code.cmd"));
    assert_eq!(
        command.args,
        [
            directory.as_os_str().to_owned(),
            "--goto".into(),
            format!("{}:12:3", location.path.display()).into(),
        ]
    );
    let system = Launch::system([location.path.as_os_str().to_owned()]);
    assert!(system.program.as_os_str().is_empty(), "the shell opens it");
    assert_eq!(
        finder(&location.path).select.as_deref(),
        Some(location.path.as_path())
    );
    let folder = finder(&directory);
    assert_eq!(folder.select, None);
    assert_eq!(folder.args, [directory.into_os_string()]);
}
