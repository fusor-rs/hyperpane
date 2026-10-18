use hyperpane_shell::{Error, ErrorKind};
use std::process::Command;

/// Runs `command` to completion and returns its standard output.
pub(crate) fn capture(
    command: &mut Command,
    kind: ErrorKind,
    remedy: &str,
) -> Result<String, Error> {
    let output = command.output().map_err(|error| {
        Error::new(
            kind,
            format!("cannot run {}: {error}", describe(command)),
            remedy,
        )
    })?;
    if !output.status.success() {
        return Err(Error::new(
            kind,
            format!(
                "{} failed with {}: {}",
                describe(command),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            remedy,
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn describe(command: &Command) -> String {
    let words: Vec<_> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|word| word.to_string_lossy())
        .collect();
    format!("`{}`", words.join(" "))
}
