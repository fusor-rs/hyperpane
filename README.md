# hyperpane

Run [fusor](https://github.com/fusor-rs/fusor) applications as iPhone, iPad and
Mac apps.

fusor compiles HTML templates and Rust into a WebAssembly site. hyperpane takes
what `fusor build` publishes and shows it in Apple's WKWebView, in a window on
macOS and full screen on iOS, from one host program. Nothing in the application
changes: the same `dist/` that a web server would serve runs inside the app.

**Pre-alpha.** This is the first milestone: the host, a CLI, and the iOS
Simulator. Commands and options will change. Device builds, signing and App
Store delivery are not here yet.

<p align="center">
  <img src="docs/ios-consumer.png" alt="The test application running in the iPhone simulator" width="230">
</p>

## What runs where

| | macOS | iOS Simulator | iOS device |
| --- | --- | --- | --- |
| Your fusor application | `hyperpane run` | `hyperpane run --ios` | not yet |
| Development server (`fusor dev`) | `hyperpane run --url …` | no | no |
| App bundle | no | `hyperpane bundle --ios --out …` | not yet |
| Threaded worker pools (`SharedArrayBuffer`) | yes, loopback origin | yes, loopback origin | not yet |

## Install

hyperpane runs on Macs with Apple silicon. It needs Xcode 27 with an iOS 27
Simulator runtime and the fusor CLI; `hyperpane doctor` checks them and prints
the command that fixes each missing one. No release is published yet; until
the first one, install from a checkout as described below.

### Binary release

The installer downloads the latest GitHub release, verifies its SHA-256
checksum and installs `hyperpane` in `~/.hyperpane/bin`, with prebuilt hosts for
the Mac and the iOS Simulator in `~/.hyperpane/hosts`. It needs no Rust
toolchain:

```sh
curl -fsSL https://github.com/fusor-rs/hyperpane/releases/latest/download/install.sh | sh
export PATH="$HOME/.hyperpane/bin:$PATH"
```

Add the `export` line to your shell profile. To select a release, append
`-s -- v0.1.0` to `sh`; `HYPERPANE_INSTALL` on the `sh` command chooses another
directory.

### Upgrade

```sh
hyperpane upgrade
```

`upgrade` installs the latest release the way hyperpane was installed: with
`install.sh` for a binary release, or with `cargo install` for a Cargo
installation. It offers a release only once all of its files are published.

### Cargo

```sh
cargo install hyperpane-cli --locked
```

The first run for each platform installs the matching host, the
`hyperpane-shell` crate at the same version, with Cargo into
`~/.hyperpane/cache`. That needs the Rust target for it, such as
`rustup target add aarch64-apple-ios-sim`.

### From a checkout

`cargo install --path crates/hyperpane-cli --locked` in this repository installs
a CLI that builds its hosts from the checkout; see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Use it

In your fusor application:

```sh
cd path/to/your-app
hyperpane run                         # build with fusor, show it in a macOS window
hyperpane run --ios --name "My App" --bundle-id com.example.myapp
hyperpane run --dist dist             # skip the build and show what fusor already wrote
hyperpane run --url http://127.0.0.1:4173/   # a running `fusor dev`
hyperpane doctor
```

`hyperpane run` finds the application's `Cargo.toml` the way Cargo does, or
takes `--manifest-path`, runs `fusor build` on it and shows the directory fusor
wrote. With `--ios` it builds the host for the simulator, assembles
`<name>.app`, installs it in a booted iPhone simulator (or boots one) and
shows the app's events until it exits. `--simulator <name or UDID>` picks
another device. `FUSOR_BIN` selects a fusor CLI other than the one on `PATH`.

Progress and events are written to standard error as a short log:

```text
▲ hyperpane 0.1.0 · iOS Simulator
○ Building my-app with fusor ...
✓ Built my-app in 4.2s
○ Building hyperpane-host for aarch64-apple-ios-sim ...
✓ Built hyperpane-host in 1.3s
○ Assembling My App.app ...
✓ Assembled My App.app in 7ms
○ Booting iPhone 17 (iOS 27.0) ...
✓ Booted iPhone 17 (iOS 27.0) in 94ms
○ Installing My App.app ...
✓ Installed My App.app in 673ms
✓ Launched My App on iPhone 17 (iOS 27.0)
- App:      com.example.myapp
- UDID:     1C6398AD-7E4D-4CE6-BE55-13ADAE994345
✓ Loaded hyperpane://localhost/
```

When standard output is not a terminal, the host's JSON events
([docs/consumption.md](docs/consumption.md)) are also written there, one per
line, for scripts. `NO_COLOR` turns off color.

The CLI uses the host given with `--host-binary`, otherwise the one shipped
with it, otherwise one built from the checkout it was built from, otherwise one
installed from crates.io.

Your application must be built for the base path `/`, and its HTML should say
how it uses the screen on iOS; see [docs/ios.md](docs/ios.md).

## How it works

- `hyperpane-host` opens a tao window and a wry web view. It serves fusor's
  output from the `hyperpane://localhost` scheme, exact paths only, or,
  when the application uses shared-memory worker pools, from a loopback HTTP
  listener with the cross-origin isolation headers.
- Pages talk to the host with `window.ipc.postMessage`. Links to other web
  origins open in the system browser.
- The host writes one JSON line per event to standard output. The CLI and the
  tests read it.

[docs/consumption.md](docs/consumption.md) is the contract with fusor's output
and with the page; [docs/ios.md](docs/ios.md) covers the app bundle and the
simulator.

## Status and known limitations

- iOS runs in the simulator only. The bundle is unsigned and declares the
  simulator platform.
- `window.open` does nothing on iOS: wry 0.57 handles new-window requests only
  on macOS. Links with `target="_blank"` work on both platforms.
- The minimum iOS version is 17.0. fusor doesn't state the oldest WebKit it
  supports ([fusor#66](https://github.com/fusor-rs/fusor/issues/66)), and only
  iOS 27.0 and 27.1 simulators are tested.
- No menus, file dialogs, clipboard, notifications or other native features
  through the bridge yet.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). hyperpane is MIT licensed.
