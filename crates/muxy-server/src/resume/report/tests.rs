use std::path::PathBuf;

use super::{Report, Request, mac, verify};

const TOKEN: &str = "dummy-token-replace-me";

fn signed(body: &str) -> Vec<u8> {
    format!("{body}:mac={}", mac(body.as_bytes(), TOKEN)).into_bytes()
}

fn encoded(text: &str) -> String {
    muxy_core::base64::encode(text.as_bytes())
}

#[test]
fn the_published_clear_example_verifies() {
    let body = b"pid=74877:time=1795543200:clear=1:mac=c30603369f98fef581a4098be889175b0c74f0641e49f8b25f4326cb38f97c93";
    assert_eq!(
        verify(body, TOKEN),
        Some(Report {
            pid: 74877,
            time: 1_795_543_200,
            request: Request::Clear,
        })
    );
}

#[test]
fn reports_without_the_token_or_with_changed_bytes_have_no_effect() {
    let body = signed(&format!("pid=7:time=9:argv={}", encoded("claude")));
    assert!(verify(&body, TOKEN).is_some());
    assert_eq!(verify(&body, "another-token-of-enough-length"), None);
    let mut changed = body.clone();
    changed[4] = b'8';
    assert_eq!(verify(&changed, TOKEN), None);
    let truncated = &body[..body.len() - 1];
    assert_eq!(verify(truncated, TOKEN), None);
    let upper = String::from_utf8(body)
        .map(|text| text.to_uppercase())
        .unwrap_or_default();
    assert_eq!(verify(upper.as_bytes(), TOKEN), None);
    assert_eq!(verify(b"?", TOKEN), None);
}

#[test]
fn arguments_and_folder_decode_exactly_with_or_without_padding() {
    let argv = ["/usr/bin/nvim", "-S", "a b 'c' $(rm -rf ~) \u{e9}", ""];
    let elements: Vec<String> = argv
        .iter()
        .map(|argument| encoded(argument).trim_end_matches('=').to_owned())
        .collect();
    let body = signed(&format!(
        "pid=7:time=9:argv={}:cwd={}:future=x",
        elements.join(","),
        encoded("/tmp/work")
    ));
    assert_eq!(
        verify(&body, TOKEN).map(|report| report.request),
        Some(Request::Resume {
            argv: argv.map(String::from).to_vec(),
            cwd: Some(PathBuf::from("/tmp/work")),
        })
    );
}

#[test]
fn signed_reports_the_server_cannot_honour_are_rejected_for_their_process() {
    for body in [
        format!("pid=7:time=9:argv={}", encoded("relative/claude")),
        format!("pid=7:time=9:argv={}", encoded("-claude")),
        format!("pid=7:time=9:argv={}", encoded("claude\n")),
        format!("pid=7:time=9:argv={},{}", encoded(""), encoded("x")),
        format!(
            "pid=7:time=9:argv={}:cwd={}",
            encoded("claude"),
            encoded("tmp")
        ),
        format!("pid=7:time=9:argv={}:clear=1", encoded("claude")),
        "pid=7:time=9".to_owned(),
        "pid=7:time=9:clear=2".to_owned(),
        format!("pid=7:time=9:argv={}:need=argv,tabs", encoded("claude")),
        format!(
            "pid=7:time=9:argv={}:argv={}",
            encoded("claude"),
            encoded("nvim")
        ),
        format!("pid=7:time=9:argv={}", vec![encoded("x"); 65].join(",")),
        format!("pid=7:time=9:argv={}:Bad=1", encoded("claude")),
        "pid=7:time=9:argv=QQ=".to_owned(),
    ] {
        assert_eq!(
            verify(&signed(&body), TOKEN),
            Some(Report {
                pid: 7,
                time: 9,
                request: Request::Rejected,
            }),
            "{body}"
        );
    }
}

#[test]
fn signed_reports_without_a_valid_process_or_time_are_ignored() {
    for body in [
        "time=9:clear=1",
        "pid=0:time=9:clear=1",
        "pid=07:time=9:clear=1",
        "pid=7:clear=1",
        "pid=99999999999:time=9:clear=1",
    ] {
        assert_eq!(verify(&signed(body), TOKEN), None, "{body}");
    }
}
