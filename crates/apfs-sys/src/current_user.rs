pub fn current_user_id() -> libc::uid_t {
    unsafe { libc::getuid() }
}
