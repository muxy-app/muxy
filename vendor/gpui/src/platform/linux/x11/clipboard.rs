/*
 * Copyright 2022 - 2025 Zed Industries, Inc.
 * License: Apache-2.0
 * See LICENSE-APACHE for complete license terms
 *
 * Adapted from the x11 submodule of the arboard project https://github.com/1Password/arboard
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 *
 * Copyright 2022 The Arboard contributors
 *
 * The project to which this file belongs is licensed under either of
 * the Apache 2.0 or the MIT license at the licensee's choice. The terms
 * and conditions of the chosen license apply to this file.
*/


use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    thread_local,
    time::{Duration, Instant},
};

use parking_lot::{Condvar, Mutex, MutexGuard, RwLock};
use x11rb::{
    COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, NONE,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property,
            PropertyNotifyEvent, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent,
            SelectionRequestEvent, Time, WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::{
    ClipboardEntry, ClipboardItem, Image, ImageFormat, hash,
    platform::linux::platform::paths_from_uri_list,
};

type Result<T, E = Error> = std::result::Result<T, E>;

static CLIPBOARD: Mutex<Option<GlobalClipboard>> = parking_lot::const_mutex(None);

x11rb::atom_manager! {
    pub Atoms: AtomCookies {
        CLIPBOARD,
        PRIMARY,
        SECONDARY,

        CLIPBOARD_MANAGER,
        SAVE_TARGETS,
        TARGETS,
        ATOM,
        INCR,

        UTF8_STRING,
        UTF8_MIME_0: b"text/plain;charset=utf-8",
        UTF8_MIME_1: b"text/plain;charset=UTF-8",
        STRING,
        TEXT,
        TEXT_MIME_UNKNOWN: b"text/plain",

        URI_LIST: b"text/uri-list",

        PNG__MIME: ImageFormat::mime_type(ImageFormat::Png ).as_bytes(),
        JPEG_MIME: ImageFormat::mime_type(ImageFormat::Jpeg).as_bytes(),
        WEBP_MIME: ImageFormat::mime_type(ImageFormat::Webp).as_bytes(),
        GIF__MIME: ImageFormat::mime_type(ImageFormat::Gif ).as_bytes(),
        SVG__MIME: ImageFormat::mime_type(ImageFormat::Svg ).as_bytes(),
        BMP__MIME: ImageFormat::mime_type(ImageFormat::Bmp ).as_bytes(),
        TIFF_MIME: ImageFormat::mime_type(ImageFormat::Tiff).as_bytes(),

        ARBOARD_CLIPBOARD,
    }
}

thread_local! {
    static ATOM_NAME_CACHE: RefCell<HashMap<Atom, &'static str>> = Default::default();
}

const LONG_TIMEOUT_DUR: Duration = Duration::from_millis(4000);
const SHORT_TIMEOUT_DUR: Duration = Duration::from_millis(10);

#[derive(Debug, PartialEq, Eq)]
enum ManagerHandoverState {
    Idle,
    InProgress,
    Finished,
}

struct GlobalClipboard {
    inner: Arc<Inner>,

    server_handle: JoinHandle<()>,
}

struct XContext {
    conn: RustConnection,
    win_id: u32,
}

struct Inner {
    server: XContext,
    atoms: Atoms,

    clipboard: Selection,
    primary: Selection,
    secondary: Selection,

    handover_state: Mutex<ManagerHandoverState>,
    handover_cv: Condvar,

    serve_stopped: AtomicBool,
}

impl XContext {
    fn new() -> Result<Self> {
        let (conn, screen_num): (RustConnection, _) =
            RustConnection::connect(None).map_err(|_| {
                Error::unknown("X11 server connection timed out because it was unreachable")
            })?;
        let screen = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or(Error::unknown("no screen found"))?;
        let win_id = conn.generate_id().map_err(into_unknown)?;

        let event_mask =
            EventMask::PROPERTY_CHANGE |
            EventMask::STRUCTURE_NOTIFY;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win_id,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::COPY_FROM_PARENT,
            COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(event_mask),
        )
        .map_err(into_unknown)?;
        conn.flush().map_err(into_unknown)?;

        Ok(Self { conn, win_id })
    }
}

