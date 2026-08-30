use super::ImageAttachment;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageSubmissionStrategy {
    Clipboard,
    InlinePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionSegment {
    Text(String),
    LocalPath(String),
    CopiedImage { number: u64, filename: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionSnapshot {
    pub text: String,
    pub revision: u64,
    pub selected_text: Option<String>,
    pub file_paths: Vec<String>,
    pub image_attachments: Vec<ImageAttachment>,
    pub append_return: bool,
    pub image_strategy: ImageSubmissionStrategy,
    pub target_pane_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionPlan {
    pub revision: u64,
    pub segments: Vec<SubmissionSegment>,
    pub append_return: bool,
    pub image_strategy: ImageSubmissionStrategy,
    pub target_pane_ids: Vec<String>,
}

impl SubmissionPlan {
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

pub fn plan_submission(snapshot: SubmissionSnapshot) -> SubmissionPlan {
    let selected = snapshot
        .selected_text
        .as_deref()
        .filter(|selected| !selected.trim().is_empty());
    let body = selected.unwrap_or(&snapshot.text);
    let body_is_blank = body.trim().is_empty();
    let mut segments = Vec::new();
    if selected.is_none() {
        let mut seen = HashSet::new();
        for path in &snapshot.file_paths {
            if !seen.insert(path) {
                continue;
            }
            if !segments.is_empty() {
                push_text(&mut segments, " ");
            }
            segments.push(SubmissionSegment::LocalPath(path.clone()));
        }
    }
    if !body_is_blank {
        if !segments.is_empty() {
            push_text(&mut segments, " ");
        }
        let images = snapshot
            .image_attachments
            .iter()
            .map(|attachment| (attachment.number, attachment.filename.as_str()))
            .collect::<BTreeMap<_, _>>();
        parse_body(body, &images, &mut segments);
    }
    SubmissionPlan {
        revision: snapshot.revision,
        segments,
        append_return: snapshot.append_return,
        image_strategy: snapshot.image_strategy,
        target_pane_ids: snapshot.target_pane_ids,
    }
}

fn parse_body(body: &str, images: &BTreeMap<u64, &str>, segments: &mut Vec<SubmissionSegment>) {
    let mut cursor = 0;
    while let Some(relative) = body[cursor..].find("[Image ") {
        let start = cursor + relative;
        let number_start = start + "[Image ".len();
        let Some(relative_end) = body[number_start..].find(']') else {
            break;
        };
        let end = number_start + relative_end;
        let raw = &body[number_start..end];
        let Ok(number) = raw.parse::<u64>() else {
            push_text(segments, &body[cursor..=end]);
            cursor = end + 1;
            continue;
        };
        push_text(segments, &body[cursor..start]);
        if let Some(filename) = images.get(&number) {
            segments.push(SubmissionSegment::CopiedImage {
                number,
                filename: (*filename).to_owned(),
            });
        } else {
            push_text(segments, &body[start..=end]);
        }
        cursor = end + 1;
    }
    push_text(segments, &body[cursor..]);
}

fn push_text(segments: &mut Vec<SubmissionSegment>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(SubmissionSegment::Text(current)) = segments.last_mut() {
        current.push_str(text);
    } else {
        segments.push(SubmissionSegment::Text(text.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str) -> SubmissionSnapshot {
        SubmissionSnapshot {
            text: text.to_owned(),
            revision: 7,
            selected_text: None,
            file_paths: Vec::new(),
            image_attachments: Vec::new(),
            append_return: true,
            image_strategy: ImageSubmissionStrategy::Clipboard,
            target_pane_ids: vec!["pane-a".to_owned()],
        }
    }

    #[test]
    fn selected_nonblank_text_suppresses_files_and_blank_selection_uses_the_draft() {
        let mut selected = snapshot("draft");
        selected.selected_text = Some("chosen".to_owned());
        selected.file_paths = vec!["/tmp/file".to_owned()];
        assert_eq!(
            plan_submission(selected).segments,
            [SubmissionSegment::Text("chosen".to_owned())]
        );

        let mut blank = snapshot("draft");
        blank.selected_text = Some(" \n".to_owned());
        blank.file_paths = vec!["/tmp/file".to_owned()];
        assert_eq!(
            plan_submission(blank).segments,
            [
                SubmissionSegment::LocalPath("/tmp/file".to_owned()),
                SubmissionSegment::Text(" draft".to_owned()),
            ]
        );
    }

    #[test]
    fn files_precede_body_with_exact_spacing_and_whitespace_body_is_omitted() {
        let mut value = snapshot("body");
        value.file_paths = vec!["/tmp/a".to_owned(), "/tmp/b".to_owned()];
        assert_eq!(
            plan_submission(value).segments,
            [
                SubmissionSegment::LocalPath("/tmp/a".to_owned()),
                SubmissionSegment::Text(" ".to_owned()),
                SubmissionSegment::LocalPath("/tmp/b".to_owned()),
                SubmissionSegment::Text(" body".to_owned()),
            ]
        );
        let mut whitespace = snapshot(" \n\t");
        whitespace.file_paths = vec!["/tmp/a".to_owned()];
        assert_eq!(
            plan_submission(whitespace).segments,
            [SubmissionSegment::LocalPath("/tmp/a".to_owned())]
        );
    }

    #[test]
    fn duplicate_files_are_removed_without_changing_first_seen_order() {
        let mut value = snapshot("");
        value.file_paths = vec![
            "/tmp/a".to_owned(),
            "/tmp/b".to_owned(),
            "/tmp/a".to_owned(),
        ];
        assert_eq!(
            plan_submission(value).segments,
            [
                SubmissionSegment::LocalPath("/tmp/a".to_owned()),
                SubmissionSegment::Text(" ".to_owned()),
                SubmissionSegment::LocalPath("/tmp/b".to_owned()),
            ]
        );
    }

    #[test]
    fn image_tokens_preserve_order_and_unknown_or_invalid_tokens_remain_literal() {
        let mut value = snapshot("a[Image 2]b[Image 9]c[Image x]d[Image 2]");
        value.image_attachments = vec![ImageAttachment {
            number: 2,
            filename: "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png".to_owned(),
        }];
        assert_eq!(
            plan_submission(value).segments,
            [
                SubmissionSegment::Text("a".to_owned()),
                SubmissionSegment::CopiedImage {
                    number: 2,
                    filename: "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png".to_owned(),
                },
                SubmissionSegment::Text("b[Image 9]c[Image x]d".to_owned()),
                SubmissionSegment::CopiedImage {
                    number: 2,
                    filename: "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn empty_snapshot_has_no_submission_segments() {
        assert!(plan_submission(snapshot(" \n")).is_empty());
    }
}
