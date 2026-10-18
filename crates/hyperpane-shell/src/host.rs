use crate::{
    Error, ErrorKind,
    bridge::{self, HostEvent},
    config::{RunConfig, Source},
    loopback,
    publication::{Origin, Publication},
};
use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder, WebViewBuilderExtDarwin};

const SCHEME: &str = "hyperpane";
const SCHEME_ORIGIN: &str = "hyperpane://localhost";
/// Removes the tap highlight and the long-press callout, which mark a page as
/// a website rather than an app.
const FEEL_SCRIPT: &str = include_str!("feel.js");

enum Command {
    Reload,
}

/// Where the web view loads pages from.
enum Served {
    Scheme(Publication),
    Loopback { origin: String, generation: String },
    Url(String),
}

impl Served {
    fn open(source: &Source) -> Result<Self, Error> {
        let root = match source {
            Source::Url(url) => return Ok(Self::Url(url.clone())),
            Source::Publication(root) => root,
        };
        let publication = Publication::open(root)?;
        match publication.origin() {
            Origin::CustomScheme => Ok(Self::Scheme(publication)),
            Origin::Loopback => {
                let generation = publication.generation().to_owned();
                let address = loopback::serve(publication)?;
                Ok(Self::Loopback {
                    origin: format!("http://{address}"),
                    generation,
                })
            }
        }
    }

    fn origin(&self) -> &str {
        match self {
            Self::Scheme(_) => SCHEME_ORIGIN,
            Self::Loopback { origin, .. } => origin,
            Self::Url(url) => url_origin(url),
        }
    }

    fn start_url(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Scheme(_) | Self::Loopback { .. } => format!("{}/", self.origin()),
        }
    }

    fn generation(&self) -> Option<&str> {
        match self {
            Self::Scheme(publication) => Some(publication.generation()),
            Self::Loopback { generation, .. } => Some(generation),
            Self::Url(_) => None,
        }
    }
}

/// The application's state between event loop callbacks.
pub(crate) struct Host {
    pending: Option<(Served, RunConfig)>,
    view: Option<(Window, WebView)>,
}

impl Host {
    pub(crate) fn new(config: RunConfig) -> Result<Self, Error> {
        Ok(Self {
            pending: Some((Served::open(&config.source)?, config)),
            view: None,
        })
    }

    /// The window and web view are created on `StartCause::Init`: on iOS,
    /// UIKit has no application object until the event loop has started.
    pub(crate) fn run(mut self) -> ! {
        let event_loop = EventLoopBuilder::with_user_event().build();
        let proxy = event_loop.create_proxy();
        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::NewEvents(StartCause::Init) => {
                    if let Err(error) = self.open(target, proxy.clone()) {
                        bridge::emit(&HostEvent::Error(&error));
                        *control_flow = ControlFlow::ExitWithCode(1);
                    }
                }
                Event::UserEvent(Command::Reload) => self.reload(),
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => {}
            }
        })
    }

    fn open(
        &mut self,
        target: &EventLoopWindowTarget<Command>,
        proxy: EventLoopProxy<Command>,
    ) -> Result<(), Error> {
        let Some((served, config)) = self.pending.take() else {
            return Ok(());
        };
        let window = WindowBuilder::new()
            .with_title(&config.name)
            .build(target)
            .map_err(|error| {
                Error::new(
                    ErrorKind::WebView,
                    format!("cannot open a window: {error}"),
                    "run the host from a logged-in desktop session",
                )
            })?;
        let webview = web_view(served, config, proxy)
            .build(&window)
            .map_err(|error| {
                Error::new(
                    ErrorKind::WebView,
                    format!("cannot create the web view: {error}"),
                    "check that WebKit is available to this process",
                )
            })?;
        self.view = Some((window, webview));
        Ok(())
    }

    /// Reloads after WebKit terminated the page's process, which iOS does to
    /// background apps; the history fallback brings back the current route.
    fn reload(&self) {
        let Some((_, webview)) = &self.view else {
            return;
        };
        if let Err(error) = webview.reload() {
            bridge::emit(&HostEvent::Error(&Error::new(
                ErrorKind::WebView,
                format!("cannot reload the page: {error}"),
                "restart the app",
            )));
        }
    }
}

