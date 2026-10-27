use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSPasteboard, NSView, NSWindow};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSPoint, NSString, NSURL, ns_string};

type DragEntered = unsafe extern "C-unwind" fn(&NSWindow, Sel, &AnyObject) -> usize;

thread_local! {
    static HANDLERS: RefCell<Vec<(&'static AnyClass, DragEntered)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn install(view: &NSView) -> Result<(), String> {
    let window = view.window().ok_or("file-drop view is detached")?;
    let class = std::iter::successors(Some(window.class()), |class| class.superclass())
        .find(|class| matches!(class.name().to_bytes(), b"GPUIWindow" | b"GPUIPanel"))
        .ok_or("file-drop window is not a GPUI window")?;
    HANDLERS.with(|handlers| -> Result<(), String> {
        let mut handlers = handlers.borrow_mut();
        if handlers.iter().any(|(installed, _)| *installed == class) {
            return Ok(());
        }
        let method = class
            .instance_method(sel!(draggingEntered:))
            .ok_or("GPUI file-drop handler is unavailable")?;
        unsafe {
            let replacement = std::mem::transmute::<DragEntered, Imp>(entered);
            let original = method.set_implementation(replacement);
            handlers.push((class, std::mem::transmute::<Imp, DragEntered>(original)));
        }
        Ok(())
    })?;
    window.registerForDraggedTypes(&NSArray::from_slice(&[
        ns_string!("public.file-url"),
        ns_string!("public.url"),
        ns_string!("public.utf8-plain-text"),
    ]));
    Ok(())
}

extern "C-unwind" fn entered(window: &NSWindow, selector: Sel, info: &AnyObject) -> usize {
    let original = HANDLERS.with(|handlers| {
        handlers
            .borrow()
            .iter()
            .find(|(class, _)| window.isKindOfClass(class))
            .map(|(_, handler)| *handler)
    });
    let Some(original) = original else {
        return 0;
    };
    let board: Retained<NSPasteboard> = unsafe { msg_send![info, draggingPasteboard] };
    if board
        .propertyListForType(ns_string!("NSFilenamesPboardType"))
        .is_some()
    {
        return unsafe { original(window, selector, info) };
    }
    let paths = paths(&board, |path| Path::new(path).exists());
    if paths.is_empty() {
        return 0;
    }
    let location = unsafe { msg_send![info, draggingLocation] };
    let drag = FileDrag::alloc(window.mtm()).set_ivars(Drag {
        paths: NSArray::from_retained_slice(&paths),
        location,
    });
    let drag: Retained<FileDrag> = unsafe { msg_send![super(drag), init] };
    unsafe { original(window, selector, &drag) }
}

#[derive(Debug)]
struct Drag {
    paths: Retained<NSArray<NSString>>,
    location: NSPoint,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyFileDrag"]
    #[ivars = Drag]
    #[derive(Debug)]
    struct FileDrag;

    unsafe impl NSObjectProtocol for FileDrag {}

    impl FileDrag {
        #[unsafe(method(draggingLocation))]
        fn location(&self) -> NSPoint { self.ivars().location }

        #[unsafe(method(draggingPasteboard))]
        fn pasteboard(&self) -> &Self { self }

        #[unsafe(method(propertyListForType:))]
        fn property_list(&self, kind: &NSString) -> Option<&NSArray<NSString>> {
            (kind == ns_string!("NSFilenamesPboardType")).then_some(&*self.ivars().paths)
        }
    }
);

fn file_url(text: &NSString) -> Option<Retained<NSString>> {
    let url = NSURL::URLWithString(text)?;
    if !url.isFileURL() {
        return None;
    }
    url.path().filter(|path| path.to_string().starts_with('/'))
}

fn paths(board: &NSPasteboard, exists: impl Fn(&str) -> bool) -> Vec<Retained<NSString>> {
    let mut files = Vec::new();
    if let Some(items) = board.pasteboardItems() {
        for item in &items {
            if let Some(value) = item.stringForType(ns_string!("public.file-url"))
                && let Some(path) = file_url(&value)
            {
                files.push(path);
            }
        }
    }
    if !files.is_empty() {
        return files;
    }
    if let Some(text) = board.stringForType(ns_string!("public.utf8-plain-text")) {
        return text_paths(&text.to_string(), exists);
    }
    board
        .stringForType(ns_string!("public.url"))
        .and_then(|text| file_url(&text))
        .into_iter()
        .collect()
}

fn text_paths(text: &str, exists: impl Fn(&str) -> bool) -> Vec<Retained<NSString>> {
    let mut paths = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if line.contains('\0') {
            return Vec::new();
        }
        let value = NSString::from_str(line);
        if line.starts_with("file://") {
            let Some(path) = file_url(&value) else {
                return Vec::new();
            };
            paths.push(path);
        } else if line.starts_with('/') && exists(line) {
            paths.push(value);
        } else {
            return Vec::new();
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(paths: Vec<Retained<NSString>>) -> Vec<String> {
        paths.into_iter().map(|path| path.to_string()).collect()
    }

    #[test]
    fn file_tree_multi_selection_preserves_spaces_and_decodes_urls() {
        assert_eq!(
            strings(text_paths(
                "/project/a b.txt\r\nfile:///project/c%23d.txt\n",
                |path| { path == "/project/a b.txt" }
            )),
            ["/project/a b.txt", "/project/c#d.txt"]
        );
        assert_eq!(
            strings(text_paths(
                "/project/report.txt \n/project/report.txt",
                |_| true
            )),
            ["/project/report.txt ", "/project/report.txt"]
        );
        for text in [
            "/project/a b.txt\nnot a path",
            "/project/missing.txt",
            "relative.txt",
            "https://example.com/file.txt",
            "/project/a\0b",
        ] {
            assert!(text_paths(text, |path| path == "/project/a b.txt").is_empty());
        }
    }
}
