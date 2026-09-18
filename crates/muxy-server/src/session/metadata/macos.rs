use libproc::net_info::VInfoStat;
use libproc::proc_pid::{self, PIDInfo, PidInfoFlavor};
use libproc::processes::{self, ProcFilter};
use muxy_protocol::ServerPath;

pub(super) fn foreground_member(group: u32) -> Option<(i32, String)> {
    let named = |pid| {
        let pid = i32::try_from(pid).ok()?;
        proc_pid::name(pid).ok().map(|name| (pid, name))
    };
    if let Some(leader) = named(group) {
        return Some(leader);
    }
    let mut members = processes::pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group }).ok()?;
    members.sort_unstable();
    members.into_iter().find_map(named)
}

pub(super) fn process_directory(pid: i32) -> Option<ServerPath> {
    let info = proc_pid::pidinfo::<VnodePathInfo>(pid, 0).ok()?;
    let path = &info.current.path;
    let length = path.iter().position(|byte| *byte == 0)?;
    (path.first() == Some(&b'/')).then(|| ServerPath(path[..length].to_vec()))
}

#[repr(C)]
struct VnodeInfo {
    stat: VInfoStat,
    kind: i32,
    padding: i32,
    filesystem: [i32; 2],
}

#[repr(C)]
struct VnodePath {
    info: VnodeInfo,
    path: [u8; 1024],
}

#[repr(C)]
struct VnodePathInfo {
    current: VnodePath,
    root: VnodePath,
}

impl PIDInfo for VnodePathInfo {
    fn flavor() -> PidInfoFlavor {
        PidInfoFlavor::VNodePathInfo
    }
}

pub(super) fn agent_provider(group: u32) -> Option<muxy_protocol::AgentProvider> {
    let mut pids = processes::pids_by_type(ProcFilter::ByProgramGroup { pgrpid: group }).ok()?;
    pids.sort_unstable();
    pids.into_iter().take(128).find_map(|pid| {
        let pid = i32::try_from(pid).ok()?;
        let name = proc_pid::name(pid).ok()?;
        crate::detection::identify(&name, &process_arguments(pid))
    })
}

#[allow(
    unsafe_code,
    reason = "Darwin exposes process arguments only through sysctl"
)]
fn process_arguments(pid: i32) -> Vec<String> {
    unsafe extern "C" {
        fn sysctl(
            name: *mut std::ffi::c_int,
            namelen: u32,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut std::ffi::c_void,
            newlen: usize,
        ) -> std::ffi::c_int;
    }
    let mut mib = [1, 49, pid]; // CTL_KERN, KERN_PROCARGS2
    let mut bytes = vec![0u8; 64 * 1024];
    let mut length = bytes.len();
    // SAFETY: all pointers reference initialized, writable buffers of the supplied lengths.
    if unsafe {
        sysctl(
            mib.as_mut_ptr(),
            3,
            bytes.as_mut_ptr().cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Vec::new();
    }
    bytes.truncate(length);
    let Some(count) = bytes
        .get(..4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(i32::from_ne_bytes)
    else {
        return Vec::new();
    };
    let Some(rest) = bytes.get(4..) else {
        return Vec::new();
    };
    let Some(executable_end) = rest.iter().position(|b| *b == 0) else {
        return Vec::new();
    };
    let args = &rest[executable_end..];
    let Some(start) = args.iter().position(|b| *b != 0) else {
        return Vec::new();
    };
    args[start..]
        .split(|b| *b == 0)
        .take(usize::try_from(count).unwrap_or(0).min(64))
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnode_layout_matches_the_darwin_abi() {
        assert_eq!(size_of::<VnodePathInfo>(), 2352);
        assert_eq!(std::mem::offset_of!(VnodePath, path), 152);
    }
}
