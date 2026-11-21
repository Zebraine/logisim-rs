//! Hand the original invocation to a new desktop process after unsaved navigation.
use std::{
    env, io,
    process::{Child, Command, Stdio},
};

const WAIT_FOR_CLOSE: &str = "LOGISIM_RS_RESTART_PIPE";

pub(super) fn spawn() -> io::Result<Child> {
    Command::new(env::current_exe()?)
        .args(env::args_os().skip(1))
        .env(WAIT_FOR_CLOSE, "1")
        .stdin(Stdio::piped())
        .spawn()
}

pub(super) fn wait_for_previous_process() -> io::Result<()> {
    // The parent owns the writing end until shutdown. EOF also releases the child
    // if the parent crashes, in which case normal recovery runs on startup.
    if env::var_os(WAIT_FOR_CLOSE).as_deref() == Some(std::ffi::OsStr::new("1")) {
        io::copy(&mut io::stdin().lock(), &mut io::sink())?;
    }
    Ok(())
}
