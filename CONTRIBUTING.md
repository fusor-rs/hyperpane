# Contributing

hyperpane is pre-alpha. Coding agents and people follow `AGENTS.md`, which is
the code standard for every change.

## Setting up

You need a Mac with Apple silicon and:

- Xcode 27 or later, selected and with its license accepted, and an iOS 27
  Simulator runtime;
- Rust 1.85 or later through rustup, with the `aarch64-apple-ios-sim`,
  `aarch64-apple-ios` and `wasm32-unknown-unknown` targets, and the 1.85
  toolchain for `just msrv`;
- `just`;
- the fusor repository checked out next to this one (below).

```sh
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -license accept
xcodebuild -downloadPlatform iOS
rustup target add aarch64-apple-ios-sim aarch64-apple-ios wasm32-unknown-unknown
rustup toolchain install 1.85 --profile minimal --target aarch64-apple-ios-sim
```

Then, with fusor checked out next to this repository (below), run `just setup`
once; it downloads the tools fusor builds with. `just doctor` checks the Xcode,
simulator and Rust parts and prints the command that fixes each missing one.

### fusor next to hyperpane

Until the host is proven, the test application uses fusor from source:

```text
workspace/
├── hyperpane/     this repository
└── rust-front/    a checkout of fusor-rs/fusor
```

`tests/consumer/Cargo.toml` takes fusor's crates by path from
`../../../rust-front/crates/`, and the justfile runs fusor's CLI from the same
checkout with `cargo run --manifest-path ../rust-front/Cargo.toml`, so the
crates and the CLI that builds them always come from the same source. A
`fusor` installed on your `PATH` is not used by the checks. CI checks out
fusor-rs/fusor at a pinned commit into the same place; moving that pin means
refreshing `tests/consumer/Cargo.lock` in the same change.

Switching to published fusor crates is a later, deliberate change: exact
version pins replace the path dependencies, the justfile runs a pinned fusor
CLI, and the sibling checkout leaves CI.

## Checks

Run `just check` before considering a change complete:

| Recipe | What it runs |
| --- | --- |
| `just lint` | rustfmt, workspace tests, Clippy for macOS and the iOS Simulator, a check for the iOS device target, Clippy for the consumer's Wasm, rustdoc |
| `just msrv` | the tests and the iOS Simulator check on Rust 1.85 |
| `just consumer` | builds `tests/consumer` with fusor, then runs it in a macOS window |
| `just consumer-ios` | builds `tests/consumer`, then bundles, installs and runs it in the iOS Simulator |

The consumer tests launch real WebKit. `just consumer` opens a window for a few
seconds; `just consumer-ios` uses a booted iPhone simulator, or boots the first
iPhone of the newest iOS runtime.

A change to what the host serves, the bridge or the bundle needs both consumer
tests. The loopback mode for shared-memory publications is not in the consumer
test, because building one needs fusor's nightly toolchain; a change to it
needs a manual run on both platforms of a fusor application with a
`#[fusor_worker::task(pool)]` task, reported in the pull request.

## fusor gaps

hyperpane is the first consumer of fusor's published output outside a browser,
so it finds things fusor doesn't do or doesn't document. For each one:

1. Reproduce it minimally and read fusor's source to confirm it is fusor.
2. Open an issue in fusor-rs/fusor stating what hyperpane needed, what fusor
   does, the reproduction, the workaround here and a proposed fix.
3. Implement the narrowest workaround here, with a one-line comment carrying
   the issue URL. It stays until a fusor release with the fix is used.

Never change the fusor checkout from this repository, and never copy fusor code
here.

## Pull requests

One change per pull request. Describe the problem, the behavior change and the
commands you ran on which platforms. Commit messages are one short imperative
sentence; no AI attribution trailers.

## Using AI tools

You're welcome to use AI tools, but you're responsible for everything you
submit. Read and understand every line, make sure the tests you add would fail
without your change, and be ready to explain your reasoning in review.
