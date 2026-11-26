use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::store::Record;

const MAX_FILE: u64 = 64 * 1024;
const MAX_PROJECTS: usize = 4096;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessFile {
    pid: u32,
    session_id: String,
    cwd: Option<PathBuf>,
}

pub(crate) fn configuration() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|directory| directory.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|home| home.is_absolute())
                .map(|home| home.join(".claude"))
        })
}

pub(crate) fn record(configuration: &Path, pid: u32) -> Option<Record> {
    use std::io::Read;
    let file =
        std::fs::File::open(configuration.join("sessions").join(format!("{pid}.json"))).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE).read_to_end(&mut bytes).ok()?;
    let process: ProcessFile = serde_json::from_slice(&bytes).ok()?;
    let id = process.session_id;
    let valid = process.pid == pid
        && !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    let cwd = process.cwd.filter(|cwd| cwd.is_absolute());
    (valid && has_transcript(configuration, cwd.as_deref(), &id)).then(|| Record {
        argv: vec!["claude".into(), "--resume".into(), id],
        cwd,
    })
}

fn has_transcript(configuration: &Path, cwd: Option<&Path>, id: &str) -> bool {
    let projects = configuration.join("projects");
    let name = format!("{id}.jsonl");
    let encoded = cwd.map(|cwd| {
        cwd.to_string_lossy()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
    });
    if encoded.is_some_and(|folder| projects.join(folder).join(&name).is_file()) {
        return true;
    }
    std::fs::read_dir(&projects)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .take(MAX_PROJECTS)
        .any(|entry| entry.path().join(&name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_claude_resumes_only_a_saved_conversation_in_its_folder() -> std::io::Result<()> {
        let directory = std::env::temp_dir().join(format!("muxy-claude-{}", std::process::id()));
        std::fs::create_dir_all(directory.join("sessions"))?;
        let write = |pid: u32, contents: &str| {
            std::fs::write(
                directory.join("sessions").join(format!("{pid}.json")),
                contents,
            )
        };
        write(
            41,
            r#"{"pid":41,"sessionId":"84dd6a10-c00c-4e36","cwd":"/tmp/project","kind":"interactive"}"#,
        )?;
        write(42, r#"{"pid":7,"sessionId":"84dd6a10","cwd":"/tmp"}"#)?;
        write(43, r#"{"pid":43,"sessionId":"x; rm -rf ~","cwd":"/tmp"}"#)?;
        write(44, r#"{"pid":44,"sessionId":"abc","cwd":"relative"}"#)?;
        write(
            46,
            r#"{"pid":46,"sessionId":"never-sent","cwd":"/tmp/project"}"#,
        )?;
        let unsent = record(&directory, 41);
        std::fs::create_dir_all(directory.join("projects/-tmp-project"))?;
        std::fs::create_dir_all(directory.join("projects/-elsewhere"))?;
        std::fs::write(
            directory.join("projects/-tmp-project/84dd6a10-c00c-4e36.jsonl"),
            "{}\n",
        )?;
        std::fs::write(directory.join("projects/-elsewhere/abc.jsonl"), "{}\n")?;
        let found = (
            record(&directory, 41),
            record(&directory, 42),
            record(&directory, 43),
            record(&directory, 44),
            record(&directory, 45),
            record(&directory, 46),
        );
        std::fs::remove_dir_all(&directory)?;
        assert_eq!(
            found.0,
            Some(Record {
                argv: vec![
                    "claude".into(),
                    "--resume".into(),
                    "84dd6a10-c00c-4e36".into()
                ],
                cwd: Some(PathBuf::from("/tmp/project")),
            })
        );
        assert_eq!(found.1, None);
        assert_eq!(found.2, None);
        assert_eq!(
            found.3.map(|record| record.argv[2].clone()),
            Some("abc".to_owned())
        );
        assert_eq!(found.4, None);
        assert_eq!(unsent, None);
        assert_eq!(found.5, None);
        Ok(())
    }
}
