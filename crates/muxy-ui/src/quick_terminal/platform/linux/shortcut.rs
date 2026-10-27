use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use gpui::{ForegroundExecutor, Task};
use muxy_core::quick_terminal::{
    QuickTerminalShortcut,
    keys::{CONTROL, OPTION, SHIFT},
};
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{self, ConnectionExt, EventMask, GrabMode, ModMask, WindowClass},
    },
    rust_connection::RustConnection,
};

use crate::quick_terminal::shortcut_service::{
    ShortcutBackend, ShortcutBackendFactory, ShortcutState,
};
use crate::tr;

pub(in crate::quick_terminal::platform) struct LinuxShortcutBackendFactory {
    executor: ForegroundExecutor,
}

impl LinuxShortcutBackendFactory {
    pub(in crate::quick_terminal::platform) fn new(executor: ForegroundExecutor) -> Self {
        Self { executor }
    }
}

impl ShortcutBackendFactory for LinuxShortcutBackendFactory {
    fn same_registration(
        &self,
        left: &QuickTerminalShortcut,
        right: &QuickTerminalShortcut,
    ) -> bool {
        left.key_combo()
            .and_then(muxy_core::quick_terminal::keys::KeyCombo::keystroke)
            == right
                .key_combo()
                .and_then(muxy_core::quick_terminal::keys::KeyCombo::keystroke)
    }

    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
        shortcut.key_combo().map(|combo| {
            Box::new(Backend {
                combo: combo.clone(),
                executor: self.executor.clone(),
                registration: None,
                listener: None,
            }) as Box<dyn ShortcutBackend>
        })
    }
}

struct Backend {
    combo: muxy_core::quick_terminal::keys::KeyCombo,
    executor: ForegroundExecutor,
    registration: Option<Registration>,
    listener: Option<Task<()>>,
}

impl ShortcutBackend for Backend {
    fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
        if self.registration.is_some() {
            return Ok(());
        }
        if super::wayland_session() {
            return Err(tr!("Global X11 shortcuts cannot cover a Wayland desktop.").to_string());
        }
        let (sender, receiver) = async_channel::bounded(1);
        let registration = Registration::start(&self.combo, sender)?;
        self.listener = Some(self.executor.spawn(async move {
            while receiver.recv().await.is_ok() {
                trigger();
            }
        }));
        self.registration = Some(registration);
        Ok(())
    }

    fn stop(&mut self) {
        self.listener = None;
        self.registration = None;
    }

    fn state(&self) -> ShortcutState {
        self.registration
            .as_ref()
            .map_or(ShortcutState::Stopped, |registration| {
                if registration.registered.load(Ordering::Acquire) {
                    ShortcutState::Registered
                } else {
                    ShortcutState::Unavailable
                }
            })
    }
}

struct Registration {
    connection: Arc<RustConnection>,
    root: u32,
    wake_window: u32,
    grabs: Vec<(u8, u16)>,
    alive: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
}