#[derive(Default)]
struct Selection {
    data: RwLock<Option<Vec<ClipboardData>>>,
    mutex: Mutex<()>,
    data_changed: Condvar,
}

#[derive(Debug, Clone)]
struct ClipboardData {
    bytes: Vec<u8>,

    format: Atom,
}

enum ReadSelNotifyResult {
    GotData(ClipboardData),
    IncrStarted,
    EventNotRecognized,
}

impl Inner {
    fn new() -> Result<Self> {
        let server = XContext::new()?;
        let atoms = Atoms::new(&server.conn)
            .map_err(into_unknown)?
            .reply()
            .map_err(into_unknown)?;

        Ok(Self {
            server,
            atoms,
            clipboard: Selection::default(),
            primary: Selection::default(),
            secondary: Selection::default(),
            handover_state: Mutex::new(ManagerHandoverState::Idle),
            handover_cv: Condvar::new(),
            serve_stopped: AtomicBool::new(false),
        })
    }

    fn write(
        &self,
        data: Vec<ClipboardData>,
        selection: ClipboardKind,
        wait: WaitConfig,
    ) -> Result<()> {
        if self.serve_stopped.load(Ordering::Relaxed) {
            return Err(Error::unknown(
                "The clipboard handler thread seems to have stopped. Logging messages may reveal the cause. (See the `log` crate.)",
            ));
        }

        let server_win = self.server.win_id;

        self.server
            .conn
            .set_selection_owner(server_win, self.atom_of(selection), Time::CURRENT_TIME)
            .map_err(|_| Error::ClipboardOccupied)?;

        self.server.conn.flush().map_err(into_unknown)?;

        let selection = self.selection_of(selection);
        let mut data_guard = selection.data.write();
        *data_guard = Some(data);

        let mut guard = selection.mutex.lock();

        selection.data_changed.notify_all();

        match wait {
            WaitConfig::None => {}
            WaitConfig::Forever => {
                drop(data_guard);
                selection.data_changed.wait(&mut guard);
            }
            WaitConfig::Until(deadline) => {
                drop(data_guard);
                selection.data_changed.wait_until(&mut guard, deadline);
            }
        }

        Ok(())
    }

    fn read(&self, formats: &[Atom], selection: ClipboardKind) -> Result<ClipboardData> {
        if self.is_owner(selection)? {
            let data = self.selection_of(selection).data.read();
            if let Some(data_list) = &*data {
                for data in data_list {
                    for format in formats {
                        if *format == data.format {
                            return Ok(data.clone());
                        }
                    }
                }
            }
            return Err(Error::ContentNotAvailable);
        }
        let reader = XContext::new()?;

        let highest_precedence_format =
            match self.read_single(&reader, selection, self.atoms.TARGETS) {
                Err(err) => {
                    log::trace!("Clipboard TARGETS query failed with {err:?}");
                    None
                }
                Ok(ClipboardData { bytes, format }) => {
                    if format == self.atoms.ATOM {
                        let available_formats = Self::parse_formats(&bytes);
                        formats
                            .iter()
                            .find(|format| available_formats.contains(format))
                    } else {
                        log::trace!(
                            "Unexpected clipboard TARGETS format {}",
                            self.atom_name(format)
                        );
                        None
                    }
                }
            };

        if let Some(&format) = highest_precedence_format {
            let data = self.read_single(&reader, selection, format)?;
            if !formats.contains(&data.format) {
                log::trace!(
                    "Conversion to {} responded with {} which is not supported",
                    self.atom_name(format),
                    self.atom_name(data.format),
                );
                return Err(Error::ConversionFailure);
            }
            return Ok(data);
        }

        log::trace!("Falling back on attempting to convert clipboard to each format.");
        for format in formats {
            match self.read_single(&reader, selection, *format) {
                Ok(data) => {
                    if formats.contains(&data.format) {
                        return Ok(data);
                    } else {
                        log::trace!(
                            "Conversion to {} responded with {} which is not supported",
                            self.atom_name(*format),
                            self.atom_name(data.format),
                        );
                        continue;
                    }
                }
                Err(Error::ContentNotAvailable) => {
                    continue;
                }
                Err(e) => {
                    log::trace!("Conversion to {} failed: {}", self.atom_name(*format), e);
                    return Err(e);
                }
            }
        }
        log::trace!("All conversions to supported formats failed.");
        Err(Error::ContentNotAvailable)
    }