/// wry's iOS web view background is the system background colour, which is
/// what an empty `UILaunchScreen` shows, so there is no flash before the
/// first paint. Setting a fixed colour here would ignore dark mode.
fn web_view(
    served: Served,
    config: RunConfig,
    proxy: EventLoopProxy<Command>,
) -> WebViewBuilder<'static> {
    let origin = served.origin().to_owned();
    let window_origin = origin.clone();
    let generation = served.generation().map(str::to_owned);
    let mut builder = WebViewBuilder::new()
        .with_url(served.start_url())
        .with_initialization_script(FEEL_SCRIPT)
        .with_allow_link_preview(false)
        .with_on_web_content_process_terminate_handler(move || {
            // Fails only after the event loop has exited, when nothing is left to reload.
            let _ = proxy.send_event(Command::Reload);
        })
        .with_ipc_handler(|request| bridge::receive(request.body()))
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) {
                bridge::emit(&HostEvent::Ready {
                    url: &url,
                    generation: generation.as_deref(),
                });
            }
        })
        .with_navigation_handler(move |url| {
            let leaves = leaves_app(&url, &origin);
            if leaves {
                open_in_browser(&url);
            }
            !leaves
        })
        .with_new_window_req_handler(move |url, _features| {
            if leaves_app(&url, &window_origin) {
                open_in_browser(&url);
            }
            NewWindowResponse::Deny
        });
    for script in config.scripts {
        builder = builder.with_initialization_script(script);
    }
    if let Served::Scheme(publication) = served {
        builder = builder.with_custom_protocol(SCHEME.into(), move |_, request| {
            publication.respond(&request)
        });
    }
    builder
}

/// Web links to other origins open in the system browser; everything else,
/// including `about:` and `blob:` frames, stays in the web view.
fn leaves_app(url: &str, origin: &str) -> bool {
    let same_origin = url
        .strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']));
    let web = url.starts_with("http://") || url.starts_with("https://");
    web && !same_origin
}

fn open_in_browser(url: &str) {
    if let Err(error) = webbrowser::open(url) {
        bridge::emit(&HostEvent::Error(&Error::new(
            ErrorKind::WebView,
            format!("cannot open {url} in the system browser: {error}"),
            "set a default web browser",
        )));
    }
}

fn url_origin(url: &str) -> &str {
    let authority = url.find("://").map_or(0, |index| index + "://".len());
    url[authority..]
        .find(['/', '?', '#'])
        .map_or(url, |end| &url[..authority + end])
}

#[cfg(test)]
mod tests {
    use super::{SCHEME_ORIGIN, leaves_app, url_origin};

    #[test]
    fn only_web_links_to_other_origins_leave_the_app() {
        let loopback = "http://127.0.0.1:5000";
        let development = url_origin("http://127.0.0.1:4173/docs/?page=1");
        assert_eq!(development, "http://127.0.0.1:4173");
        for (url, origin, leaves) in [
            ("hyperpane://localhost/details?tab=1", SCHEME_ORIGIN, false),
            ("about:blank", SCHEME_ORIGIN, false),
            ("blob:hyperpane://localhost/1d2e", SCHEME_ORIGIN, false),
            ("https://fusor.build/docs/", SCHEME_ORIGIN, true),
            ("http://127.0.0.1:5000", loopback, false),
            ("http://127.0.0.1:5000/#top", loopback, false),
            ("http://127.0.0.1:50001/", loopback, true),
            ("http://127.0.0.1:4173/docs/routing", development, false),
            ("http://127.0.0.1:4174/", development, true),
        ] {
            assert_eq!(leaves_app(url, origin), leaves, "{url} from {origin}");
        }
    }
}
