use muxy_protocol::{SandboxNetwork, SandboxPolicy, SandboxSettings, SandboxSpec, ServerPath};

#[test]
fn policies_reject_ambiguous_network_and_environment_permissions() {
    let mut policy = SandboxPolicy::default();
    assert!(policy.validate().is_ok());
    policy.network = SandboxNetwork::Domains;
    assert!(policy.validate().is_err());
    for domain in [
        "https://example.com",
        "*.example.com",
        "example.com:443",
        "127.0.0.1",
        ".example.com",
        "a..com",
        "-a.com",
    ] {
        policy.domains = vec![domain.into()];
        assert!(policy.validate().is_err(), "{domain}");
    }
    policy.domains = vec!["api.example.com".into()];
    assert!(policy.validate().is_ok());
    for name in [
        "HOME",
        "PATH",
        "DYLD_INSERT_LIBRARIES",
        "NODE_OPTIONS",
        "NONO_PROFILE",
        "HTTP_PROXY",
        "SSH_AUTH_SOCK",
        "XDG_STATE_HOME",
        "key=value",
    ] {
        policy.environment = vec![name.into()];
        assert!(policy.validate().is_err(), "{name}");
    }
    policy.environment = vec!["EXAMPLE_API_KEY".into()];
    assert!(policy.validate().is_ok());
    policy.network = SandboxNetwork::Unrecognized(900);
    assert!(policy.validate().is_err());
}

#[test]
fn grants_require_absolute_bounded_paths() {
    for path in ["/", "relative", "/a\0b"] {
        let spec = SandboxSpec {
            workspace: ServerPath(path.as_bytes().to_vec()),
            policy: SandboxPolicy::default(),
        };
        assert!(spec.validate().is_err());
        let settings = SandboxSettings {
            executable: Some(spec.workspace),
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }
}

#[test]
fn legacy_session_records_have_no_sandbox() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(minicbor::Encode)]
    struct Legacy {
        #[n(0)]
        project: muxy_protocol::ProjectId,
        #[n(1)]
        id: muxy_protocol::SessionId,
        #[n(2)]
        directory: ServerPath,
    }
    let encoded = minicbor::to_vec(Legacy {
        project: muxy_protocol::ProjectId::from_u128(1),
        id: std::num::NonZeroU64::MIN.into(),
        directory: ServerPath(b"/tmp".to_vec()),
    })?;
    let info: muxy_protocol::SessionInfo = minicbor::decode(&encoded)?;
    assert!(info.sandbox.is_none());
    Ok(())
}