    fn parse_formats(bytes: &[u8]) -> Vec<Atom> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    fn read_single(
        &self,
        reader: &XContext,
        selection: ClipboardKind,
        target_format: Atom,
    ) -> Result<ClipboardData> {
        reader
            .conn
            .delete_property(reader.win_id, self.atoms.ARBOARD_CLIPBOARD)
            .map_err(into_unknown)?;

        reader
            .conn
            .convert_selection(
                reader.win_id,
                self.atom_of(selection),
                target_format,
                self.atoms.ARBOARD_CLIPBOARD,
                Time::CURRENT_TIME,
            )
            .map_err(into_unknown)?;
        reader.conn.sync().map_err(into_unknown)?;

        log::trace!("Finished `convert_selection`");

        let mut incr_data: Vec<u8> = Vec::new();
        let mut using_incr = false;

        let mut timeout_end = Instant::now() + LONG_TIMEOUT_DUR;

        while Instant::now() < timeout_end {
            let event = reader.conn.poll_for_event().map_err(into_unknown)?;
            let event = match event {
                Some(e) => e,
                None => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
            };
            match event {
                Event::SelectionNotify(event) => {
                    log::trace!("Read SelectionNotify");
                    let result = self.handle_read_selection_notify(
                        reader,
                        target_format,
                        &mut using_incr,
                        &mut incr_data,
                        event,
                    )?;
                    match result {
                        ReadSelNotifyResult::GotData(data) => return Ok(data),
                        ReadSelNotifyResult::IncrStarted => {
                            timeout_end += SHORT_TIMEOUT_DUR;
                        }
                        ReadSelNotifyResult::EventNotRecognized => (),
                    }
                }
                Event::PropertyNotify(event) => {
                    let result = self.handle_read_property_notify(
                        reader,
                        target_format,
                        using_incr,
                        &mut incr_data,
                        &mut timeout_end,
                        event,
                    )?;
                    if result {
                        return Ok(ClipboardData {
                            bytes: incr_data,
                            format: target_format,
                        });
                    }
                }
                _ => log::trace!(
                    "An unexpected event arrived while reading the clipboard: {:?}",
                    event
                ),
            }
        }
        log::info!("Time-out hit while reading the clipboard.");
        Err(Error::ContentNotAvailable)
    }

    fn atom_of(&self, selection: ClipboardKind) -> Atom {
        match selection {
            ClipboardKind::Clipboard => self.atoms.CLIPBOARD,
            ClipboardKind::Primary => self.atoms.PRIMARY,
            ClipboardKind::Secondary => self.atoms.SECONDARY,
        }
    }

    fn selection_of(&self, selection: ClipboardKind) -> &Selection {
        match selection {
            ClipboardKind::Clipboard => &self.clipboard,
            ClipboardKind::Primary => &self.primary,
            ClipboardKind::Secondary => &self.secondary,
        }
    }

    fn kind_of(&self, atom: Atom) -> Option<ClipboardKind> {
        match atom {
            a if a == self.atoms.CLIPBOARD => Some(ClipboardKind::Clipboard),
            a if a == self.atoms.PRIMARY => Some(ClipboardKind::Primary),
            a if a == self.atoms.SECONDARY => Some(ClipboardKind::Secondary),
            _ => None,
        }
    }

