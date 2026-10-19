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
`../../../rust-front/crates/`. The justfile builds fusor's CLI from the same
checkout and points hyperpane at it with `FUSOR_BIN`, so the crates and the CLI
that builds them always come from the same source. A `fusor` installed on your
`PATH` is not used by the checks. CI checks out
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
| `just consumer` | `hyperpane run --manifest-path tests/consumer/Cargo.toml`: builds the consumer with fusor and runs it in a macOS window |
| `just consumer-ios` | the same with `--ios`: builds, bundles, installs and runs it in the iOS Simulator |

The consumer tests launch real WebKit. `just consumer` opens a window for a few
seconds; `just consumer-ios` uses a booted iPhone simulator, or boots the first
iPhone of the newest iOS runtime.

A change to what the host serves, the bridge or the bundle needs both consumer
tests. The loopback mode for shared-memory publications is not in the consumer
test, because building one needs fusor's nightly toolchain; a change to it
needs a manual run on both platforms of a fusor application with a
`#[fusor_worker::task(pool)]` task, reported in the pull request.

## Releasing

Every crate shares one version: `version` under `[workspace.package]` in the
root `Cargo.toml`. A release publishes `hyperpane-shell` and `hyperpane-cli` at
that version and attaches an archive for Macs with Apple silicon.

1. Bump the version and the `=x.y.z` pin of `hyperpane-shell` under
   `[workspace.dependencies]`, run `just check` so `Cargo.lock` follows, and
   merge that to `main` with CI passing.
2. On GitHub, publish a release whose tag is the version with a `v`, such as
   `v0.2.0`, on that commit.

The [Release workflow](.github/workflows/release.yml) then runs Check, checks
that the tag matches the version and runs `just archive`, `just archive-check`
and `just upgrade-check` on the `xcode-27` runner:

- `just archive` builds the CLI and the macOS host, builds the iOS Simulator
  host through the CLI's own build path, and packages them as
  `hyperpane-VERSION-aarch64-apple-darwin.tar.gz`, holding `bin/hyperpane`,
  `hosts/<target>/hyperpane-host`, `LICENSE` and `README.md`, with a `.sha256`
  file.
- `just archive-check` installs that archive with [install.sh](install.sh) from
  the local files (`HYPERPANE_DOWNLOAD_BASE`), checks the installed version,
  assembles an app with the installed CLI and checks that it carries the
  shipped host, and checks that a corrupt download is refused.
- `just upgrade-check` builds the next patch version from a copy of the
  checkout, serves both archives and a release description from local files
  (`HYPERPANE_RELEASE_URL`), and checks that `hyperpane upgrade` refuses an
  incomplete release, upgrades a release installation to the new version and
  its hosts, and then reports it current.

Once the archive passes, the workflow attaches it to the release and publishes
the crates to crates.io from the `crates-io` environment, whose
`CARGO_REGISTRY_TOKEN` secret needs the `publish-new` and `publish-update`
scopes. It verifies the packaged workspace first and skips crates already
published at this version, so a rerun finishes a release that stopped halfway.
After both, it attaches `install.sh`, which marks the release complete; the
installer's URL is the latest release's `install.sh`, and `hyperpane upgrade`
offers a release only once it is attached.

Running the workflow by hand from the Actions tab builds and checks the archive
and keeps it as a workflow artifact, publishing nothing. Both recipes also run
locally on a Mac with Xcode 27.

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