impl Registration {
    fn start(
        combo: &muxy_core::quick_terminal::keys::KeyCombo,
        sender: async_channel::Sender<()>,
    ) -> Result<Self, String> {
        if !combo.is_supported_shortcut() {
            return Err(tr!("Invalid Quick Terminal shortcut").to_string());
        }
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let connection = Arc::new(connection);
        let root = connection.setup().roots[screen].root;
        let wake_window = connection
            .generate_id()
            .map_err(|error| error.to_string())?;
        connection
            .create_window(
                0,
                wake_window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &xproto::CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
        let mut registration = Self {
            connection,
            root,
            wake_window,
            grabs: Vec::new(),
            alive: Arc::new(AtomicBool::new(true)),
            registered: Arc::new(AtomicBool::new(true)),
        };
        registration.grabs = registrations(&registration.connection, combo)?;
        grab(&registration.connection, root, &registration.grabs)?;
        let combo = combo.clone();
        let mut grabs = registration.grabs.clone();
        let connection = registration.connection.clone();
        let alive = registration.alive.clone();
        let registered = registration.registered.clone();
        std::thread::Builder::new()
            .name("muxy-quick-shortcut".into())
            .spawn(move || {
                let mut pressed = false;
                let mut released_at = None;
                while alive.load(Ordering::Acquire) {
                    let Ok(event) = connection.wait_for_event() else {
                        break;
                    };
                    if !alive.load(Ordering::Acquire) {
                        break;
                    }
                    match event {
                        Event::KeyPress(key)
                            if grabs.contains(&(key.detail, u16::from(key.state) & 0xff)) =>
                        {
                            if !pressed && released_at != Some(key.time) {
                                let _ = sender.try_send(());
                            }
                            pressed = true;
                        }
                        Event::KeyRelease(key)
                            if grabs.iter().any(|&(code, _)| code == key.detail) =>
                        {
                            pressed = false;
                            released_at = Some(key.time);
                        }
                        Event::MappingNotify(_) => {
                            let _ = connection.ungrab_key(0_u8, root, ModMask::ANY);
                            let restored = registrations(&connection, &combo)
                                .and_then(|next| {
                                    grab(&connection, root, &next)?;
                                    grabs = next;
                                    Ok(())
                                })
                                .is_ok();
                            if !restored {
                                let _ = connection.ungrab_key(0_u8, root, ModMask::ANY);
                            }
                            registered.store(restored, Ordering::Release);
                            let _ = connection.flush();
                            pressed = false;
                            released_at = None;
                        }
                        _ => {}
                    }
                }
                alive.store(false, Ordering::Release);
                registered.store(false, Ordering::Release);
                let _ = connection.ungrab_key(0_u8, root, ModMask::ANY);
                let _ = connection.flush();
            })
            .map_err(|error| error.to_string())?;
        Ok(registration)
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        if let Ok(cookie) = self.connection.ungrab_key(0_u8, self.root, ModMask::ANY) {
            let _ = cookie.check();
        }
        let _ = self.connection.destroy_window(self.wake_window);
        let _ = self.connection.flush();
    }
}

fn keysym(key: &str) -> Option<u32> {
    Some(match key {
        "space" => 0x20,
        "tab" => 0xff09,
        "return" => 0xff0d,
        "leftarrow" => 0xff51,
        "uparrow" => 0xff52,
        "rightarrow" => 0xff53,
        "downarrow" => 0xff54,
        _ => {
            let mut chars = key.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let code = u32::from(ch);
            if code <= 0xff {
                code
            } else {
                0x0100_0000 | code
            }
        }
    })
}

fn symbol_modifiers(symbols: &[u32], symbol: u32, modifiers: u16) -> Option<u16> {
    if symbols.first() == Some(&symbol) {
        return Some(modifiers);
    }
    if symbols.get(1) == Some(&symbol) {
        return Some(modifiers | u16::from(ModMask::SHIFT));
    }
    None
}

fn registrations(
    connection: &RustConnection,
    combo: &muxy_core::quick_terminal::keys::KeyCombo,
) -> Result<Vec<(u8, u16)>, String> {
    let symbol = keysym(&combo.key).ok_or_else(|| tr!("Unsupported physical key").to_string())?;
    let setup = connection.setup();
    let mapping = connection
        .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let per_key = usize::from(mapping.keysyms_per_keycode);
    if per_key == 0 {
        return Err(tr!("X11 returned an empty keyboard map.").to_string());
    }
    let effective = combo.effective_modifiers();
    let mut modifiers = 0_u16;
    if effective & CONTROL != 0 {
        modifiers |= u16::from(ModMask::CONTROL);
    }
    if effective & OPTION != 0 {
        modifiers |= u16::from(ModMask::M1);
    }
    if effective & SHIFT != 0 {
        modifiers |= u16::from(ModMask::SHIFT);
    }
    let modifier_map = connection
        .get_modifier_mapping()
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    let mut locks = u16::from(ModMask::LOCK);
    let per_modifier = modifier_map.keycodes.len() / 8;
    for (index, codes) in modifier_map
        .keycodes
        .chunks(per_modifier.max(1))
        .enumerate()
    {
        if codes.iter().any(|&code| {
            code.checked_sub(setup.min_keycode)
                .and_then(|index| {
                    mapping
                        .keysyms
                        .get(usize::from(index) * per_key..usize::from(index) * per_key + per_key)
                })
                .is_some_and(|symbols| {
                    symbols
                        .iter()
                        .any(|symbol| matches!(*symbol, 0xff7f | 0xff14))
                })
        }) {
            locks |= 1 << index;
        }
    }
    let mut grabs = Vec::new();
    for (index, symbols) in mapping.keysyms.chunks(per_key).enumerate() {
        let Some(modifiers) = symbol_modifiers(symbols, symbol, modifiers) else {
            continue;
        };
        let key = u8::try_from(index).map_err(|error| error.to_string())? + setup.min_keycode;
        for extra in 0..=u16::from(u8::MAX) {
            if extra & !locks == 0 {
                grabs.push((key, modifiers | extra));
            }
        }
    }
    grabs.sort_unstable();
    grabs.dedup();
    if grabs.is_empty() {
        return Err(
            tr!("The shortcut key is not available in the current keyboard layout.").to_string(),
        );
    }
    Ok(grabs)
}

fn grab(connection: &RustConnection, root: u32, grabs: &[(u8, u16)]) -> Result<(), String> {
    for &(key, modifiers) in grabs {
        connection
            .grab_key(
                false,
                root,
                ModMask::from(modifiers),
                key,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| {
                tr!(
                    "Could not register Quick Terminal shortcut; it may already be in use: %@",
                    error.to_string()
                )
                .to_string()
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_symbols_restore_consumed_shift_without_grabbing_other_groups() {
        let modifiers = u16::from(ModMask::CONTROL | ModMask::M1);
        let shifted = modifiers | u16::from(ModMask::SHIFT);
        let symbols = [u32::from('='), u32::from('+'), u32::from('~')];
        assert_eq!(
            symbol_modifiers(&symbols, u32::from('+'), modifiers),
            Some(shifted)
        );
        assert_eq!(
            symbol_modifiers(&symbols, u32::from('='), modifiers),
            Some(modifiers)
        );
        assert_eq!(symbol_modifiers(&symbols, u32::from('~'), modifiers), None);
        assert_eq!(
            symbol_modifiers(&[u32::from('a'), u32::from('A')], u32::from('a'), shifted),
            Some(shifted)
        );
    }
}
