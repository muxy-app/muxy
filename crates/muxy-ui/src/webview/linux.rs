//! Pages in `WebKitGTK`. A GPUI window is not a GTK window, so a page cannot
//! be placed inside one. Each page renders into an offscreen GTK window on a
//! thread that runs GTK; the app draws its frames and passes it input.

pub(crate) mod browser;
mod ffi;
mod page;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};

use async_channel::{Receiver, Sender};
use gpui::{Bounds, CursorStyle, Keystroke, Pixels, Rgba, Window};
use muxy_core::capability::Capability;
use raw_window_handle::HasWindowHandle;

use self::page::{Key, Runtime, Setup};
use super::{AssetResult, Event, Pointer, assets::Source};

pub const CAPABILITY: Capability = Capability::Supported;
/// The page renders offscreen, so the app draws it and passes it input.
pub(super) const DRAWN_BY_APP: bool = true;

pub(super) fn probe() -> Capability {
    match ffi::load() {
        Ok(_) => Capability::Supported,
        Err(reason) => Capability::Unavailable(reason),
    }
}

static DATA_DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(super) fn set_data_directory(directory: &Path) {
    *DATA_DIRECTORY
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(directory.to_owned());
}

fn data_directory() -> Option<PathBuf> {
    DATA_DIRECTORY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Work for the GTK thread, or the reason there is none to run it.
type Job = Box<dyn FnOnce(Result<&Rc<Runtime>, &str>) + Send>;

enum Thread {
    Idle,
    Running(mpsc::Sender<Job>),
    Failed(String),
}

static THREAD: Mutex<Thread> = Mutex::new(Thread::Idle);

/// Runs `job` on the GTK thread, starting it for the first one. The caller
/// never waits: a thread that could not start hands the job its reason.
fn post(job: impl FnOnce(Result<&Rc<Runtime>, &str>) + Send + 'static) {
    let job: Job = Box::new(job);
    let mut thread = THREAD.lock().unwrap_or_else(PoisonError::into_inner);
    let job = match &*thread {
        Thread::Running(jobs) => match jobs.send(job) {
            Ok(()) => {
                drop(thread);
                page::wake();
                return;
            }
            Err(mpsc::SendError(job)) => job,
        },
        Thread::Failed(reason) => {
            let reason = reason.clone();
            drop(thread);
            job(Err(&reason));
            return;
        }
        Thread::Idle => job,
    };
    let (jobs, queue) = mpsc::channel();
    match std::thread::Builder::new()
        .name("webview".into())
        .spawn(move || run(queue))
    {
        Ok(_) => {
            let _ = jobs.send(job);
            *thread = Thread::Running(jobs);
        }
        Err(error) => {
            let reason = error.to_string();
            *thread = Thread::Failed(reason.clone());
            drop(thread);
            job(Err(&reason));
        }
    }
}

fn run(queue: mpsc::Receiver<Job>) {
    let reason = match Runtime::start() {
        Ok(runtime) => return runtime.serve(queue),
        Err(reason) => reason,
    };
    *THREAD.lock().unwrap_or_else(PoisonError::into_inner) = Thread::Failed(reason.clone());
    while let Ok(job) = queue.try_recv() {
        job(Err(&reason));
    }
}

/// What the GTK thread publishes about a page for the app to read.
#[derive(Debug, Default)]
pub(super) struct Shared {
    generation: AtomicU64,
    snapshot: AtomicU64,
}

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(super) struct NativeWebview {
    id: u64,
    events: Events,
    shared: Arc<Shared>,
    modal: Cell<bool>,
    passthrough: Cell<bool>,
    hit_regions: RefCell<Option<Vec<Bounds<Pixels>>>>,
    pressed_inside: Cell<bool>,
    placed: Cell<Option<(u32, u32, f32)>>,
    shown: Cell<Option<bool>>,
}

impl NativeWebview {
    pub(super) fn new(
        window: &Window,
        source: Source,
        script: &str,
        _background: Rgba,
        transparent_background: bool,
    ) -> Result<(Self, Receiver<Event>), String> {
        // A window the desktop does not show, as in a test, has nowhere to
        // draw a page, so no engine is started for it.
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
        Self::open(source, script, transparent_background)
    }

    /// Opens a page that renders off screen, whoever draws it.
    fn open(
        source: Source,
        script: &str,
        transparent: bool,
    ) -> Result<(Self, Receiver<Event>), String> {
        let url = source.entry_url().map_err(|error| error.to_string())?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (events, receiver) = async_channel::bounded(256);
        let shared = Arc::new(Shared::default());
        let setup = Setup {
            id,
            url,
            source: Some(source),
            browser: None,
            script: script.to_owned(),
            transparent,
            events: Events::Extension(events.clone()),
            shared: shared.clone(),
        };
        post(move |runtime| match runtime {
            Ok(runtime) => runtime.open(setup),
            Err(reason) => {
                let _ = setup.events.try_send(Event::Failed(reason.to_owned()));
            }
        });
        Ok((
            Self {
                id,
                events: Events::Extension(events),
                shared,
                modal: Cell::new(false),
                passthrough: Cell::new(false),
                hit_regions: RefCell::new(None),
                pressed_inside: Cell::new(false),
                placed: Cell::new(None),
                shown: Cell::new(None),
            },
            receiver,
        ))
    }

    /// Runs `job` for this page on the GTK thread, if the page is still open.
    fn with(&self, job: impl FnOnce(&Rc<Runtime>, &page::Page) + Send + 'static) {
        let id = self.id;
        post(move |runtime| {
            if let Ok(runtime) = runtime
                && let Some(page) = runtime.page(id)
            {
                job(runtime, &page);
            }
        });
    }

    /// GPUI offers a key to the app's shortcuts before the page, so only
    /// Escape is taken back here.
    pub(super) fn set_shortcuts(&self, modal: bool, _shortcuts: Rc<Vec<Keystroke>>) {
        self.modal.set(modal);
    }

    // GPUI sends keys to the page only while it is focused, and draws and
    // hit-tests what is above the page itself.
    #[allow(clippy::unused_self)]
    pub(super) fn set_focused(&self, _focused: bool) {}

    #[allow(clippy::unused_self)]
    pub(super) fn set_grips(&self, _grips: &[(Bounds<Pixels>, CursorStyle)]) {}

    #[allow(clippy::unused_self)]
    pub(super) fn occlude(&self, _occlusions: &[Bounds<Pixels>]) {}

    #[allow(clippy::unused_self)]
    pub(super) fn occlude_also(&self, _bounds: Bounds<Pixels>) {}

    pub(super) fn set_mouse_passthrough(&self, enabled: bool) {
        self.passthrough.set(enabled);
    }

    pub(super) fn set_hit_regions(&self, regions: Option<&[Bounds<Pixels>]>) {
        let regions = regions.map(<[_]>::to_vec);
        if *self.hit_regions.borrow() != regions {
            *self.hit_regions.borrow_mut() = regions;
        }
    }

    /// GPUI draws these pages in its own order, so there is nothing to raise.
    pub(super) fn order_front(_views: &[&Self]) {}

    /// The pointer event the page gets, or `None` where it lets the pointer
    /// through to what is under it.
    fn admit(&self, event: Pointer) -> Option<Pointer> {
        let regions = self.hit_regions.borrow();
        let Some(regions) = regions.as_ref() else {
            return Some(event);
        };
        let inside =
            |position: gpui::Point<Pixels>| regions.iter().any(|region| region.contains(&position));
        match event {
            Pointer::Pressed { position, .. } => {
                let inside = inside(position);
                self.pressed_inside.set(inside);
                inside.then_some(event)
            }
            Pointer::Released { .. } => self.pressed_inside.replace(false).then_some(event),
            Pointer::Moved { position, .. } => {
                if self.pressed_inside.get() || inside(position) {
                    Some(event)
                } else {
                    Some(Pointer::Left)
                }
            }
            Pointer::Scrolled { position, .. } => inside(position).then_some(event),
            Pointer::Left => Some(event),
        }
    }

    pub(super) fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    pub(super) fn install_script(&self, script: &str) {
        let script = script.to_owned();
        self.with(move |runtime, page| page.install(runtime, &script));
    }

    pub(super) fn evaluate(&self, script: &str) {
        let script = script.to_owned();
        self.with(move |runtime, page| page.evaluate(runtime, &script));
    }

    pub(super) fn reply(&self, id: u64, generation: u64, json: &str) {
        let json = json.to_owned();
        self.with(move |runtime, page| page.reply(runtime, id, generation, &json));
    }

    // Assets are read and answered on the GTK thread without passing here.
    #[allow(clippy::unused_self)]
    pub(super) fn complete_asset(&self, _id: u64, _result: AssetResult) {}

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn sync(
        &self,
        bounds: Bounds<Pixels>,
        _clip: Bounds<Pixels>,
        _background: Rgba,
        _radius: f64,
        scale: f32,
    ) {
        let pixels =
            |points: Pixels| (f32::from(points) * scale).round().clamp(0.0, 16384.0) as u32;
        let placed = (pixels(bounds.size.width), pixels(bounds.size.height), scale);
        if placed.0 == 0 || placed.1 == 0 || self.placed.replace(Some(placed)) == Some(placed) {
            return;
        }
        self.with(move |runtime, page| page.place(runtime, placed.0, placed.1, placed.2));
    }

    /// Called every frame, so only a change is passed on.
    pub(super) fn set_visible(&self, visible: bool) {
        if self.shown.replace(Some(visible)) != Some(visible) {
            self.with(move |runtime, page| page.set_visible(runtime, visible));
        }
    }

    pub(super) fn focus(&self) {
        self.with(|runtime, page| page.focus(runtime, true));
    }

    pub(super) fn blur(&self) {
        self.with(|runtime, page| page.focus(runtime, false));
    }

    pub(super) fn snapshot(&self) {
        self.with(|runtime, page| page.snapshot(runtime));
    }

    /// The page sends one frame at a time, so a slow window is never behind
    /// by more than one. Taking a frame lets the next one be sent.
    pub(super) fn accept_snapshot(&self, id: u64, generation: u64) -> bool {
        let accepted =
            self.shared.snapshot.load(Ordering::Acquire) == id && self.generation() == generation;
        self.with(|runtime, page| page.snapshot_taken(runtime));
        accepted
    }

    pub(super) fn pointer(&self, event: Pointer) {
        if self.passthrough.get() {
            return;
        }
        let Some(event) = self.admit(event) else {
            return;
        };
        self.with(move |runtime, page| page.pointer(runtime, event));
    }

    pub(super) fn key(
        &self,
        keystroke: &Keystroke,
        pressed: bool,
        clipboard: impl FnOnce() -> Option<String>,
    ) {
        let modifiers = keystroke.modifiers;
        if self.modal.get() && keystroke.key == "escape" && !modifiers.modified() {
            if pressed {
                let _ = self.events.try_send(Event::Escape);
            }
            return;
        }
        let offered = (pressed && pastes(keystroke)).then(clipboard).flatten();
        let key = Key {
            name: keystroke.key.clone(),
            text: keystroke.key_char.clone(),
            modifiers,
            pressed,
        };
        self.with(move |runtime, page| {
            if let Some(text) = &offered {
                page.offer_paste(runtime, text);
            }
            page.key(runtime, &key);
        });
    }

    pub(super) fn choose(&self, option: Option<usize>) {
        self.with(move |runtime, page| page.choose(runtime, option));
    }
}

impl Drop for NativeWebview {
    fn drop(&mut self) {
        let id = self.id;
        post(move |runtime| {
            if let Ok(runtime) = runtime {
                runtime.close(id);
            }
        });
    }
}

/// Whether the keystroke is one a desktop pastes with.
fn pastes(keystroke: &Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.alt || modifiers.platform || modifiers.function {
        return false;
    }
    (keystroke.key == "v" && modifiers.control && !modifiers.shift)
        || (keystroke.key == "insert" && modifiers.shift && !modifiers.control)
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
enum Events {
    Extension(Sender<Event>),
    Browser(Sender<crate::browser::Event>, Arc<AtomicU64>),
}
impl Events {
    fn try_send(&self, event: Event) -> Result<(), ()> {
        match self {
            Self::Extension(sender) => sender.try_send(event).map_err(|_| ()),
            Self::Browser(_, _) if matches!(event, Event::Failed(_)) => Ok(()),
            Self::Browser(sender, _) => sender
                .try_send(crate::browser::Event::Surface(event))
                .map_err(|_| ()),
        }
    }
    fn browser(&self, event: polybrowser::BrowserEvent) {
        if let Self::Browser(sender, navigation) = self {
            let _ = sender.try_send(crate::browser::Event::Page {
                navigation: navigation.load(Ordering::Acquire),
                event,
            });
        }
    }

    fn begin_navigation(&self, id: u64) {
        if let Self::Browser(_, navigation) = self {
            navigation.store(id, Ordering::Release);
        }
    }
}