    fn is_owner(&self, selection: ClipboardKind) -> Result<bool> {
        let current = self
            .server
            .conn
            .get_selection_owner(self.atom_of(selection))
            .map_err(into_unknown)?
            .reply()
            .map_err(into_unknown)?
            .owner;

        Ok(current == self.server.win_id)
    }

    fn query_atom_name(&self, atom: x11rb::protocol::xproto::Atom) -> Result<String> {
        String::from_utf8(
            self.server
                .conn
                .get_atom_name(atom)
                .map_err(into_unknown)?
                .reply()
                .map_err(into_unknown)?
                .name,
        )
        .map_err(into_unknown)
    }

    fn atom_name(&self, atom: x11rb::protocol::xproto::Atom) -> &'static str {
        ATOM_NAME_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            match cache.entry(atom) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let s = self
                        .query_atom_name(atom)
                        .map(|s| Box::leak(s.into_boxed_str()) as &str)
                        .unwrap_or("FAILED-TO-GET-THE-ATOM-NAME");
                    entry.insert(s);
                    s
                }
            }
        })
    }

    fn handle_read_selection_notify(
        &self,
        reader: &XContext,
        target_format: u32,
        using_incr: &mut bool,
        incr_data: &mut Vec<u8>,
        event: SelectionNotifyEvent,
    ) -> Result<ReadSelNotifyResult> {

        if event.property == NONE || event.target != target_format {
            return Err(Error::ContentNotAvailable);
        }
        if self.kind_of(event.selection).is_none() {
            log::info!(
                "Received a SelectionNotify for a selection other than CLIPBOARD, PRIMARY or SECONDARY. This is unexpected."
            );
            return Ok(ReadSelNotifyResult::EventNotRecognized);
        }
        if *using_incr {
            log::warn!("Received a SelectionNotify while already expecting INCR segments.");
            return Ok(ReadSelNotifyResult::EventNotRecognized);
        }
        let property_type = AtomEnum::ANY;
        let mut reply = reader
            .conn
            .get_property(
                true,
                event.requestor,
                event.property,
                property_type,
                0,
                u32::MAX / 4,
            )
            .map_err(into_unknown)?
            .reply()
            .map_err(into_unknown)?;

        if reply.type_ == self.atoms.INCR {
            reply = reader
                .conn
                .get_property(
                    true,
                    event.requestor,
                    event.property,
                    self.atoms.INCR,
                    0,
                    u32::MAX / 4,
                )
                .map_err(into_unknown)?
                .reply()
                .map_err(into_unknown)?;
            log::trace!("Receiving INCR segments");
            *using_incr = true;
            if reply.value_len == 4 {
                let min_data_len = reply
                    .value32()
                    .and_then(|mut vals| vals.next())
                    .unwrap_or(0);
                incr_data.reserve(min_data_len as usize);
            }
            Ok(ReadSelNotifyResult::IncrStarted)
        } else {
            Ok(ReadSelNotifyResult::GotData(ClipboardData {
                bytes: reply.value,
                format: reply.type_,
            }))
        }
    }

    fn handle_read_property_notify(
        &self,
        reader: &XContext,
        target_format: u32,
        using_incr: bool,
        incr_data: &mut Vec<u8>,
        timeout_end: &mut Instant,
        event: PropertyNotifyEvent,
    ) -> Result<bool> {
        if event.atom != self.atoms.ARBOARD_CLIPBOARD || event.state != Property::NEW_VALUE {
            return Ok(false);
        }
        if !using_incr {
            return Ok(false);
        }
        let reply = reader
            .conn
            .get_property(
                true,
                event.window,
                event.atom,
                if target_format == self.atoms.TARGETS {
                    self.atoms.ATOM
                } else {
                    target_format
                },
                0,
                u32::MAX / 4,
            )
            .map_err(into_unknown)?
            .reply()
            .map_err(into_unknown)?;

        if reply.value_len == 0 {
            return Ok(true);
        }
        incr_data.extend(reply.value);

        *timeout_end = Instant::now() + SHORT_TIMEOUT_DUR;

        Ok(false)
    }

    fn handle_selection_request(&self, event: SelectionRequestEvent) -> Result<()> {
        let selection = match self.kind_of(event.selection) {
            Some(kind) => kind,
            None => {
                log::warn!(
                    "Received a selection request to a selection other than the CLIPBOARD, PRIMARY or SECONDARY. This is unexpected."
                );
                return Ok(());
            }
        };

        let success;
        if event.target == self.atoms.TARGETS {
            log::trace!(
                "Handling TARGETS, dst property is {}",
                self.atom_name(event.property)
            );
            let mut targets = Vec::with_capacity(10);
            targets.push(self.atoms.TARGETS);
            targets.push(self.atoms.SAVE_TARGETS);
            let data = self.selection_of(selection).data.read();
            if let Some(data_list) = &*data {
                for data in data_list {
                    targets.push(data.format);
                    if data.format == self.atoms.UTF8_STRING {
                        targets.push(self.atoms.UTF8_MIME_0);
                        targets.push(self.atoms.UTF8_MIME_1);
                    }
                }
            }
            self.server
                .conn
                .change_property32(
                    PropMode::REPLACE,
                    event.requestor,
                    event.property,
                    self.atoms.ATOM,
                    &targets,
                )
                .map_err(into_unknown)?;
            self.server.conn.flush().map_err(into_unknown)?;
            success = true;
        } else {
            log::trace!("Handling request for (probably) the clipboard contents.");
            let data = self.selection_of(selection).data.read();
            if let Some(data_list) = &*data {
                success = match data_list.iter().find(|d| d.format == event.target) {
                    Some(data) => {
                        self.server
                            .conn
                            .change_property8(
                                PropMode::REPLACE,
                                event.requestor,
                                event.property,
                                event.target,
                                &data.bytes,
                            )
                            .map_err(into_unknown)?;
                        self.server.conn.flush().map_err(into_unknown)?;
                        true
                    }
                    None => false,
                };
            } else {
                success = false;
            }
        }
        let property = if success {
            event.property
        } else {
            AtomEnum::NONE.into()
        };
        self.server
            .conn
            .send_event(
                false,
                event.requestor,
                EventMask::NO_EVENT,
                SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: event.sequence,
                    time: event.time,
                    requestor: event.requestor,
                    selection: event.selection,
                    target: event.target,
                    property,
                },
            )
            .map_err(into_unknown)?;

        self.server.conn.flush().map_err(into_unknown)
    }

    fn ask_clipboard_manager_to_request_our_data(&self) -> Result<()> {
        if self.server.win_id == 0 {
            log::error!("The server's window id was 0. This is unexpected");
            return Ok(());
        }

        if !self.is_owner(ClipboardKind::Clipboard)? {
            return Ok(());
        }
        if self
            .selection_of(ClipboardKind::Clipboard)
            .data
            .read()
            .is_none()
        {
            return Ok(());
        }

        let mut handover_state = self.handover_state.lock();

        log::trace!("Sending the data to the clipboard manager");
        self.server
            .conn
            .convert_selection(
                self.server.win_id,
                self.atoms.CLIPBOARD_MANAGER,
                self.atoms.SAVE_TARGETS,
                self.atoms.ARBOARD_CLIPBOARD,
                Time::CURRENT_TIME,
            )
            .map_err(into_unknown)?;
        self.server.conn.flush().map_err(into_unknown)?;

        *handover_state = ManagerHandoverState::InProgress;
        let max_handover_duration = Duration::from_millis(100);

        let result = self
            .handover_cv
            .wait_for(&mut handover_state, max_handover_duration);

        if *handover_state == ManagerHandoverState::Finished {
            return Ok(());
        }
        if result.timed_out() {
            log::warn!(
                "Could not hand the clipboard contents over to the clipboard manager. The request timed out."
            );
            return Ok(());
        }

        Err(Error::unknown(
            "The handover was not finished and the condvar didn't time out, yet the condvar wait ended. This should be unreachable.",
        ))
    }
}

