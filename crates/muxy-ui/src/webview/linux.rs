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

type Job = Box<dyn FnOnce(Result<&Rc<Runtime>, &str>) + Send>;

enum Thread {
    Idle,
    Running(mpsc::Sender<Job>),
    Failed(String),
}

static THREAD: Mutex<Thread> = Mutex::new(Thread::Idle);

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
    pressed_inside: Cell<u8>,
    last_click: Cell<Option<std::time::Instant>>,
    placed: Cell<Option<(u32, u32, f32)>>,
    shown: Cell<Option<bool>>,
    background: Cell<Option<Rgba>>,
}

impl NativeWebview {
    pub(super) fn new(
        window: &Window,
        source: Source,
        script: &str,
        background: Rgba,
    ) -> Result<(Self, Receiver<Event>), String> {
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
        Self::open(source, script, background)
    }

    fn open(
        source: Source,
        script: &str,
        background: Rgba,
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
            background: Some(background),
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
                pressed_inside: Cell::new(0),
                last_click: Cell::new(None),
                placed: Cell::new(None),
                shown: Cell::new(None),
                background: Cell::new(Some(background)),
            },
            receiver,
        ))
    }

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

    pub(super) fn set_shortcuts(&self, modal: bool, _shortcuts: Rc<Vec<Keystroke>>) {
        self.modal.set(modal);
    }

    #[allow(clippy::unused_self)]
    pub(super) fn set_focused(&self, _focused: bool) {}

    pub(super) fn take_click(&self) -> bool {
        super::rules::take_click(&self.last_click)
    }

    #[allow(clippy::unused_self)]
    pub(super) fn set_grips(&self, _grips: &[(Bounds<Pixels>, CursorStyle)]) {}

    #[allow(clippy::unused_self)]
    pub(super) fn occlude(&self, _occlusions: &[super::Cover], _window: &mut Window) {}

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

    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub(super) fn move_to_window(&self, _window: &Window) -> Result<(), String> {
        Ok(())
    }

    pub(super) fn order_front(_views: &[&Self]) {}

    fn admit(&self, event: Pointer) -> Option<Pointer> {
        let regions = self.hit_regions.borrow();
        let inside = |position: gpui::Point<Pixels>| {
            regions
                .as_ref()
                .is_none_or(|regions| regions.iter().any(|region| region.contains(&position)))
        };
        let held = self.pressed_inside.get();
        match event {
            Pointer::Pressed {
                position, button, ..
            } => {
                if !inside(position) {
                    return None;
                }
                self.pressed_inside.set(held | button_bit(button));
                self.last_click.set(Some(std::time::Instant::now()));
                Some(event)
            }
            Pointer::Released { button, .. } => {
                let bit = button_bit(button);
                self.pressed_inside.set(held & !bit);
                (held & bit != 0 || regions.is_none()).then_some(event)
            }
            Pointer::Moved { position, .. } => {
                if held != 0 || inside(position) {
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

    #[allow(clippy::unused_self)]
    pub(super) fn complete_asset(&self, _id: u64, _result: AssetResult) {}

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn sync(
        &self,
        bounds: Bounds<Pixels>,
        _clip: Bounds<Pixels>,
        background: Rgba,
        _radius: f64,
        scale: f32,
    ) {
        if let Some(current) = self.background.get()
            && current != background
        {
            self.background.set(Some(background));
            self.with(move |runtime, page| page.set_background(runtime, background));
        }
        let pixels =
            |points: Pixels| (f32::from(points) * scale).round().clamp(0.0, 16384.0) as u32;
        let placed = (pixels(bounds.size.width), pixels(bounds.size.height), scale);
        if placed.0 == 0 || placed.1 == 0 || self.placed.replace(Some(placed)) == Some(placed) {
            return;
        }
        self.with(move |runtime, page| page.place(runtime, placed.0, placed.1, placed.2));
    }

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

fn button_bit(button: gpui::MouseButton) -> u8 {
    match button {
        gpui::MouseButton::Left => 1,
        gpui::MouseButton::Right => 2,
        gpui::MouseButton::Middle => 4,
        gpui::MouseButton::Navigate(_) => 8,
    }
}
