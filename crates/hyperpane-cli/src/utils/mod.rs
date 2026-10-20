use crate::tool;
use hyperpane_shell::ErrorKind;
use std::process::Command;

/// Runs `command` and returns its trimmed standard output, or what went wrong
/// as a message.
pub(crate) fn output(command: &mut Command) -> Result<String, String> {
    tool::capture(command, ErrorKind::Toolchain, "")
        .map(|stdout| stdout.trim().to_owned())
        .map_err(|error| error.message)
}
