use muxy_protocol::title::{derive, derive_for_header};
use muxy_protocol::{ForegroundProcess, ServerPath};

fn process(name: &str, is_shell: bool) -> ForegroundProcess {
    ForegroundProcess {
        name: name.into(),
        is_shell,
    }
}

#[test]
fn a_header_names_the_shell_where_a_tab_names_the_folder() {
    let directory = ServerPath::from("/home/dev/muxy");
    let shell = process("zsh", true);
    assert_eq!(derive("", Some(&shell), &directory), "muxy");
    assert_eq!(derive_for_header("", Some(&shell), &directory), "zsh");
    assert_eq!(
        derive_for_header("", Some(&process("-zsh", true)), &directory),
        "zsh"
    );
}

#[test]
fn a_header_otherwise_titles_a_terminal_as_a_tab_does() {
    let directory = ServerPath::from("/home/dev/muxy");
    let shell = process("zsh", true);
    let vim = process("vim", false);
    for (title, foreground) in [
        ("~/muxy", Some(&shell)),
        ("", Some(&vim)),
        ("", None),
        ("", Some(&process("", true))),
    ] {
        assert_eq!(
            derive_for_header(title, foreground, &directory),
            derive(title, foreground, &directory),
        );
    }
    assert_eq!(
        derive_for_header("~/muxy", Some(&shell), &directory),
        "~/muxy"
    );
    assert_eq!(derive_for_header("", Some(&vim), &directory), "vim");
    assert_eq!(derive_for_header("", None, &directory), "muxy");
}

#[test]
fn a_header_shows_the_folder_within_home_as_a_tilde() {
    let home = std::path::Path::new("/home/dev");
    let shown = |directory: &str, home| {
        muxy_protocol::title::directory_for_header(&ServerPath::from(directory), home)
    };
    assert_eq!(shown("", Some(home)), None);
    assert_eq!(shown("/home/dev", Some(home)).as_deref(), Some("~"));
    assert_eq!(
        shown("/home/dev/muxy/crates", Some(home)).as_deref(),
        Some("~/muxy/crates")
    );
    assert_eq!(
        shown("/home/developer", Some(home)).as_deref(),
        Some("/home/developer")
    );
    assert_eq!(shown("/srv/app", Some(home)).as_deref(), Some("/srv/app"));
    assert_eq!(
        shown("/home/dev/muxy", None).as_deref(),
        Some("/home/dev/muxy")
    );
    assert_eq!(
        shown("/etc", Some(std::path::Path::new("/"))).as_deref(),
        Some("/etc")
    );
}
