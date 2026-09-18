use std::cell::Cell;

use dispatch2::DispatchQueue;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{NSMenu, NSMenuItem, NSView};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
}

impl Item {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            checked: false,
            separator: false,
        }
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<Option<usize>>]
    #[name = "MuxyNativeMenuTarget"]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}

    impl MenuTarget {
        #[unsafe(method(choose:))]
        fn choose(&self, item: &NSMenuItem) { self.ivars().set(usize::try_from(item.tag()).ok()); }
    }
);

pub fn show(
    items: Vec<Item>,
    anchor: gpui::Point<gpui::Pixels>,
    window: &gpui::Window,
) -> async_channel::Receiver<Option<usize>> {
    let (sender, receiver) = async_channel::bounded(1);
    let location = screen_location(anchor, window);
    DispatchQueue::main().exec_async(move || {
        let result = location.and_then(|location| {
            MainThreadMarker::new().and_then(|mtm| show_on_main(&items, location, mtm))
        });
        let _ = sender.try_send(result);
    });
    receiver
}

fn screen_location(anchor: gpui::Point<gpui::Pixels>, window: &gpui::Window) -> Option<NSPoint> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
    else {
        return None;
    };
    let view = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?;
    let y = f64::from(f32::from(anchor.y));
    let point = NSPoint::new(
        f64::from(f32::from(anchor.x)),
        if view.isFlipped() {
            y
        } else {
            view.bounds().size.height - y
        },
    );
    let point = view.convertPoint_toView(point, None);
    Some(view.window()?.convertPointToScreen(point))
}

fn show_on_main(items: &[Item], location: NSPoint, mtm: MainThreadMarker) -> Option<usize> {
    let target = MenuTarget::alloc(mtm).set_ivars(Cell::new(None));
    let target: Retained<MenuTarget> = unsafe { msg_send![super(target), init] };
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::new());
    menu.setAutoenablesItems(false);
    for (index, entry) in items.iter().enumerate() {
        if entry.separator {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
        }
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(&entry.label),
                Some(sel!(choose:)),
                &NSString::new(),
            )
        };
        item.setEnabled(entry.enabled);
        item.setState(isize::from(entry.checked));
        item.setTag(isize::try_from(index).ok()?);
        unsafe {
            item.setTarget(Some(&target));
        }
        menu.addItem(&item);
    }
    menu.popUpMenuPositioningItem_atLocation_inView(None, location, None);
    target.ivars().get()
}
