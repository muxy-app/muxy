use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEvent {
    Title(String),
    /// The working directory reported by OSC 7, as the program sent it.
    Directory(Vec<u8>),
    ClipboardWrite(String),
    Bell,
    Progress(muxy_protocol::SessionProgress),
}

#[derive(Debug, Default)]
pub(crate) struct Events {
    pub(crate) title: Rc<Cell<bool>>,
    pub(crate) directory: Rc<Cell<bool>>,
    pub(crate) clipboard_write: Rc<RefCell<Option<String>>>,
    pub(crate) bell: Rc<Cell<bool>>,
}