fn serve_requests(context: Arc<Inner>) -> Result<(), Box<dyn std::error::Error>> {
    fn handover_finished(clip: &Arc<Inner>, mut handover_state: MutexGuard<ManagerHandoverState>) {
        log::trace!("Finishing clipboard manager handover.");
        *handover_state = ManagerHandoverState::Finished;

        drop(handover_state);

        clip.handover_cv.notify_all();
    }

    log::trace!("Started serve requests thread.");

    let _guard = util::defer(|| {
        context.serve_stopped.store(true, Ordering::Relaxed);
    });

    let mut written = false;
    let mut notified = false;

    loop {
        match context.server.conn.wait_for_event().map_err(into_unknown)? {
            Event::DestroyNotify(_) => {
                log::trace!("Clipboard server window is being destroyed x_x");
                return Ok(());
            }
            Event::SelectionClear(event) => {
                log::trace!("Somebody else owns the clipboard now");

                if let Some(selection) = context.kind_of(event.selection) {
                    let selection = context.selection_of(selection);
                    let mut data_guard = selection.data.write();
                    *data_guard = None;

                    let _guard = selection.mutex.lock();
                    selection.data_changed.notify_all();
                }
            }
            Event::SelectionRequest(event) => {
                log::trace!(
                    "SelectionRequest - selection is: {}, target is {}",
                    context.atom_name(event.selection),
                    context.atom_name(event.target),
                );
                context
                    .handle_selection_request(event)
                    .map_err(into_unknown)?;

                let handover_state = context.handover_state.lock();
                if *handover_state == ManagerHandoverState::InProgress {
                    if event.target != context.atoms.TARGETS {
                        log::trace!("The contents were written to the clipboard manager.");
                        written = true;
                        if notified {
                            handover_finished(&context, handover_state);
                        }
                    }
                }
            }
            Event::SelectionNotify(event) => {
                if event.selection != context.atoms.CLIPBOARD_MANAGER {
                    log::error!(
                        "Received a `SelectionNotify` from a selection other than the CLIPBOARD_MANAGER. This is unexpected in this thread."
                    );
                    continue;
                }
                let handover_state = context.handover_state.lock();
                if *handover_state == ManagerHandoverState::InProgress {
                    log::trace!(
                        "The clipboard manager indicated that it's done requesting the contents from us."
                    );
                    notified = true;

                    if written {
                        handover_finished(&context, handover_state);
                    }
                }
            }
            _event => {
            }
        }
    }
}

