use super::ProcessRecord;
use muxy_proto::session::ProcessIdentity;
use std::io;

pub fn snapshot() -> io::Result<Vec<ProcessRecord>> {
    let mut records = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let Some(process_id) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(record) = read_record(process_id) {
            records.push(record);
        }
    }
    Ok(records)
}

fn read_record(process_id: u32) -> Option<ProcessRecord> {
    let stat = std::fs::read_to_string(format!("/proc/{process_id}/stat")).ok()?;
    let closing = stat.rfind(") ")?;
    let fields: Vec<_> = stat[closing + 2..].split_ascii_whitespace().collect();
    if fields.len() < 20 {
        return None;
    }
    Some(ProcessRecord {
        identity: ProcessIdentity {
            process_id,
            start_identity: fields[19].parse().ok()?,
        },
        parent_process_id: fields[1].parse().ok()?,
        process_group_id: fields[2].parse().ok()?,
        process_session_id: fields[3].parse().ok()?,
        tty_device: fields[4].parse::<i64>().ok()?.unsigned_abs(),
    })
}
