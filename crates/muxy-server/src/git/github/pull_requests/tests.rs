use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::json;

use super::*;
use crate::git::tests::Repository;

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Lookup {
    repository: Repository,
    github: Github,
    current: CurrentBranch,
}

impl Lookup {
    fn new(responses: &[std::result::Result<String, &str>]) -> TestResult<Self> {
        let repository = Repository::new();
        let executable = repository.path.join("gh");
        fs::write(
            &executable,
            r#"#!/bin/sh
printf '%s\n' "$*" >> calls
index=$(wc -l < calls | tr -d ' ')
if [ -f "$index.err" ]; then
    cat "$index.err" >&2
    exit 1
fi
cat "$index.out"
"#,
        )?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
        for (index, response) in responses.iter().enumerate() {
            let (extension, contents) = match response {
                Ok(contents) => ("out", contents.as_str()),
                Err(contents) => ("err", *contents),
            };
            fs::write(
                repository.path.join(format!("{}.{extension}", index + 1)),
                contents,
            )?;
        }
        Ok(Self {
            repository,
            github: Github::with_cli(executable),
            current: CurrentBranch {
                branch: "feature".into(),
                head: "abcd".into(),
                configured_number: None,
            },
        })
    }

    fn run(&self) -> Result<Option<Value>> {
        self.github.lookup_current(
            &self.repository.path,
            &self.current,
            "https://github.com/owner/repo",
            "number,headRefName,headRefOid,state,isCrossRepository",
        )
    }

    fn calls(&self) -> TestResult<Vec<String>> {
        Ok(fs::read_to_string(self.repository.path.join("calls"))?
            .lines()
            .map(str::to_owned)
            .collect())
    }
}

fn pull_request(number: u64, branch: &str, head: &str, state: &str, fork: bool) -> Value {
    json!({
        "number": number,
        "headRefName": branch,
        "headRefOid": head,
        "state": state,
        "isCrossRepository": fork,
    })
}

#[test]
fn a_configured_open_pr_wins_even_for_a_differently_named_checkout() -> TestResult {
    let expected = pull_request(7, "upstream-feature", "other-head", "OPEN", true);
    let mut lookup = Lookup::new(&[Ok(expected.to_string())])?;
    lookup.current.configured_number = Some(7);
    assert_eq!(lookup.run()?, Some(expected));
    assert_eq!(
        lookup.calls()?,
        [
            "pr view 7 --repo https://github.com/owner/repo --json number,headRefName,headRefOid,state,isCrossRepository"
        ]
    );
    Ok(())
}