pub(crate) struct Clipboard {
    inner: Arc<Inner>,
}

impl Clipboard {
    pub(crate) fn new() -> Result<Self> {
        let mut global_cb = CLIPBOARD.lock();
        if let Some(global_cb) = &*global_cb {
            return Ok(Self {
                inner: Arc::clone(&global_cb.inner),
            });
        }
        let ctx = Arc::new(Inner::new()?);
        let join_handle = std::thread::Builder::new()
            .name("Clipboard".to_owned())
            .spawn({
                let ctx = Arc::clone(&ctx);
                move || {
                    if let Err(error) = serve_requests(ctx) {
                        log::error!("Worker thread errored with: {}", error);
                    }
                }
            })
            .unwrap();
        *global_cb = Some(GlobalClipboard {
            inner: Arc::clone(&ctx),
            server_handle: join_handle,
        });
        Ok(Self { inner: ctx })
    }

    pub(crate) fn set_text(
        &self,
        message: Cow<'_, str>,
        selection: ClipboardKind,
        wait: WaitConfig,
    ) -> Result<()> {
        let data = vec![ClipboardData {
            bytes: message.into_owned().into_bytes(),
            format: self.inner.atoms.UTF8_STRING,
        }];
        self.inner.write(data, selection, wait)
    }

    #[allow(unused)]
    pub(crate) fn set_image(
        &self,
        image: Image,
        selection: ClipboardKind,
        wait: WaitConfig,
    ) -> Result<()> {
        let format = match image.format {
            ImageFormat::Png => self.inner.atoms.PNG__MIME,
            ImageFormat::Jpeg => self.inner.atoms.JPEG_MIME,
            ImageFormat::Webp => self.inner.atoms.WEBP_MIME,
            ImageFormat::Gif => self.inner.atoms.GIF__MIME,
            ImageFormat::Svg => self.inner.atoms.SVG__MIME,
            ImageFormat::Bmp => self.inner.atoms.BMP__MIME,
            ImageFormat::Tiff => self.inner.atoms.TIFF_MIME,
        };
        let data = vec![ClipboardData {
            bytes: image.bytes,
            format: self.inner.atoms.PNG__MIME,
        }];
        self.inner.write(data, selection, wait)
    }

