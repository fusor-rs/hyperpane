use crate::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The file in an iOS app bundle that holds its serialized [`RunConfig`].
pub const BUNDLE_CONFIG: &str = "hyperpane.json";

/// The name an app gets when none is given.
pub const DEFAULT_NAME: &str = "Hyperpane";

/// Everything one run of the host needs. On macOS it comes from the command
/// line; on iOS, from [`BUNDLE_CONFIG`] next to the executable.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunConfig {
    pub source: Source,
    /// The window title.
    pub name: String,
    /// JavaScript sources injected into every page before the page's own scripts.
    pub scripts: Vec<String>,
}

/// Where the pages come from.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// A directory published by `fusor build`. In a bundle's configuration, a
    /// relative path resolves against the bundle.
    Publication(PathBuf),
    /// A running development server, such as `fusor dev`.
    Url(String),
}

/// Reads a script to inject, for [`RunConfig::scripts`].
pub fn read_script(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorKind::Configuration,
            format!("cannot read the script {}: {error}", path.display()),
            "pass a readable JavaScript file to --script",
        )
    })
}

#[cfg(not(target_os = "ios"))]
impl RunConfig {
    pub(crate) fn load() -> Result<Self, Error> {
        use clap::Parser;

        #[derive(Parser)]
        #[command(
            name = "hyperpane-host",
            version,
            about = "Show a fusor application in a window"
        )]
        struct Arguments {
            /// The directory that `fusor build` wrote
            #[arg(required_unless_present = "url", conflicts_with = "url")]
            directory: Option<PathBuf>,
            /// Open this development server instead
            #[arg(long)]
            url: Option<String>,
            /// The window title
            #[arg(long, default_value = DEFAULT_NAME)]
            name: String,
            /// A JavaScript file to inject into every page; repeatable
            #[arg(long = "script")]
            scripts: Vec<PathBuf>,
        }

        let arguments = Arguments::parse();
        let source = arguments
            .url
            .map(Source::Url)
            .or(arguments.directory.map(Source::Publication))
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Configuration,
                    "nothing to show",
                    "pass the directory that `fusor build` wrote, or --url <address>",
                )
            })?;
        let scripts = arguments
            .scripts
            .iter()
            .map(|path| read_script(path))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            source,
            name: arguments.name,
            scripts,
        })
    }
}

#[cfg(target_os = "ios")]
impl RunConfig {
    pub(crate) fn load() -> Result<Self, Error> {
        let invalid_bundle = |message: String| {
            Error::new(
                ErrorKind::Configuration,
                message,
                "rebuild the app with `hyperpane bundle --ios` or `hyperpane run --ios`",
            )
        };
        let executable = std::env::current_exe()
            .map_err(|error| invalid_bundle(format!("cannot locate the app bundle: {error}")))?;
        let bundle = executable
            .parent()
            .ok_or_else(|| invalid_bundle("the executable is not inside an app bundle".into()))?;
        let path = bundle.join(BUNDLE_CONFIG);
        let bytes = fs::read(&path)
            .map_err(|error| invalid_bundle(format!("cannot read {}: {error}", path.display())))?;
        let mut config: Self = serde_json::from_slice(&bytes)
            .map_err(|error| invalid_bundle(format!("{} is invalid: {error}", path.display())))?;
        if let Source::Publication(publication) = &mut config.source {
            *publication = bundle.join(&*publication);
        }
        Ok(config)
    }
}
