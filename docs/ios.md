# iOS

`hyperpane run <publication> --ios` and `hyperpane bundle <publication> --ios
--out <dir>` turn a fusor publication into an iOS app for the simulator. There
is no Xcode project: the CLI assembles the bundle itself, and the simulator
installs and runs it unsigned.

## Building the host

The executable is the same `hyperpane-host` that runs in a macOS window,
compiled for the simulator:

```sh
IPHONEOS_DEPLOYMENT_TARGET=17.0 cargo build --release --locked \
  --target aarch64-apple-ios-sim -p hyperpane-shell --bin hyperpane-host
```

The CLI runs this from the hyperpane checkout it was built in, and
`--host-binary <path>` replaces it with an executable you built. tao's event
loop calls `UIApplicationMain`, so the host is an ordinary Rust program with a
`main` function on both platforms. UIKit has no application object until that
loop runs, so the host creates its window and web view on the loop's first
event, on macOS too.

## The bundle

```text
<name>.app/
├── hyperpane-host     the host, built for aarch64-apple-ios-sim
├── Info.plist
├── PkgInfo            APPL????
├── hyperpane.json     the run configuration
└── publication/       a copy of the fusor publication, manifests included
```

`hyperpane.json` holds the host's run configuration. The publication path in
it is relative and resolves against the bundle; scripts given with `--script`
are stored inline:

```json
{
  "source": { "publication": "publication" },
  "name": "Consumer",
  "scripts": []
}
```

`bundle` and `run --ios` replace an existing `<name>.app` only if it contains
`hyperpane.json`, so a bundle hyperpane did not assemble is never removed.
`run --ios` assembles into `hyperpane/` in the system's temporary directory.

### Info.plist

| Key | Value | Why |
| --- | --- | --- |
| `CFBundleIdentifier` | `--bundle-id`, default `build.fusor.hyperpane` | The app's identity. Installing an app with the same identifier replaces it; `simctl launch` and `terminate` address it. |
| `CFBundleExecutable` | `hyperpane-host` | The file iOS starts. |
| `CFBundleName`, `CFBundleDisplayName` | `--name`, default `Hyperpane` | The name under the icon. |
| `CFBundlePackageType` | `APPL` | Marks the bundle as an application. |
| `CFBundleShortVersionString`, `CFBundleVersion` | `1.0`, `1` | Required version keys; fixed until there is a release process. |
| `LSRequiresIPhoneOS` | true | An iOS app. |
| `MinimumOSVersion` | `17.0` | The same release the host is compiled for. fusor states no oldest WebKit ([fusor#66](https://github.com/fusor-rs/fusor/issues/66)); only iOS 27 simulators are tested. |
| `UIDeviceFamily` | 1, 2 | iPhone and iPad. |
| `CFBundleSupportedPlatforms`, `DTPlatformName` | `iPhoneSimulator`, `iphonesimulator` | A simulator build. |
| `DTSDKName` | `iphonesimulator` and the SDK version from `xcrun --sdk iphonesimulator --show-sdk-version` | The SDK the bundle was assembled with. |
| `UILaunchScreen` | an empty dictionary | Without a launch screen iOS runs the app in a 320 by 480 point compatibility window. An empty one shows the system background and gives the app the whole screen. |

## The simulator

`run --ios`:

1. checks the publication (base path `/`, manifest version);
2. builds the host and assembles the bundle;
3. picks a simulator: the one named by `--simulator <name or UDID>`, otherwise a
   booted iPhone, otherwise the first iPhone of the newest iOS runtime;
4. boots it if needed and waits (`xcrun simctl bootstatus <udid> -b`);
5. installs the bundle (`simctl install`) and launches it with
   `simctl launch --console --terminate-running-process`;
6. relays the app's standard output, the host's JSON lines in
   [consumption.md](consumption.md), until the app exits.

`simctl` prints `<bundle id>: <process id>` among those lines. With
`--timeout <seconds>`, the CLI terminates the app and fails if it has not
exited in time; the consumer test uses this. Without it, the CLI returns when
the app exits.

The simulator shares the Mac's loopback interface, but the host's loopback
listener for shared-memory publications runs inside the app, as it must on a
device.

## How the app feels

The host sets, on both platforms:

- link previews off, so a long press on a link does not open a preview;
- no tap highlight and no long-press callout, through an injected style sheet;
- reload on WebContent process termination (see
  [consumption.md](consumption.md)).

wry turns scroll bounce off on iOS, and gives the web view the system
background color, which is what the empty launch screen shows, so there is no
white flash before the first paint in dark mode.

### What your HTML must do

The host does not rewrite your page. For a page that uses the whole screen:

```html
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
```

and keep content out from under the status bar and the home indicator with the
safe-area insets:

```css
body {
  padding: max(1rem, env(safe-area-inset-top)) max(1rem, env(safe-area-inset-right))
    max(1rem, env(safe-area-inset-bottom)) max(1rem, env(safe-area-inset-left));
}
```

Also:

- build with base path `/` (no `base-path` in `[package.metadata.fusor]`);
- declare `history-fallback` for your routes, so a reload after iOS terminated
  the page returns to the same route;
- add `overscroll-behavior: none` on scrolling containers that should not
  bounce;
- use links rather than `window.open`, which does nothing on iOS.

## Milestone 2

Not built yet:

- device builds for `aarch64-apple-ios`, code signing, provisioning profiles
  and entitlements;
- a generated Xcode project and `xcodebuild`, for archives and TestFlight;
- a widget extension, which is how an iPhone app reaches HomeView;
- re-checking App Transport Security for the loopback origin on a device;
- a prebuilt host, so applications need no hyperpane checkout.