    pub(crate) fn get_any(&self, selection: ClipboardKind) -> Result<ClipboardItem> {
        const IMAGE_FORMAT_COUNT: usize = 7;
        let image_format_atoms: [Atom; IMAGE_FORMAT_COUNT] = [
            self.inner.atoms.PNG__MIME,
            self.inner.atoms.JPEG_MIME,
            self.inner.atoms.WEBP_MIME,
            self.inner.atoms.GIF__MIME,
            self.inner.atoms.SVG__MIME,
            self.inner.atoms.BMP__MIME,
            self.inner.atoms.TIFF_MIME,
        ];
        let image_formats: [ImageFormat; IMAGE_FORMAT_COUNT] = [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Webp,
            ImageFormat::Gif,
            ImageFormat::Svg,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
        ];

        const TEXT_FORMAT_COUNT: usize = 6;
        let text_format_atoms: [Atom; TEXT_FORMAT_COUNT] = [
            self.inner.atoms.UTF8_STRING,
            self.inner.atoms.UTF8_MIME_0,
            self.inner.atoms.UTF8_MIME_1,
            self.inner.atoms.STRING,
            self.inner.atoms.TEXT,
            self.inner.atoms.TEXT_MIME_UNKNOWN,
        ];

        let atom_none: Atom = AtomEnum::NONE.into();

        const FORMAT_ATOM_COUNT: usize = 1 + TEXT_FORMAT_COUNT + IMAGE_FORMAT_COUNT;

        let mut format_atoms: [Atom; FORMAT_ATOM_COUNT] = [atom_none; FORMAT_ATOM_COUNT];

        format_atoms[0] = self.inner.atoms.URI_LIST;
        format_atoms[1..=IMAGE_FORMAT_COUNT].copy_from_slice(&image_format_atoms);
        format_atoms[IMAGE_FORMAT_COUNT + 1..].copy_from_slice(&text_format_atoms);
        debug_assert!(!format_atoms.contains(&atom_none));

        let content = |result: ClipboardData| -> Result<ClipboardItem> {
            for (format_atom, image_format) in image_format_atoms.into_iter().zip(image_formats) {
                if result.format == format_atom {
                    let bytes = result.bytes;
                    let id = hash(&bytes);
                    return Ok(ClipboardItem::new_image(&Image {
                        id,
                        format: image_format,
                        bytes,
                    }));
                }
            }

            let text = if result.format == self.inner.atoms.STRING {
                result.bytes.into_iter().map(|c| c as char).collect()
            } else {
                String::from_utf8(result.bytes).map_err(|_| Error::ConversionFailure)?
            };
            Ok(ClipboardItem::new_string(text))
        };

        let result = self.inner.read(&format_atoms, selection)?;

        log::trace!(
            "read clipboard as format {:?}",
            self.inner.atom_name(result.format)
        );

        if result.format != self.inner.atoms.URI_LIST {
            return content(result);
        }
        let beside = self
            .inner
            .read(&format_atoms[1..], selection)
            .and_then(content);
        let Some(paths) = paths_from_uri_list(&result.bytes) else {
            return beside;
        };
        let mut entries = vec![ClipboardEntry::ExternalPaths(paths)];
        entries.extend(beside.into_iter().flat_map(ClipboardItem::into_entries));
        Ok(ClipboardItem { entries })
    }

