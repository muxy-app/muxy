pub trait QuickTerminalSessionHandle {
    fn set_focused(&self, focused: bool);
    fn set_occluded(&self, occluded: bool);
    fn request_close(&self);
}

pub struct QuickTerminalSession<H> {
    surface: Option<H>,
    generation: u64,
    visible: bool,
    terminated: bool,
}

impl<H: QuickTerminalSessionHandle> Default for QuickTerminalSession<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: QuickTerminalSessionHandle> QuickTerminalSession<H> {
    pub fn new() -> Self {
        Self {
            surface: None,
            generation: 0,
            visible: false,
            terminated: false,
        }
    }

    pub fn show(&mut self, create: impl FnOnce() -> Result<H, String>) -> Result<u64, String> {
        if self.terminated {
            return Err("Quick Terminal session is terminated".to_owned());
        }
        if self.surface.is_none() {
            self.surface = Some(create()?);
            self.generation = self.generation.saturating_add(1).max(1);
        }
        let surface = self.surface.as_ref().expect("surface was created");
        surface.set_occluded(false);
        surface.set_focused(true);
        self.visible = true;
        Ok(self.generation)
    }

    pub fn hide(&mut self) {
        if let Some(surface) = &self.surface {
            surface.set_focused(false);
            surface.set_occluded(true);
        }
        self.visible = false;
    }

    pub fn process_exited(&mut self, generation: u64) -> bool {
        if generation != self.generation || self.surface.is_none() {
            return false;
        }
        self.surface.take();
        self.visible = false;
        true
    }

    pub fn reload(&mut self, reload: impl FnOnce(&H)) {
        if let Some(surface) = &self.surface {
            reload(surface);
        }
    }

    pub fn request_close(&self) -> bool {
        let Some(surface) = &self.surface else {
            return false;
        };
        surface.request_close();
        true
    }

    pub fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        self.visible = false;
        if let Some(surface) = self.surface.take() {
            surface.set_focused(false);
            surface.set_occluded(true);
            surface.request_close();
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    pub fn surface(&self) -> Option<&H> {
        self.surface.as_ref()
    }

    pub fn surface_mut(&mut self) -> Option<&mut H> {
        self.surface.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::{QuickTerminalSession, QuickTerminalSessionHandle};
    use std::cell::Cell;
    use std::rc::Rc;

    struct FakeHandle {
        focused: Rc<Cell<bool>>,
        occluded: Rc<Cell<bool>>,
        closes: Rc<Cell<usize>>,
    }

    struct FakeControls {
        focused: Rc<Cell<bool>>,
        occluded: Rc<Cell<bool>>,
        closes: Rc<Cell<usize>>,
    }

    impl QuickTerminalSessionHandle for FakeHandle {
        fn set_focused(&self, focused: bool) {
            self.focused.set(focused);
        }

        fn set_occluded(&self, occluded: bool) {
            self.occluded.set(occluded);
        }

        fn request_close(&self) {
            self.closes.set(self.closes.get() + 1);
        }
    }

    fn fake() -> (FakeHandle, FakeControls) {
        let controls = FakeControls {
            focused: Rc::new(Cell::new(false)),
            occluded: Rc::new(Cell::new(true)),
            closes: Rc::new(Cell::new(0)),
        };
        (
            FakeHandle {
                focused: controls.focused.clone(),
                occluded: controls.occluded.clone(),
                closes: controls.closes.clone(),
            },
            controls,
        )
    }

    #[test]
    fn quick_terminal_session_creates_lazily_and_retains_one_surface() {
        let mut session = QuickTerminalSession::new();
        assert!(!session.has_surface());
        let (handle, controls) = fake();
        let generation = session.show(|| Ok(handle)).unwrap();
        assert_eq!(generation, 1);
        assert!(controls.focused.get());
        assert!(!controls.occluded.get());
        session.hide();
        assert!(!controls.focused.get());
        assert!(controls.occluded.get());
        assert_eq!(
            session.show(|| panic!("must retain surface")).unwrap(),
            generation
        );
    }

    #[test]
    fn quick_terminal_session_releases_exited_generation_and_recreates() {
        let mut session = QuickTerminalSession::new();
        let first = session.show(|| Ok(fake().0)).unwrap();
        assert!(!session.process_exited(first + 1));
        assert!(session.has_surface());
        assert!(session.process_exited(first));
        assert!(!session.has_surface());
        assert!(!session.is_visible());
        assert_eq!(session.show(|| Ok(fake().0)).unwrap(), first + 1);
    }

    #[test]
    fn quick_terminal_session_reloads_only_the_live_surface() {
        let mut session = QuickTerminalSession::new();
        let reloads = Cell::new(0);
        session.reload(|_| reloads.set(reloads.get() + 1));
        session.show(|| Ok(fake().0)).unwrap();
        session.reload(|_| reloads.set(reloads.get() + 1));
        assert_eq!(reloads.get(), 1);
    }

    #[test]
    fn quick_terminal_session_close_request_keeps_surface_until_exit() {
        let mut session = QuickTerminalSession::new();
        let (handle, controls) = fake();
        let generation = session.show(|| Ok(handle)).unwrap();
        assert!(session.request_close());
        assert_eq!(controls.closes.get(), 1);
        assert!(session.has_surface());
        assert!(session.process_exited(generation));
        assert!(!session.has_surface());
    }

    #[test]
    fn quick_terminal_session_terminate_is_idempotent_and_blocks_recreation() {
        let mut session = QuickTerminalSession::new();
        let (handle, controls) = fake();
        session.show(|| Ok(handle)).unwrap();
        session.terminate();
        session.terminate();
        assert_eq!(controls.closes.get(), 1);
        assert!(session.is_terminated());
        assert!(session.show(|| Ok(fake().0)).is_err());
    }
}
