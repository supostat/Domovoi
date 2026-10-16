use std::io;

use crate::sys::{
    IOPOL_MATERIALIZE_DATALESS_FILES_OFF, IOPOL_SCOPE_PROCESS, IOPOL_SCOPE_THREAD, IOPOL_TYPE_DISK,
    IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_TYPE_VFS_TRIGGER_RESOLVE, IOPOL_UTILITY,
    IOPOL_VFS_TRIGGER_RESOLVE_OFF, getiopolicy_np, setiopolicy_np,
};
use crate::syscall::check;

pub fn set_thread_disk_policy_utility() -> io::Result<()> {
    set_policy(IOPOL_TYPE_DISK, IOPOL_SCOPE_THREAD, IOPOL_UTILITY)
}

pub fn thread_disk_policy() -> io::Result<u32> {
    let policy = check(unsafe { getiopolicy_np(as_int(IOPOL_TYPE_DISK), as_int(IOPOL_SCOPE_THREAD)) })?;
    Ok(u32::try_from(policy).expect("a successful policy is non-negative"))
}

pub fn set_vfs_policies() -> io::Result<()> {
    set_policy(
        IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
        IOPOL_SCOPE_PROCESS,
        IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
    )?;
    set_policy(
        IOPOL_TYPE_VFS_TRIGGER_RESOLVE,
        IOPOL_SCOPE_PROCESS,
        IOPOL_VFS_TRIGGER_RESOLVE_OFF,
    )
}

pub fn raise_nofile_limit() -> io::Result<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    check(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) })?;
    limit.rlim_cur = limit.rlim_max;
    check(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit) })?;
    check(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) })?;
    Ok(limit.rlim_cur)
}

fn set_policy(policy_type: u32, scope: u32, policy: u32) -> io::Result<()> {
    check(unsafe { setiopolicy_np(as_int(policy_type), as_int(scope), as_int(policy)) }).map(drop)
}

fn as_int(constant: u32) -> libc::c_int {
    libc::c_int::try_from(constant).expect("I/O policy constants fit in c_int")
}
