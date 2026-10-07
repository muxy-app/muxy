use muxy_protocol::{ErrorCode, ServerPath, ServerSettingsDoc};

#[test]
fn server_configuration_has_bounded_budget_and_lossless_absolute_shell_paths() {
    let mut settings = ServerSettingsDoc {
        sandbox: None,
        default_shell: None,
        history_budget_bytes: 0,
        shell_integration: true,
    };
    assert!(settings.validate().is_ok());
    settings.history_budget_bytes = 64 * 1024 * 1024 * 1024 + 1;
    assert_eq!(settings.validate(), Err(ErrorCode::BadRequest));
    settings.history_budget_bytes = 16 * 1024 * 1024;
    for path in [b"relative/sh".to_vec(), b"/bin/sh\0extra".to_vec(), vec![]] {
        settings.default_shell = Some(ServerPath(path));
        assert_eq!(settings.validate(), Err(ErrorCode::BadPath));
    }
    settings.default_shell = Some(ServerPath(b"/bin/\xffshell".to_vec()));
    assert!(settings.validate().is_ok());
}