    pub fn is_owner(&self, selection: ClipboardKind) -> bool {
        self.inner.is_owner(selection).unwrap_or(false)
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        const MIN_OWNERS: usize = 3;

        let mut global_cb = CLIPBOARD.lock();
        if Arc::strong_count(&self.inner) == MIN_OWNERS {

            if let Err(e) = self.inner.ask_clipboard_manager_to_request_our_data() {
                log::error!(
                    "Could not hand the clipboard data over to the clipboard manager: {}",
                    e
                );
            }
            let global_cb = global_cb.take();
            if let Err(e) = self
                .inner
                .server
                .conn
                .destroy_window(self.inner.server.win_id)
            {
                log::error!("Failed to destroy the clipboard window. Error: {}", e);
                return;
            }
            if let Err(e) = self.inner.server.conn.flush() {
                log::error!("Failed to flush the clipboard window. Error: {}", e);
                return;
            }
            if let Some(global_cb) = global_cb
                && let Err(e) = global_cb.server_handle.join()
            {
                let message;
                if let Some(msg) = e.downcast_ref::<&'static str>() {
                    message = Some((*msg).to_string());
                } else if let Some(msg) = e.downcast_ref::<String>() {
                    message = Some(msg.clone());
                } else {
                    message = None;
                }
                if let Some(message) = message {
                    log::error!(
                        "The clipboard server thread panicked. Panic message: '{}'",
                        message,
                    );
                } else {
                    log::error!("The clipboard server thread panicked.");
                }
            }
        }
    }
}

fn into_unknown<E: std::fmt::Display>(error: E) -> Error {
    Error::Unknown {
        description: error.to_string(),
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ClipboardKind {
    Clipboard,

    Primary,

    Secondary,
}

#[derive(Default)]
pub(crate) enum WaitConfig {
    #[allow(
        unused,
        reason = "Right now we don't wait for clipboard contents to sync on app close, but we may in the future"
    )]
    Until(Instant),

    #[allow(unused)]
    #[allow(
        unused,
        reason = "Right now we don't wait for clipboard contents to sync on app close, but we may in the future"
    )]
    Forever,

    #[default]
    None,
}

#[non_exhaustive]
pub enum Error {
    ContentNotAvailable,

    ClipboardOccupied,

    ConversionFailure,

    Unknown { description: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Error::ContentNotAvailable => f.write_str("The clipboard contents were not available in the requested format or the clipboard is empty."),
			Error::ClipboardOccupied => f.write_str("The native clipboard is not accessible due to being held by an other party."),
			Error::ConversionFailure => f.write_str("The image or the text that was about the be transferred to/from the clipboard could not be converted to the appropriate format."),
			Error::Unknown { description } => f.write_fmt(format_args!("Unknown error while interacting with the clipboard: {description}")),
		}
    }
}

impl std::error::Error for Error {}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        macro_rules! kind_to_str {
			($( $e: pat ),*) => {
				match self {
					$(
						$e => stringify!($e),
					)*
				}
			}
		}
        let name = kind_to_str!(
            ContentNotAvailable,
            ClipboardOccupied,
            ConversionFailure,
            Unknown { .. }
        );
        f.write_fmt(format_args!("{name} - \"{self}\""))
    }
}

impl Error {
    pub(crate) fn unknown<M: Into<String>>(message: M) -> Self {
        Error::Unknown {
            description: message.into(),
        }
    }
}
