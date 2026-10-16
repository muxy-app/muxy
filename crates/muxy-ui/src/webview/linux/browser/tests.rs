#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use polybrowser::{BrowserEvent, ProfileManager};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{atomic::AtomicBool, mpsc};
use std::thread::JoinHandle;

const WAIT: Duration = Duration::from_secs(20);

struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let task = std::thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                let Ok((mut stream, _)) = server.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_read_timeout(Some(WAIT)).unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request);
                let body = "<!doctype html><title>Browser test</title><p>Needle text</p><input id='field'><select id='choice'><option>A</option><option>B</option></select><iframe srcdoc=\"<script>window.parent.postMessage('frame','*')</script>\"></iframe>";
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            }
        });
        Self {
            url,
            stop,
            task: Some(task),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.task.take().unwrap().join().unwrap();
    }
}

struct Page {
    native: NativeBrowser,
    events: Receiver<Event>,
}
impl Page {
    fn new(profile: &Profile, url: &str) -> Self {
        let (native, events) = NativeBrowser::open(&Options {
            profile,
            url: Some(url),
            allows: |url| url.starts_with("http://127.0.0.1:") || url == "about:blank",
        })
        .unwrap();
        let bounds = Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(500.0), gpui::px(400.0)),
        );
        native.sync(bounds, bounds, 0.0, 1.0);
        Self { native, events }
    }
    fn loaded(&self) {
        self.wait(|event| {
            matches!(
                event,
                Event::Page {
                    event: BrowserEvent::LoadFinished { .. },
                    ..
                }
            )
        });
    }
    fn wait(&self, found: impl Fn(&Event) -> bool) {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Ok(event) = self.events.try_recv() {
                if let Event::Page { navigation, .. } = &event
                    && !self.native.is_current_navigation(*navigation)
                {
                    continue;
                }
                if let Event::Surface(crate::webview::Event::Snapshot { id, generation, .. }) =
                    &event
                {
                    self.native.accept_frame(*id, *generation);
                }
                assert!(
                    !matches!(
                        event,
                        Event::Page {
                            event: BrowserEvent::LoadFailed { .. },
                            ..
                        }
                    ),
                    "{event:?}"
                );
                if found(&event) {
                    return;
                }
            } else {
                assert!(Instant::now() < deadline, "browser event timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    fn eval(&self, body: &str) -> Value {
        let (send, receive) = mpsc::channel();
        self.native
            .run_script(body, move |result| {
                let _ = send.send(result);
            })
            .unwrap();
        serde_json::from_str(&receive.recv_timeout(WAIT).unwrap().unwrap()).unwrap()
    }
    fn cookies(&self) -> Vec<Cookie> {
        let (send, receive) = mpsc::channel();
        self.native
            .cookies(None, move |result| {
                let _ = send.send(result);
            })
            .unwrap();
        receive.recv_timeout(WAIT).unwrap().unwrap()
    }
    fn set_cookie(&self, value: &str) {
        let (send, receive) = mpsc::channel();
        self.native
            .set_cookie(
                Cookie {
                    name: "login".into(),
                    value: value.into(),
                    domain: "127.0.0.1".into(),
                    path: "/".into(),
                    secure: false,
                    http_only: true,
                    expires: Some(
                        (std::time::SystemTime::now() + Duration::from_secs(3600)).into(),
                    ),
                },
                move |result| {
                    let _ = send.send(result);
                },
            )
            .unwrap();
        receive.recv_timeout(WAIT).unwrap().unwrap();
    }
}

fn drained() {
    let (send, receive) = mpsc::channel();
    post(move |runtime| {
        send.send(runtime.unwrap().live()).unwrap();
    });
    assert_eq!(receive.recv_timeout(WAIT).unwrap(), 0);
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn navigating_while_the_initial_blank_page_loads_keeps_the_requested_address() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    for delay in [0, 1, 5, 10, 0, 1, 5, 10] {
        let page = Page::new(&Profile::ephemeral(), "about:blank");
        std::thread::sleep(Duration::from_millis(delay));
        page.native.load_url(&server.url).unwrap();
        let expected = format!("{}/", server.url);
        page.wait(|event| {
            if let Event::Page { event: BrowserEvent::UrlChanged { url }, .. } = event {
                assert_eq!(url, &expected);
            }
            matches!(event, Event::Page { event: BrowserEvent::LoadFinished { url }, .. } if url == &expected)
        });
        drop(page);
        drained();
    }
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn a_fragment_navigation_does_not_block_later_page_navigation() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    let page = Page::new(&Profile::ephemeral(), &server.url);
    page.loaded();
    let fragment = format!("{}/#section", server.url);
    page.native.load_url(&fragment).unwrap();
    page.wait(|event| matches!(event, Event::Page { event: BrowserEvent::UrlChanged { url }, .. } if url == &fragment));
    assert_eq!(page.eval("return location.href"), json!(fragment));
    page.eval("location.hash = 'two'; return true;");
    let hash = format!("{}/#two", server.url);
    page.wait(|event| matches!(event, Event::Page { event: BrowserEvent::UrlChanged { url }, .. } if url == &hash));
    page.eval("history.pushState({}, '', '/client-route'); return true;");
    let route = format!("{}/client-route", server.url);
    page.wait(|event| matches!(event, Event::Page { event: BrowserEvent::UrlChanged { url }, .. } if url == &route));
    page.eval("setTimeout(() => location.href = '/next', 10); return true;");
    let next = format!("{}/next", server.url);
    page.wait(|event| matches!(event, Event::Page { event: BrowserEvent::UrlChanged { url }, .. } if url == &next));
    page.loaded();
    drop(page);
    drained();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn right_click_requests_the_browser_menu_unless_the_page_handles_it() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    let page = Page::new(&Profile::ephemeral(), &server.url);
    page.loaded();
    page.eval("document.body.replaceChildren(); return true;");
    page.native.set_visible(true);
    let right_click = || {
        let position = gpui::point(gpui::px(200.0), gpui::px(150.0));
        page.native.pointer(Pointer::Pressed {
            position,
            button: gpui::MouseButton::Right,
            modifiers: gpui::Modifiers::default(),
        });
        page.native.pointer(Pointer::Released {
            position,
            button: gpui::MouseButton::Right,
            modifiers: gpui::Modifiers::default(),
        });
    };
    right_click();
    page.wait(|event| matches!(event, Event::ContextMenu(_)));

    page.eval("window.handled = false; window.addEventListener('contextmenu', event => { event.preventDefault(); window.handled = true; }); return true;");
    right_click();
    let deadline = Instant::now() + WAIT;
    while page.eval("return window.handled") != json!(true) {
        assert!(
            Instant::now() < deadline,
            "page did not receive right-click"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    while let Ok(event) = page.events.try_recv() {
        assert!(!matches!(event, Event::ContextMenu(_)), "{event:?}");
    }
    drop(page);
    drained();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn replacing_a_pending_load_does_not_erase_the_address() {
    let _serial = super::super::tests::alone();
    let pending = TcpListener::bind("127.0.0.1:0").unwrap();
    let page = Page::new(
        &Profile::ephemeral(),
        &format!("http://{}/pending", pending.local_addr().unwrap()),
    );
    let started = Cell::new(None);
    page.wait(|event| {
        if let Event::Page {
            navigation,
            event: BrowserEvent::LoadStarted { .. },
        } = event
        {
            started.set(Some(*navigation));
            return true;
        }
        false
    });
    let server = Server::new();
    let url = format!("{}/replacement", server.url);
    page.native.load_url(&url).unwrap();
    assert!(!page.native.is_current_navigation(started.get().unwrap()));
    page.wait(|event| {
        if let Event::Page { event: BrowserEvent::UrlChanged { url }, .. } = event {
            assert!(!url.is_empty(), "a cancelled load erased the tab's address");
        }
        matches!(event, Event::Page { event: BrowserEvent::LoadFinished { url: loaded }, .. } if loaded == &url)
    });
    assert_eq!(page.eval("return location.href"), json!(url));
    drop(page);
    drained();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn navigation_scripts_hidden_screenshots_and_extension_isolation() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    let page = Page::new(&Profile::ephemeral(), &server.url);
    page.loaded();
    assert_eq!(page.eval("return await Promise.resolve(42)"), json!(42));
    assert_eq!(
        page.eval("return typeof window.webkit?.messageHandlers?.muxy"),
        json!("undefined")
    );
    assert_eq!(
        page.eval("return typeof window.__muxyPaste"),
        json!("undefined")
    );
    page.eval("history.pushState({}, '', '/next'); document.title = 'Changed'; return true;");
    page.wait(|event| matches!(event, Event::Page { event: BrowserEvent::UrlChanged { url }, .. } if url.ends_with("/next")));
    let (send, receive) = mpsc::channel();
    page.native
        .capture(move |result| {
            let _ = send.send(result);
        })
        .unwrap();
    let image = receive.recv_timeout(WAIT).unwrap().unwrap();
    assert!(!image.rgba.is_empty());
    page.native
        .load_url(&format!("{}/second", server.url))
        .unwrap();
    let (loaded, can_go_back) = (Cell::new(false), Cell::new(false));
    page.wait(|event| {
        match event {
            Event::Page {
                event: BrowserEvent::LoadFinished { .. },
                ..
            } => loaded.set(true),
            Event::Page {
                event:
                    BrowserEvent::NavigationStateChanged {
                        can_go_back: back, ..
                    },
                ..
            } => can_go_back.set(*back),
            _ => (),
        }
        loaded.get() && can_go_back.get()
    });
    page.native.go_back().unwrap();
    page.loaded();
    assert!(
        page.eval("return location.href")
            .as_str()
            .unwrap()
            .ends_with("/next")
    );
    let (send, receive) = mpsc::channel();
    page.native
        .find("Needle", FindOptions::default(), move |result| {
            let _ = send.send(result);
        })
        .unwrap();
    assert!(receive.recv_timeout(WAIT).unwrap().unwrap());
    drop(page);
    drained();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn profiles_share_their_own_persistent_data_and_clear_only_the_selected_profile() {
    let _serial = super::super::tests::alone();
    let root = std::env::temp_dir().join(format!("muxy-browser-profile-{}", std::process::id()));
    let manager = ProfileManager::open_at(&root).unwrap();
    let first = manager.get_or_create("First").unwrap();
    let second = manager.get_or_create("Second").unwrap();
    let server = Server::new();
    let a = Page::new(&first, &server.url);
    a.loaded();
    let b = Page::new(&first, &server.url);
    b.loaded();
    let c = Page::new(&second, &server.url);
    c.loaded();
    a.set_cookie("one");
    assert_eq!(b.cookies()[0].value, "one");
    assert!(c.cookies().is_empty());
    a.eval("localStorage.setItem('value','one'); return true");
    let deadline = Instant::now() + WAIT;
    while b.eval("return localStorage.getItem('value')") != json!("one") {
        assert!(
            Instant::now() < deadline,
            "profile storage did not propagate"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(c.eval("return localStorage.getItem('value')"), Value::Null);
    c.set_cookie("two");
    drop(a);
    drop(b);
    let a = Page::new(&first, &server.url);
    a.loaded();
    assert_eq!(a.cookies()[0].value, "one");
    let (send, receive) = mpsc::channel();
    clear_site_data(Engine::WebKit, &first, move |result| {
        let _ = send.send(result);
    })
    .unwrap();
    receive.recv_timeout(WAIT).unwrap().unwrap();
    assert!(a.cookies().is_empty());
    assert_eq!(c.cookies()[0].value, "two");
    drop(a);
    let (send, receive) = mpsc::channel();
    close_profile(&second, move |result| {
        let _ = send.send(result);
    });
    receive.recv_timeout(WAIT).unwrap().unwrap();
    manager.delete(second.id()).unwrap();
    let (send, receive) = mpsc::channel();
    c.native
        .cookies(None, move |result| {
            let _ = send.send(result);
        })
        .unwrap();
    assert!(receive.recv_timeout(WAIT).unwrap().is_err());
    drop(c);
    drained();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn closing_or_navigating_a_page_cancels_pending_automation() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    for navigate in [true, false] {
        let page = Page::new(&Profile::ephemeral(), &server.url);
        page.loaded();
        let (send, receive) = mpsc::channel();
        page.native
            .run_script("await new Promise(() => {});", move |result| {
                let _ = send.send(result);
            })
            .unwrap();
        if navigate {
            page.native
                .load_url(&format!("{}/next", server.url))
                .unwrap();
            page.loaded();
        }
        drop(page);
        assert!(receive.recv_timeout(WAIT).unwrap().is_err());
        drained();
    }
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn retirement_drains_an_inflight_cookie_import_before_deleting_data() {
    let _serial = super::super::tests::alone();
    let root = std::env::temp_dir().join(format!("muxy-browser-retirement-{}", std::process::id()));
    let manager = ProfileManager::open_at(&root).unwrap();
    let profile = manager.get_or_create("Importing").unwrap();
    let server = Server::new();
    let page = Page::new(&profile, &server.url);
    page.loaded();
    let cookies = (0..64)
        .map(|index| Cookie {
            name: format!("import-{index}"),
            value: "value".into(),
            domain: "127.0.0.1".into(),
            path: "/".into(),
            secure: false,
            http_only: true,
            expires: None,
        })
        .collect();
    let (import_send, imported) = mpsc::channel();
    import_cookies(Engine::WebKit, &profile, cookies, move |result| {
        let _ = import_send.send(result);
    })
    .unwrap();
    let (close_send, closed) = mpsc::channel();
    close_profile(&profile, move |result| {
        let _ = close_send.send(result);
    });
    closed.recv_timeout(WAIT).unwrap().unwrap();
    assert_eq!(imported.try_recv().unwrap().unwrap(), 64);
    manager.delete(profile.id()).unwrap();
    drop(page);
    drained();
    assert!(!profile.engine_data_dir(Engine::WebKit).unwrap().exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn automation_rejects_a_page_that_changes_the_json_result_type() {
    let _serial = super::super::tests::alone();
    let server = Server::new();
    let page = Page::new(&Profile::ephemeral(), &server.url);
    page.loaded();
    let (send, receive) = mpsc::channel();
    page.native.run_script("const original = JSON.stringify; JSON.stringify = value => { if (value !== 'not json') return original(value); JSON.stringify = original; return 5; }; return 'not json';", move |result| { let _ = send.send(result); }).unwrap();
    assert!(receive.recv_timeout(WAIT).unwrap().is_err());
    assert_eq!(page.eval("return 42"), json!(42));
    drop(page);
    drained();
}
