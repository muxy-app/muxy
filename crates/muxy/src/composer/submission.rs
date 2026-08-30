use muxy_core::composer::submission::{ImageSubmissionStrategy, SubmissionPlan, SubmissionSegment};
use muxy_terminal::backend::shell_escape;
use muxy_terminal::input::{TerminalInputStep, TerminalInputTransaction};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

pub(crate) fn write_staged_status(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    muxy_core::store::write_private(path, contents)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionError {
    MissingFile(String),
    FileCheckFailed(String),
    MissingImage(String),
    ImageReadFailed(String),
    ImageNormalizationFailed(String),
}

impl fmt::Display for SubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile(path) => write!(formatter, "Attached file does not exist: {path}"),
            Self::FileCheckFailed(path) => {
                write!(formatter, "Could not verify attached file: {path}")
            }
            Self::MissingImage(filename) => {
                write!(formatter, "Copied Composer image is missing: {filename}")
            }
            Self::ImageReadFailed(filename) => {
                write!(
                    formatter,
                    "Could not read copied Composer image: {filename}"
                )
            }
            Self::ImageNormalizationFailed(filename) => {
                write!(
                    formatter,
                    "Could not normalize copied Composer image: {filename}"
                )
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionImage {
    pub path: String,
    pub png: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSubmission {
    pub revision: u64,
    pub target_pane_ids: Vec<String>,
    pub transaction: TerminalInputTransaction,
}

pub fn should_clear_submission(
    all_succeeded: bool,
    clear_after: bool,
    submitted_revision: u64,
    current_revision: u64,
) -> bool {
    all_succeeded && clear_after && submitted_revision == current_revision
}

pub fn copied_image_filenames(plan: &SubmissionPlan) -> Vec<String> {
    let mut seen = HashSet::new();
    plan.segments
        .iter()
        .filter_map(|segment| match segment {
            SubmissionSegment::CopiedImage { filename, .. } if seen.insert(filename.clone()) => {
                Some(filename.clone())
            }
            _ => None,
        })
        .collect()
}

pub fn resolve_submission(
    plan: SubmissionPlan,
    images: &HashMap<String, SubmissionImage>,
) -> Result<ResolvedSubmission, SubmissionError> {
    for segment in &plan.segments {
        if let SubmissionSegment::LocalPath(path) = segment {
            match Path::new(path).try_exists() {
                Ok(true) => {}
                Ok(false) => return Err(SubmissionError::MissingFile(path.clone())),
                Err(_) => return Err(SubmissionError::FileCheckFailed(path.clone())),
            }
        }
    }

    let contains_images = plan
        .segments
        .iter()
        .any(|segment| matches!(segment, SubmissionSegment::CopiedImage { .. }));
    let mut steps = Vec::new();
    if contains_images {
        steps.push(TerminalInputStep::ClearInput { submitted_lines: 0 });
        for segment in plan.segments {
            match segment {
                SubmissionSegment::Text(text) if !text.is_empty() => {
                    steps.push(TerminalInputStep::BracketedText(text));
                }
                SubmissionSegment::LocalPath(path) => {
                    steps.push(TerminalInputStep::BracketedText(shell_escape(&path)));
                }
                SubmissionSegment::CopiedImage { filename, .. } => {
                    let image = images
                        .get(&filename)
                        .ok_or_else(|| SubmissionError::MissingImage(filename.clone()))?;
                    match plan.image_strategy {
                        ImageSubmissionStrategy::Clipboard => {
                            steps.push(TerminalInputStep::PastePng(image.png.clone()));
                        }
                        ImageSubmissionStrategy::InlinePath => {
                            steps.push(TerminalInputStep::BracketedText(shell_escape(&image.path)));
                        }
                    }
                }
                SubmissionSegment::Text(_) => {}
            }
        }
    } else {
        let mut payload = String::new();
        for segment in plan.segments {
            match segment {
                SubmissionSegment::Text(text) => payload.push_str(&text),
                SubmissionSegment::LocalPath(path) => payload.push_str(&shell_escape(&path)),
                SubmissionSegment::CopiedImage { .. } => unreachable!(),
            }
        }
        if !payload.is_empty() {
            steps.extend([
                TerminalInputStep::ClearInput { submitted_lines: 0 },
                TerminalInputStep::BracketedText(payload),
            ]);
        }
    }

    Ok(ResolvedSubmission {
        revision: plan.revision,
        target_pane_ids: plan.target_pane_ids,
        transaction: if contains_images {
            TerminalInputTransaction::new(steps, plan.append_return).with_rollback_on_failure()
        } else {
            TerminalInputTransaction::new(steps, plan.append_return)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::composer::submission::{ImageSubmissionStrategy, SubmissionPlan};

    const IMAGE: &str = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.jpg";

    fn plan(segments: Vec<SubmissionSegment>) -> SubmissionPlan {
        SubmissionPlan {
            revision: 9,
            segments,
            append_return: true,
            image_strategy: ImageSubmissionStrategy::Clipboard,
            target_pane_ids: vec!["pane-a".to_owned()],
        }
    }

    fn images() -> HashMap<String, SubmissionImage> {
        HashMap::from([(
            IMAGE.to_owned(),
            SubmissionImage {
                path: "/tmp/image with quote's.jpg".to_owned(),
                png: vec![1, 2, 3],
            },
        )])
    }

    #[test]
    fn local_paths_are_preflighted_and_shell_escaped_only_in_the_app() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file with ' quote.txt");
        std::fs::write(&path, b"value").unwrap();
        let resolved = resolve_submission(
            plan(vec![SubmissionSegment::LocalPath(
                path.to_string_lossy().into_owned(),
            )]),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(
            resolved.transaction.steps,
            [
                TerminalInputStep::ClearInput { submitted_lines: 0 },
                TerminalInputStep::BracketedText(shell_escape(&path.to_string_lossy())),
            ]
        );
    }

    #[test]
    fn missing_files_and_unprepared_images_fail_before_a_terminal_transaction_exists() {
        assert_eq!(
            resolve_submission(
                plan(vec![SubmissionSegment::LocalPath(
                    "/path/that/does/not/exist".to_owned(),
                )]),
                &HashMap::new(),
            ),
            Err(SubmissionError::MissingFile(
                "/path/that/does/not/exist".to_owned()
            ))
        );
        assert_eq!(
            resolve_submission(
                plan(vec![SubmissionSegment::CopiedImage {
                    number: 1,
                    filename: IMAGE.to_owned(),
                }]),
                &HashMap::new(),
            ),
            Err(SubmissionError::MissingImage(IMAGE.to_owned()))
        );
    }

    #[test]
    fn clipboard_images_preserve_segment_order_and_inline_paths_are_escaped() {
        let segments = vec![
            SubmissionSegment::Text("before ".to_owned()),
            SubmissionSegment::CopiedImage {
                number: 1,
                filename: IMAGE.to_owned(),
            },
            SubmissionSegment::Text(" after".to_owned()),
        ];
        let clipboard = resolve_submission(plan(segments.clone()), &images()).unwrap();
        assert_eq!(
            clipboard.transaction.steps,
            [
                TerminalInputStep::ClearInput { submitted_lines: 0 },
                TerminalInputStep::BracketedText("before ".to_owned()),
                TerminalInputStep::PastePng(vec![1, 2, 3]),
                TerminalInputStep::BracketedText(" after".to_owned()),
            ]
        );
        let mut inline = plan(segments);
        inline.image_strategy = ImageSubmissionStrategy::InlinePath;
        let inline = resolve_submission(inline, &images()).unwrap();
        assert_eq!(
            inline.transaction.steps[2],
            TerminalInputStep::BracketedText(shell_escape("/tmp/image with quote's.jpg"))
        );
    }

    #[test]
    fn duplicate_image_tokens_require_one_prepared_image() {
        let plan = plan(vec![
            SubmissionSegment::CopiedImage {
                number: 1,
                filename: IMAGE.to_owned(),
            },
            SubmissionSegment::CopiedImage {
                number: 1,
                filename: IMAGE.to_owned(),
            },
        ]);
        assert_eq!(copied_image_filenames(&plan), [IMAGE]);
        let resolved = resolve_submission(plan, &images()).unwrap();
        assert_eq!(
            resolved
                .transaction
                .steps
                .iter()
                .filter(|step| matches!(step, TerminalInputStep::PastePng(_)))
                .count(),
            2
        );
    }

    #[test]
    fn clearing_requires_success_enabled_policy_and_an_unchanged_revision() {
        assert!(should_clear_submission(true, true, 4, 4));
        assert!(!should_clear_submission(false, true, 4, 4));
        assert!(!should_clear_submission(true, false, 4, 4));
        assert!(!should_clear_submission(true, true, 4, 5));
    }

    #[test]
    fn return_policy_and_empty_payload_are_preserved() {
        let mut without_return = plan(vec![SubmissionSegment::Text("hello".to_owned())]);
        without_return.append_return = false;
        assert!(
            !resolve_submission(without_return, &HashMap::new())
                .unwrap()
                .transaction
                .append_return
        );
        assert!(
            resolve_submission(plan(Vec::new()), &HashMap::new())
                .unwrap()
                .transaction
                .steps
                .is_empty()
        );
    }
}
