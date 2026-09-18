use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};
use std::{marker::PhantomData, path::PathBuf, rc::Rc};

type Snapshot = Vec<Vec<(String, Vec<u8>)>>;

#[derive(Debug)]
pub enum Content {
    Files(Vec<PathBuf>),
    Image(Vec<u8>),
    Text,
}

pub fn read_content() -> Result<Content, String> {
    MainThreadMarker::new().ok_or("Clipboard requires the main thread")?;
    let board = NSPasteboard::generalPasteboard();
    let mut paths = Vec::new();
    if let Some(items) = board.pasteboardItems() {
        for item in &items {
            if let Some(value) = item.stringForType(&NSString::from_str("public.file-url"))
                && let Some(url) = NSURL::URLWithString(&value)
                && url.isFileURL()
                && let Some(path) = url.path()
            {
                paths.push(PathBuf::from(path.to_string()));
            }
        }
    }
    if !paths.is_empty() {
        return Ok(Content::Files(paths));
    }
    for identifier in [
        "public.png",
        "public.tiff",
        "public.jpeg",
        "com.compuserve.gif",
        "org.webmproject.webp",
    ] {
        if let Some(data) = board.dataForType(&NSString::from_str(identifier)) {
            if data.len() > 25 * 1024 * 1024 {
                return Err("Copied image is larger than 25 MiB".into());
            }
            return Ok(Content::Image(data.to_vec()));
        }
    }
    Ok(Content::Text)
}

#[derive(Debug)]
pub struct Lease {
    board: objc2::rc::Retained<NSPasteboard>,
    previous: Snapshot,
    count: isize,
    _main_thread: PhantomData<Rc<()>>,
}

impl Lease {
    pub fn capture() -> Result<Self, String> {
        MainThreadMarker::new().ok_or("Clipboard requires the main thread")?;
        Self::capture_board(NSPasteboard::generalPasteboard())
    }

    fn capture_board(board: objc2::rc::Retained<NSPasteboard>) -> Result<Self, String> {
        let count = board.changeCount();
        let mut previous = Vec::new();
        let mut total = 0;
        if let Some(items) = board.pasteboardItems() {
            for item in &items {
                let mut representations = Vec::new();
                for identifier in &item.types() {
                    let data = item
                        .dataForType(&identifier)
                        .ok_or("Could not preserve the clipboard")?;
                    total += data.len();
                    if total > 64 * 1024 * 1024 {
                        return Err(
                            "Clipboard is too large to preserve. Use inline image paths instead."
                                .into(),
                        );
                    }
                    representations.push((identifier.to_string(), data.to_vec()));
                }
                previous.push(representations);
            }
        }
        if board.changeCount() != count {
            return Err("Clipboard changed during capture. Try again.".into());
        }
        Ok(Self {
            board,
            previous,
            count,
            _main_thread: PhantomData,
        })
    }

    pub fn write_png(&mut self, png: Vec<u8>) -> Result<(), String> {
        let board = &self.board;
        if board.changeCount() != self.count {
            return Err("Clipboard changed during submission; sending stopped.".into());
        }
        let result = write(board, &vec![vec![("public.png".into(), png)]]);
        self.count = board.changeCount();
        result
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let board = &self.board;
        if board.changeCount() == self.count {
            let _ = write(board, &self.previous);
        }
    }
}

fn write(board: &NSPasteboard, snapshot: &Snapshot) -> Result<(), String> {
    let mut items = Vec::new();
    for representations in snapshot {
        let item = NSPasteboardItem::new();
        for (identifier, bytes) in representations {
            if !item.setData_forType(&NSData::with_bytes(bytes), &NSString::from_str(identifier)) {
                return Err("Could not write clipboard data".into());
            }
        }
        items.push(item);
    }
    let items = NSArray::from_retained_slice(&items);
    board.clearContents();
    if items.is_empty() {
        return Ok(());
    }
    let written: bool = unsafe { msg_send![board, writeObjects: &*items] };
    written
        .then_some(())
        .ok_or_else(|| "Could not replace clipboard contents".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_restores_every_format_and_preserves_external_changes() -> Result<(), String> {
        let board = NSPasteboard::pasteboardWithUniqueName();
        let original = vec![vec![
            ("public.utf8-plain-text".into(), b"hello".to_vec()),
            ("app.muxy.test".into(), vec![0, 1, 2]),
        ]];
        write(&board, &original)?;
        let mut lease = Lease::capture_board(board.clone())?;
        lease.write_png(vec![4, 5, 6])?;
        drop(lease);
        assert_eq!(Lease::capture_board(board.clone())?.previous, original);
        let mut lease = Lease::capture_board(board.clone())?;
        lease.write_png(vec![4, 5, 6])?;
        let external = vec![vec![(
            "public.utf8-plain-text".into(),
            b"new copy".to_vec(),
        )]];
        write(&board, &external)?;
        assert!(lease.write_png(vec![7]).is_err());
        drop(lease);
        assert_eq!(Lease::capture_board(board.clone())?.previous, external);
        board.clearContents();
        Ok(())
    }
}
