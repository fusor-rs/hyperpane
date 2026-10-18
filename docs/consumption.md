# What the host consumes and produces

`hyperpane-host` shows one fusor publication, the directory `fusor build`
writes, or one development server. This page is its contract: what it reads
from the publication, how it serves it, what a page can say to it, and what it
prints.

## The publication

The host reads `.fusor-output.json` and checks it before opening a window:

| Field | Use |
| --- | --- |
| `version` | Must be `1`. Anything else stops the host with a `publication` error. |
| `generation` | Reported in each `ready` event, so a harness can tell which build is running. |
| `base_path` | Must be `/`: the host serves the publication at the root of its origin. Otherwise the host stops; remove `base-path` from `[package.metadata.fusor]` and build again. |
| `history_fallback` | Route prefixes that serve `index.html` (below). Missing means none. |

Other fields are ignored. When `.fusor-worker-headers.json` exists, which fusor
writes for applications with shared-memory worker pools, the host serves from
the loopback origin instead of the custom scheme.

## Serving

By default the page's origin is `hyperpane://localhost`, a custom URL scheme
that only this web view can reach. For each request:

1. Methods other than `GET` and `HEAD` get 405.
2. The path is percent-decoded. A path with a `..` segment, a backslash, a null
   byte, invalid UTF-8 or a segment starting with `.` gets 404, so the output
   manifests are never served.
3. An existing file is served with status 200. A directory serves its
   `index.html`.
4. Otherwise, a request whose `Accept` header contains `text/html`, for a path
   outside `__fusor/` with no `.` in any segment, under a declared
   `history_fallback` prefix (`/` covers every path; `/details` covers
   `/details` and `/details/…`), gets the root `index.html` with status 200.
5. Everything else is 404. A missing script or Wasm file is never answered with
   `index.html`.

Content types follow fusor's own development server:

| Extension | Content type |
| --- | --- |
| `html` | `text/html; charset=utf-8` |
| `js`, `mjs` | `text/javascript; charset=utf-8` |
| `css` | `text/css; charset=utf-8` |
| `wasm` | `application/wasm` |
| `json`, `map` | `application/json` |
| `txt` | `text/plain; charset=utf-8` |
| `md` | `text/markdown; charset=utf-8` |
| `svg` | `image/svg+xml` |
| `png` | `image/png` |
| `jpg`, `jpeg` | `image/jpeg` |
| `webp` | `image/webp` |
| `ico` | `image/x-icon` |
| `woff` | `font/woff` |
| `woff2` | `font/woff2` |
| anything else | `application/octet-stream` |

fusor does not document this table yet
([fusor#67](https://github.com/fusor-rs/fusor/issues/67)).

### The loopback rule

WebKit exposes `SharedArrayBuffer`, which fusor's worker pools need, only on an
HTTP origin that sends the cross-origin isolation headers. On the custom
scheme it is undefined on both platforms, even where iOS reports
`crossOriginIsolated` as true, so a check must look at `SharedArrayBuffer`
itself.

For a publication with `.fusor-worker-headers.json`, the host therefore listens
on `127.0.0.1` at a port the system chooses, inside the host process, and loads
`http://127.0.0.1:<port>/`. The same rules as above apply, every response also
carries

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

and every connection closes after one response. The listener lives as long as
the host. In the iOS Simulator no App Transport Security exception was needed
for this origin.

### Development servers

`hyperpane-host --url <address>` (`hyperpane run --url` in the CLI, macOS only)
loads a running server such as `fusor dev` instead of a publication. The
server serves the pages; the rest of this page still applies.

## Inside the page

Before the page's own scripts, the host injects, in order:

1. a style sheet on the root that turns off the tap highlight and the
   long-press callout (`-webkit-tap-highlight-color: transparent`,
   `-webkit-touch-callout: none`);
2. a click handler that turns a link with `target="_blank"`, when the page has
   not handled the click itself, into a navigation of the current page;
3. each script passed with `--script`, in the order given.

### Navigation

A navigation to the page's own origin, or to a non-web URL such as `about:` or
`blob:`, stays in the web view. A navigation to an `http` or `https` URL on any
other origin is cancelled and the URL opens in the system browser, Safari on
iOS. On macOS, `window.open` follows the same rule for other origins and does
nothing for the page's own origin; on iOS it does nothing.

### Messages to the host

A page sends a message with

```js
window.ipc.postMessage(JSON.stringify(message));
```

The host reads one shape:

| `type` | Fields | Effect |
| --- | --- | --- |
| `report` | `report`: any JSON value | The host prints a `report` event and exits with status 0. Test harnesses use it to hand back what they observed. |

Anything else produces an `error` event and is otherwise ignored. The host
sends nothing to the page yet.

## The host's output

Standard output carries one JSON object per line, one per event, and nothing
else:

| `event` | Fields | When |
| --- | --- | --- |
| `ready` | `url`; `generation` for a publication | A page finished loading, including after a reload. |
| `report` | `report` | The page posted a report. The host exits next. |
| `error` | `kind`, `message`, `remedy` | Something failed. Before the window opens, the host then exits with status 1. |

```text
{"event":"ready","url":"hyperpane://localhost/","generation":"g-1792285523955963000-65780"}
{"event":"report","report":{"worker":"Worker answered 42"}}
{"event":"error","kind":"publication","message":"the application was built for base path /docs/, but the host serves it at /","remedy":"remove `base-path` from [package.metadata.fusor] in Cargo.toml and run `fusor build` again"}
```

`kind` is one of `configuration`, `publication`, `host`, `web-view`,
`loopback`, `toolchain`, `simulator` and `bundle`. Standard error carries
diagnostics for people, such as a failed loopback request, and is not part of
the contract.

The host exits with status 0 when its window closes (macOS) or after a report,
and 1 when it cannot start.

If WebKit terminates the page's process, which iOS does to apps in the
background, the host reloads the current URL and prints `ready` again. The
route comes back through the history fallback; state held only in the page
does not.

## Run configuration

The host takes one configuration: where the pages come from, a name, and the
scripts to inject. On macOS it comes from the command line:

```text
hyperpane-host [--name <name>] [--script <file>]... (<publication-dir> | --url <address>)
```

An iOS app receives no arguments, so there it comes from `hyperpane.json` next
to the executable, which the CLI writes; see [ios.md](ios.md).
