#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInfo {
    pub process_id: u32,
    pub name: String,
    pub arguments: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TtyProcessEntry {
    pub process_id: u32,
    pub process_group_id: u32,
    pub foreground_group_id: u32,
}

pub fn select_foreground_process(entries: &[TtyProcessEntry]) -> Option<u32> {
    let group = entries
        .iter()
        .map(|entry| entry.foreground_group_id)
        .find(|group| *group > 0)?;
    if entries.iter().any(|entry| entry.process_id == group) {
        return Some(group);
    }
    entries
        .iter()
        .filter(|entry| entry.process_group_id == group)
        .map(|entry| entry.process_id)
        .min()
}

pub fn is_running_command(foreground_process_id: Option<u32>, shell_process_id: u32) -> bool {
    foreground_process_id.is_some_and(|process_id| {
        process_id > 0 && shell_process_id > 0 && process_id != shell_process_id
    })
}

pub trait ProcessInspector {
    fn process(&self, process_id: u32) -> Option<ProcessInfo>;
    fn tty_processes(&self, tty_device: u64) -> Vec<TtyProcessEntry>;

    fn foreground_process_id(&self, tty_device: u64) -> Option<u32> {
        select_foreground_process(&self.tty_processes(tty_device))
    }

    fn foreground_process(&self, tty_device: u64) -> Option<ProcessInfo> {
        self.process(self.foreground_process_id(tty_device)?)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemProcessInspector;

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;

    use super::{ProcessInfo, ProcessInspector, SystemProcessInspector, TtyProcessEntry};

    const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
    const MAX_KINFO_PROC_BYTES: usize = 16 * 1024 * 1024;
    const KINFO_PROC_BYTES: usize = 648;
    const KINFO_PROC_PID_OFFSET: usize = 40;
    const KINFO_PROC_PGID_OFFSET: usize = 564;
    const KINFO_PROC_TPGID_OFFSET: usize = 576;

    unsafe extern "C" {
        fn proc_name(pid: libc::c_int, buffer: *mut c_void, buffersize: u32) -> libc::c_int;
    }

    impl ProcessInspector for SystemProcessInspector {
        fn process(&self, process_id: u32) -> Option<ProcessInfo> {
            let pid = i32::try_from(process_id).ok()?;
            Some(ProcessInfo {
                process_id,
                name: process_name(pid)?,
                arguments: process_arguments(pid)?,
            })
        }

        fn tty_processes(&self, tty_device: u64) -> Vec<TtyProcessEntry> {
            kern_proc_tty(tty_device)
        }
    }

    fn kern_proc_tty(tty_device: u64) -> Vec<TtyProcessEntry> {
        let Ok(tty_device) = u32::try_from(tty_device) else {
            return Vec::new();
        };
        if tty_device == 0 {
            return Vec::new();
        }
        let mut name = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_TTY,
            i32::from_ne_bytes(tty_device.to_ne_bytes()),
        ];
        let mut size = 0usize;
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as u32,
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            )
        } != 0
            || size == 0
            || size > MAX_KINFO_PROC_BYTES
        {
            return Vec::new();
        }
        let mut bytes = vec![0u8; size];
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as u32,
                bytes.as_mut_ptr().cast(),
                &mut size,
                ptr::null_mut(),
                0,
            )
        } != 0
        {
            return Vec::new();
        }
        bytes.truncate(size);
        decode_kern_proc_tty(&bytes)
    }

    fn decode_kern_proc_tty(bytes: &[u8]) -> Vec<TtyProcessEntry> {
        bytes
            .chunks_exact(KINFO_PROC_BYTES)
            .filter_map(|record| {
                let process_id = positive_u32(record, KINFO_PROC_PID_OFFSET)?;
                let process_group_id = positive_u32(record, KINFO_PROC_PGID_OFFSET)?;
                let foreground_group_id = positive_u32(record, KINFO_PROC_TPGID_OFFSET)?;
                Some(TtyProcessEntry {
                    process_id,
                    process_group_id,
                    foreground_group_id,
                })
            })
            .collect()
    }

    fn positive_u32(bytes: &[u8], offset: usize) -> Option<u32> {
        let value = i32::from_ne_bytes(
            bytes
                .get(offset..offset + size_of::<i32>())?
                .try_into()
                .ok()?,
        );
        u32::try_from(value).ok().filter(|value| *value > 0)
    }

    fn process_name(pid: i32) -> Option<String> {
        let mut buffer = [0u8; 1024];
        let len = unsafe {
            proc_name(
                pid,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).expect("fixed buffer fits u32"),
            )
        };
        let len = usize::try_from(len).ok().filter(|len| *len > 0)?;
        std::str::from_utf8(&buffer[..len]).ok().map(str::to_owned)
    }

    fn process_arguments(pid: i32) -> Option<Vec<String>> {
        let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
        let mut size = 0usize;
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as u32,
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            )
        } != 0
            || size < size_of::<i32>()
            || size > MAX_ARGUMENT_BYTES
        {
            return None;
        }
        let mut bytes = vec![0u8; size];
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as u32,
                bytes.as_mut_ptr().cast(),
                &mut size,
                ptr::null_mut(),
                0,
            )
        } != 0
        {
            return None;
        }
        bytes.truncate(size);
        decode_process_arguments(&bytes)
    }

    fn decode_process_arguments(bytes: &[u8]) -> Option<Vec<String>> {
        let argc_bytes: [u8; size_of::<i32>()] = bytes.get(..size_of::<i32>())?.try_into().ok()?;
        let argc = usize::try_from(i32::from_ne_bytes(argc_bytes)).ok()?;
        let mut cursor = size_of::<i32>();
        cursor += bytes.get(cursor..)?.iter().position(|byte| *byte == 0)? + 1;
        while bytes.get(cursor) == Some(&0) {
            cursor += 1;
        }
        let mut arguments = Vec::with_capacity(argc);
        while arguments.len() < argc && cursor < bytes.len() {
            let remainder = &bytes[cursor..];
            let end = remainder.iter().position(|byte| *byte == 0)?;
            arguments.push(std::str::from_utf8(&remainder[..end]).ok()?.to_owned());
            cursor += end + 1;
        }
        (arguments.len() == argc).then_some(arguments)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn decodes_kern_procargs2_layout() {
            let mut bytes = 3i32.to_ne_bytes().to_vec();
            bytes.extend_from_slice(b"/bin/zsh\0\0-zsh\0-l\0-i\0");
            assert_eq!(
                decode_process_arguments(&bytes),
                Some(vec!["-zsh".to_owned(), "-l".to_owned(), "-i".to_owned()])
            );
        }

        #[test]
        fn rejects_truncated_kern_procargs2_layout() {
            let mut bytes = 2i32.to_ne_bytes().to_vec();
            bytes.extend_from_slice(b"/bin/zsh\0\0-zsh\0");
            assert_eq!(decode_process_arguments(&bytes), None);
        }

        #[test]
        fn decodes_kern_proc_tty_records_and_ignores_invalid_ones() {
            let mut bytes = vec![0u8; KINFO_PROC_BYTES * 3];
            write_i32(&mut bytes, KINFO_PROC_PID_OFFSET, 42);
            write_i32(&mut bytes, KINFO_PROC_PGID_OFFSET, 42);
            write_i32(&mut bytes, KINFO_PROC_TPGID_OFFSET, 42);
            let second = KINFO_PROC_BYTES;
            write_i32(&mut bytes, second + KINFO_PROC_PID_OFFSET, 43);
            write_i32(&mut bytes, second + KINFO_PROC_PGID_OFFSET, 42);
            write_i32(&mut bytes, second + KINFO_PROC_TPGID_OFFSET, 42);
            let third = KINFO_PROC_BYTES * 2;
            write_i32(&mut bytes, third + KINFO_PROC_PID_OFFSET, -1);
            write_i32(&mut bytes, third + KINFO_PROC_PGID_OFFSET, 42);
            write_i32(&mut bytes, third + KINFO_PROC_TPGID_OFFSET, 42);
            bytes.push(1);
            assert_eq!(
                decode_kern_proc_tty(&bytes),
                vec![
                    TtyProcessEntry {
                        process_id: 42,
                        process_group_id: 42,
                        foreground_group_id: 42,
                    },
                    TtyProcessEntry {
                        process_id: 43,
                        process_group_id: 42,
                        foreground_group_id: 42,
                    },
                ]
            );
        }

        fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
            bytes[offset..offset + size_of::<i32>()].copy_from_slice(&value.to_ne_bytes());
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl ProcessInspector for SystemProcessInspector {
    fn process(&self, _process_id: u32) -> Option<ProcessInfo> {
        None
    }

    fn tty_processes(&self, _tty_device: u64) -> Vec<TtyProcessEntry> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_handles_empty_missing_and_vanished_groups() {
        assert_eq!(select_foreground_process(&[]), None);
        assert_eq!(
            select_foreground_process(&[TtyProcessEntry {
                process_id: 10,
                process_group_id: 10,
                foreground_group_id: 0,
            }]),
            None
        );
        assert_eq!(
            select_foreground_process(&[TtyProcessEntry {
                process_id: 10,
                process_group_id: 10,
                foreground_group_id: 42,
            }]),
            None
        );
    }

    #[test]
    fn selection_prefers_the_group_leader_then_lowest_surviving_member() {
        let entries = [
            TtyProcessEntry {
                process_id: 10,
                process_group_id: 10,
                foreground_group_id: 42,
            },
            TtyProcessEntry {
                process_id: 42,
                process_group_id: 42,
                foreground_group_id: 42,
            },
            TtyProcessEntry {
                process_id: 43,
                process_group_id: 42,
                foreground_group_id: 42,
            },
        ];
        assert_eq!(select_foreground_process(&entries), Some(42));
        let entries = [
            entries[0],
            TtyProcessEntry {
                process_id: 44,
                ..entries[1]
            },
            entries[2],
        ];
        assert_eq!(select_foreground_process(&entries), Some(43));
    }

    #[test]
    fn running_command_distinguishes_shell_from_other_foreground_processes() {
        assert!(!is_running_command(Some(900), 900));
        assert!(is_running_command(Some(950), 900));
        assert!(!is_running_command(None, 900));
        assert!(!is_running_command(Some(0), 900));
        assert!(!is_running_command(Some(950), 0));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn portable_system_inspector_fails_closed_without_platform_support() {
        let inspector = SystemProcessInspector;
        assert_eq!(inspector.process(1), None);
        assert!(inspector.tty_processes(1).is_empty());
        assert_eq!(inspector.foreground_process(1), None);
    }
}
