use crate::{Error, ErrorKind};
use http::{
    Method, Request, Response, StatusCode,
    header::{ACCEPT, CONTENT_TYPE},
};
use serde::Deserialize;
use std::{
    borrow::Cow,
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

const OUTPUT_MANIFEST: &str = ".fusor-output.json";
const OUTPUT_MANIFEST_VERSION: u32 = 1;
/// fusor writes it only for publications whose workers share memory.
const WORKER_HEADERS: &str = ".fusor-worker-headers.json";
const GENERATED: &str = "__fusor";
const INDEX: &str = "index.html";
const SERVED_BASE_PATH: &str = "/";

/// The content types fusor's own development and preview servers send.
/// fusor documents them nowhere else: https://github.com/fusor-rs/fusor/issues/67
const CONTENT_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("wasm", "application/wasm"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("txt", "text/plain; charset=utf-8"),
    ("md", "text/markdown; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("ico", "image/x-icon"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
];
const UNKNOWN_CONTENT_TYPE: &str = "application/octet-stream";
const PLAIN_TEXT: &str = "text/plain; charset=utf-8";

pub(crate) type Asset = Response<Cow<'static, [u8]>>;

#[derive(Deserialize)]
struct OutputManifest {
    version: u32,
    generation: String,
    base_path: String,
    #[serde(default)]
    history_fallback: Vec<String>,
}

/// Which origin a publication must be served from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `hyperpane://localhost`, reachable only from the web view.
    CustomScheme,
    /// A loopback HTTP listener sending the cross-origin isolation headers,
    /// the only origin on which WebKit exposes `SharedArrayBuffer`.
    Loopback,
}

/// A directory published by `fusor build`, checked against what the host can serve.
#[derive(Debug)]
pub struct Publication {
    root: PathBuf,
    generation: String,
    history_fallback: Vec<String>,
    origin: Origin,
}

impl Publication {
    /// Reads the output manifest in `root`. Fails when it is missing or
    /// unsupported, or when the application was built for a base path other than `/`.
    pub fn open(root: &Path) -> Result<Self, Error> {
        let path = root.join(OUTPUT_MANIFEST);
        let unreadable = |message: String| {
            Error::new(
                ErrorKind::Publication,
                message,
                "pass the output directory of `fusor build`, usually dist/",
            )
        };
        let bytes = fs::read(&path)
            .map_err(|error| unreadable(format!("cannot read {}: {error}", path.display())))?;
        let manifest: OutputManifest = serde_json::from_slice(&bytes)
            .map_err(|error| unreadable(format!("{} is invalid: {error}", path.display())))?;
        if manifest.version != OUTPUT_MANIFEST_VERSION {
            return Err(Error::new(
                ErrorKind::Publication,
                format!(
                    "{} has version {}; this host reads version {OUTPUT_MANIFEST_VERSION}",
                    path.display(),
                    manifest.version
                ),
                "build the application with a fusor CLI that matches this hyperpane",
            ));
        }
        if manifest.base_path != SERVED_BASE_PATH {
            return Err(Error::new(
                ErrorKind::Publication,
                format!(
                    "the application was built for base path {}, but the host serves it at {SERVED_BASE_PATH}",
                    manifest.base_path
                ),
                "remove `base-path` from [package.metadata.fusor] in Cargo.toml and run `fusor build` again",
            ));
        }
        let origin = if root.join(WORKER_HEADERS).is_file() {
            Origin::Loopback
        } else {
            Origin::CustomScheme
        };
        Ok(Self {
            root: root.to_path_buf(),
            generation: manifest.generation,
            history_fallback: manifest.history_fallback,
            origin,
        })
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// Exact files only. A directory serves its `index.html`, and a document
    /// request under a declared history fallback prefix serves the root
    /// `index.html`; everything else is 404.
    pub(crate) fn respond<T>(&self, request: &Request<T>) -> Asset {
        let url_path = request.uri().path();
        if !matches!(*request.method(), Method::GET | Method::HEAD) {
            return self.response(StatusCode::METHOD_NOT_ALLOWED, PLAIN_TEXT, Vec::new());
        }
        let Some(relative) = decode_path(url_path) else {
            return self.not_found(url_path);
        };
        let mut file = self.root.join(&relative);
        if file.is_dir() {
            file.push(INDEX);
        }
        if !file.is_file() && accepts_html(request) && self.falls_back(&relative) {
            file = self.root.join(INDEX);
        }
        match fs::read(&file) {
            Ok(body) => self.response(StatusCode::OK, content_type(&file), body),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                self.not_found(url_path)
            }
            Err(error) => self.response(
                StatusCode::INTERNAL_SERVER_ERROR,
                PLAIN_TEXT,
                error.to_string().into_bytes(),
            ),
        }
    }

    /// Only for paths that do not look like files, outside generated output,
    /// under a prefix the application declared.
    fn falls_back(&self, relative: &Path) -> bool {
        if relative.starts_with(GENERATED)
            || relative
                .iter()
                .any(|part| part.to_string_lossy().contains('.'))
        {
            return false;
        }
        let path = format!("/{}", relative.to_string_lossy());
        self.history_fallback.iter().any(|prefix| {
            prefix == SERVED_BASE_PATH
                || path == *prefix
                || path
                    .strip_prefix(prefix.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    fn not_found(&self, path: &str) -> Asset {
        self.response(
            StatusCode::NOT_FOUND,
            PLAIN_TEXT,
            format!("not found: {path}").into_bytes(),
        )
    }

    fn response(&self, status: StatusCode, content_type: &str, body: Vec<u8>) -> Asset {
        let mut response = Response::builder()
            .status(status)
            .header(CONTENT_TYPE, content_type);
        if self.origin == Origin::Loopback {
            response = response
                .header("Cross-Origin-Opener-Policy", "same-origin")
                .header("Cross-Origin-Embedder-Policy", "require-corp");
        }
        response
            .body(Cow::Owned(body))
            .expect("the status and header values are valid constants")
    }
}

fn accepts_html<T>(request: &Request<T>) -> bool {
    request
        .headers()
        .get(ACCEPT)
        .is_some_and(|accept| String::from_utf8_lossy(accept.as_bytes()).contains("text/html"))
}

fn content_type(file: &Path) -> &'static str {
    let extension = file.extension().and_then(OsStr::to_str);
    CONTENT_TYPES
        .iter()
        .find(|(known, _)| Some(*known) == extension)
        .map_or(UNKNOWN_CONTENT_TYPE, |(_, content_type)| content_type)
}

/// Percent-decodes a URL path into a path relative to the publication.
/// Rejects malformed escapes, invalid UTF-8, traversal, backslashes, null
/// bytes and dotfiles rather than normalizing them, so the output manifests
/// are never served.
fn decode_path(url_path: &str) -> Option<PathBuf> {
    let mut encoded = url_path.trim_start_matches('/').bytes();
    let mut decoded = Vec::new();
    while let Some(byte) = encoded.next() {
        if byte != b'%' {
            decoded.push(byte);
            continue;
        }
        let high = char::from(encoded.next()?).to_digit(16)?;
        let low = char::from(encoded.next()?).to_digit(16)?;
        decoded.push((high * 16 + low) as u8);
    }
    let decoded = String::from_utf8(decoded).ok()?;
    if decoded.contains(['\0', '\\']) {
        return None;
    }
    let path = PathBuf::from(decoded);
    let visible = path.components().all(|part| match part {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    visible.then_some(path)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Origin, Publication};
    use crate::ErrorKind;
    use http::{
        Request,
        header::{ACCEPT, CONTENT_TYPE},
    };
    use std::{fs, path::Path};

    pub(crate) const MANIFEST: &str =
        r#"{"version":1,"generation":"g-1","base_path":"/","history_fallback":["/details"]}"#;
    const DOCUMENT: &str = "text/html,application/xhtml+xml;q=0.9";

    pub(crate) fn publish(root: &Path, files: &[(&str, &str)]) {
        for (path, contents) in files {
            let file = root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, contents).unwrap();
        }
    }

    #[test]
    fn serves_exact_files_directory_indexes_and_declared_routes_only() {
        let root = tempfile::tempdir().unwrap();
        publish(
            root.path(),
            &[
                (".fusor-output.json", MANIFEST),
                ("index.html", "home"),
                ("docs/index.html", "docs"),
                ("hello world.txt", "hello"),
                ("__fusor/g-1/pkg/app_bg.wasm", "wasm"),
            ],
        );
        let publication = Publication::open(root.path()).unwrap();
        assert_eq!(publication.origin(), Origin::CustomScheme);
        let respond = |path: &str, accept: &str| {
            let request = Request::get(path).header(ACCEPT, accept).body(()).unwrap();
            publication.respond(&request)
        };
        for (path, accept, content_type, body) in [
            ("/", DOCUMENT, "text/html; charset=utf-8", "home"),
            ("/docs/", "*/*", "text/html; charset=utf-8", "docs"),
            (
                "/hello%20world.txt",
                "*/*",
                "text/plain; charset=utf-8",
                "hello",
            ),
            (
                "/__fusor/g-1/pkg/app_bg.wasm",
                "*/*",
                "application/wasm",
                "wasm",
            ),
            ("/details", DOCUMENT, "text/html; charset=utf-8", "home"),
            ("/details/42", DOCUMENT, "text/html; charset=utf-8", "home"),
        ] {
            let response = respond(path, accept);
            assert_eq!(response.status(), 200, "{path}");
            assert_eq!(response.headers()[CONTENT_TYPE], content_type, "{path}");
            assert_eq!(response.body().as_ref(), body.as_bytes(), "{path}");
            assert_eq!(response.headers().len(), 1, "{path}");
        }
        for (path, accept) in [
            ("/details/42", "*/*"),
            ("/details/app.js", DOCUMENT),
            ("/detailsx", DOCUMENT),
            ("/__fusor/g-1/pkg/missing", DOCUMENT),
            ("/missing.js", "*/*"),
            ("/.fusor-output.json", "*/*"),
            ("/%2e%2e/index.html", DOCUMENT),
            ("/docs%5cindex.html", "*/*"),
        ] {
            assert_eq!(respond(path, accept).status(), 404, "{path}");
        }
    }

    #[test]
    fn rejects_publications_it_cannot_serve() {
        for manifest in [
            None,
            Some("{"),
            Some(r#"{"version":2,"generation":"g-1","base_path":"/"}"#),
            Some(r#"{"version":1,"generation":"g-1","base_path":"/app/"}"#),
        ] {
            let root = tempfile::tempdir().unwrap();
            if let Some(manifest) = manifest {
                publish(root.path(), &[(".fusor-output.json", manifest)]);
            }
            let error = Publication::open(root.path()).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Publication, "{manifest:?}");
        }
    }
}
