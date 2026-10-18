//! Everything the CLI prints. The host's output is relayed to standard output
//! unchanged; the CLI's own messages go to standard error.
use hyperpane_shell::Error;

pub(crate) fn step(message: &str) {
    eprintln!("hyperpane: {message}");
}

pub(crate) fn relay(line: &str) {
    println!("{line}");
}

pub(crate) fn failure(error: &Error) {
    eprintln!("hyperpane: {error}");
}

pub(crate) fn check(name: &str, outcome: &Result<String, String>, remedy: &str) {
    match outcome {
        Ok(detail) => println!("ok    {name}: {detail}"),
        Err(problem) => println!("fail  {name}: {problem}\n      fix: {remedy}"),
    }
}