#[test]
fn branch_views_reject_closed_prs_wrong_branches_and_unrelated_forks() -> TestResult {
    for (branch, head, state, fork, accepted) in [
        ("feature", "different-head", "OPEN", false, true),
        ("feature", "ABCD", "OPEN", true, true),
        ("feature", "different-head", "OPEN", true, false),
        ("other", "abcd", "OPEN", false, false),
        ("feature", "abcd", "CLOSED", false, false),
    ] {
        for index in 0..2 {
            let candidate = pull_request(8, branch, head, state, fork);
            let mut responses = vec![Ok("null".into()), Ok("null".into()), Ok("[]".into())];
            responses[index] = Ok(candidate.to_string());
            let lookup = Lookup::new(&responses)?;
            assert_eq!(
                lookup.run()?,
                accepted.then_some(candidate),
                "view {index}: {branch}, {head}, {state}, fork={fork}"
            );
            let calls = lookup.calls()?;
            assert_eq!(calls.len(), if accepted { index + 1 } else { 3 });
            assert_eq!(
                calls[0],
                "pr view --json number,headRefName,headRefOid,state,isCrossRepository"
            );
            if calls.len() > 1 {
                assert_eq!(
                    calls[1],
                    "pr view feature --repo https://github.com/owner/repo --json number,headRefName,headRefOid,state,isCrossRepository"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn a_closed_configured_pr_and_numeric_branch_fall_back_to_an_exact_head_match() -> TestResult {
    let expected = pull_request(12, "42", "ABCD", "OPEN", true);
    let mut lookup = Lookup::new(&[
        Ok(pull_request(7, "42", "abcd", "CLOSED", false).to_string()),
        Ok(pull_request(8, "other", "abcd", "OPEN", false).to_string()),
        Ok(json!([
            pull_request(9, "42", "other-head", "OPEN", false),
            pull_request(10, "42", "abcd", "CLOSED", false),
            pull_request(11, "other", "abcd", "OPEN", false),
            expected,
            pull_request(13, "42", "abcd", "OPEN", false),
        ])
        .to_string()),
    ])?;
    lookup.current.branch = "42".into();
    lookup.current.configured_number = Some(7);
    assert_eq!(lookup.run()?, Some(expected));
    let calls = lookup.calls()?;
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[1],
        "pr view --json number,headRefName,headRefOid,state,isCrossRepository"
    );
    assert_eq!(
        calls[2],
        "pr list --repo https://github.com/owner/repo --head 42 --state open --limit 100 --json number,headRefName,headRefOid,state,isCrossRepository"
    );
    Ok(())
}

#[test]
fn successful_fallbacks_win_over_errors_from_earlier_views() -> TestResult {
    let expected = pull_request(8, "feature", "abcd", "OPEN", false);
    for index in 1..=3 {
        let mut responses = vec![
            Err("configured failure"),
            Err("context failure"),
            Err("branch failure"),
            Ok("[]".into()),
        ];
        responses[index] = Ok(if index == 3 {
            json!([expected]).to_string()
        } else {
            expected.to_string()
        });
        let mut lookup = Lookup::new(&responses)?;
        lookup.current.configured_number = Some(7);
        assert_eq!(lookup.run()?, Some(expected.clone()));
        assert_eq!(lookup.calls()?.len(), index + 1);
    }
    Ok(())
}

#[test]
fn an_unsuccessful_search_preserves_error_precedence() -> TestResult {
    for (responses, expected) in [
        (
            [
                Err("first failure"),
                Err("second failure"),
                Err("third failure"),
                Ok("[]".into()),
            ],
            Some("GitHub CLI: first failure"),
        ),
        (
            [
                Err("first failure"),
                Err("second failure"),
                Err("third failure"),
                Err("no pull requests found"),
            ],
            Some("GitHub CLI: first failure"),
        ),
        (
            [
                Err("no pull request found"),
                Err("second failure"),
                Err("third failure"),
                Ok("[]".into()),
            ],
            Some("GitHub CLI: second failure"),
        ),
        (
            [
                Err("no pull request found"),
                Err("could not resolve"),
                Err("third failure"),
                Ok("[]".into()),
            ],
            Some("GitHub CLI: third failure"),
        ),
        (
            [
                Err("first failure"),
                Ok("null".into()),
                Ok("null".into()),
                Err("list failure"),
            ],
            Some("GitHub CLI: list failure"),
        ),
        (
            [
                Err("no pull request found"),
                Err("could not resolve"),
                Err("no commits between"),
                Err("no pull requests found"),
            ],
            None,
        ),
        (
            [
                Ok("null".into()),
                Ok("null".into()),
                Ok("null".into()),
                Ok("[]".into()),
            ],
            None,
        ),
    ] {
        let mut lookup = Lookup::new(&responses)?;
        lookup.current.configured_number = Some(7);
        assert_eq!(
            lookup.run().map_err(|error| error.message().to_owned()),
            expected.map_or(Ok(None), |message| Err(message.to_owned()))
        );
        assert_eq!(lookup.calls()?.len(), 4);
    }
    Ok(())
}

#[test]
fn malformed_successful_responses_stop_lookup_immediately() -> TestResult {
    for index in 0..4 {
        let mut responses = vec![Err("earlier failure"); 4];
        responses[index] = Ok("not json".into());
        let mut lookup = Lookup::new(&responses)?;
        lookup.current.configured_number = Some(7);
        let error = lookup
            .run()
            .err()
            .ok_or("malformed response was accepted")?;
        assert!(error.message().contains("expected ident"), "{error}");
        assert_eq!(lookup.calls()?.len(), index + 1);
    }
    Ok(())
}
