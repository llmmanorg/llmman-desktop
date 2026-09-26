//! llmman's web UI in a native window over a local `llmman serve`.
//!
//! Nothing here knows about models or chat; that is all llmman's `webui/`.
//! This is only what a browser tab would be, plus the daemon: reuse the one
//! already answering on 127.0.0.1:17434 (a CLI-started daemon, say), or
//! run `llmman serve` as a child for as long as the window is open.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;

use std::sync::{Arc, Mutex};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, WebViewBuilder};

use daemon::{Daemon, Status, URL};

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<Status>::with_user_event().build();
    #[cfg(target_os = "macos")]
    let _menu = macos_menu();
    let window = WindowBuilder::new()
        .with_title("llmman")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 800.0))
        .build(&event_loop)
        .expect("create window");

    let daemon = Arc::new(Mutex::new(Daemon::default()));
    let proxy = event_loop.create_proxy();
    let start = {
        let daemon = daemon.clone();
        move |proxy: EventLoopProxy<Status>| {
            let daemon = daemon.clone();
            std::thread::spawn(move || Daemon::start(&daemon, |s| drop(proxy.send_event(s))));
        }
    };

    let builder = WebViewBuilder::new()
        .with_html(status_page(&Status::Starting(String::new())))
        // Our origin stays in the window; other web pages go to the
        // browser. blob:, data: and about: (the status page) are the UI's own.
        .with_navigation_handler(|url| {
            let ours = url.starts_with(URL) || !is_web(&url);
            if !ours {
                open_externally(&url);
            }
            ours
        })
        // target=_blank links in replies, and the Shell tab's clicked URLs.
        .with_new_window_req_handler(|url, _| {
            open_externally(&url);
            NewWindowResponse::Deny
        })
        .with_ipc_handler({
            let (start, proxy) = (start.clone(), proxy.clone());
            move |req| {
                if req.body() == "retry" {
                    start(proxy.clone());
                }
            }
        });
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window)?;
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().expect("gtk vbox"))?
    };

    start(proxy);
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Status::Ready) => drop(webview.load_url(URL)),
            Event::UserEvent(status) => drop(webview.load_html(&status_page(&status))),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::LoopDestroyed => daemon.lock().unwrap().stop(),
            _ => {}
        }
    });
}

fn is_web(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// The system browser; only web URLs, never a file: or custom scheme.
fn open_externally(url: &str) {
    if !is_web(url) {
        return;
    }
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");
    let _ = cmd.arg(url).spawn();
}

/// Shown until the daemon answers, and again if it dies.
fn status_page(status: &Status) -> String {
    let (title, detail, retry) = match status {
        Status::Starting(line) => ("Starting llmman\u{2026}", line.as_str(), false),
        Status::Failed(log) => ("llmman serve stopped", log.as_str(), true),
        Status::Ready => ("", "", false),
    };
    let button = if retry {
        r#"<button onclick="window.ipc.postMessage('retry')">Retry</button>"#
    } else {
        ""
    };
    format!(
        r#"<!doctype html><meta charset="utf-8"><style>
:root {{ color-scheme: light dark; font: 14px system-ui, sans-serif; }}
body {{ margin: 0; height: 100vh; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 1em; }}
pre {{ max-width: 90vw; max-height: 60vh; overflow: auto; white-space: pre-wrap;
  font-size: 12px; opacity: .8; margin: 0; }}
button {{ font: inherit; padding: .4em 1.2em; }}
</style><h2>{}</h2><pre>{}</pre>{button}"#,
        title,
        escape(detail)
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Without an Edit menu, WKWebView has no copy/paste shortcuts.
#[cfg(target_os = "macos")]
fn macos_menu() -> muda::Menu {
    use muda::{Menu, PredefinedMenuItem as P, Submenu};
    let menu = Menu::new();
    let app = Submenu::with_items(
        "llmman",
        true,
        &[
            &P::about(None, None),
            &P::separator(),
            &P::hide(None),
            &P::hide_others(None),
            &P::show_all(None),
            &P::separator(),
            &P::quit(None),
        ],
    );
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[
            &P::undo(None),
            &P::redo(None),
            &P::separator(),
            &P::cut(None),
            &P::copy(None),
            &P::paste(None),
            &P::select_all(None),
        ],
    );
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &P::minimize(None),
            &P::fullscreen(None),
            &P::close_window(None),
        ],
    );
    menu.append_items(&[&app.unwrap(), &edit.unwrap(), &window.unwrap()])
        .expect("menu");
    menu.init_for_nsapp();
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_log_text() {
        let page = status_page(&Status::Failed("<script>&".into()));
        assert!(page.contains("&lt;script&gt;&amp;"));
        assert!(page.contains("Retry"));
    }

    #[test]
    fn only_web_urls_leave() {
        assert!(is_web("https://example.com"));
        assert!(!is_web("file:///etc/passwd"));
        assert!(!is_web("blob:http://127.0.0.1:17434/x"));
    }
}
