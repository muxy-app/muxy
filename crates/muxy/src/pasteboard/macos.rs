use super::{
    PasteboardContent, PasteboardError, PasteboardItem, PasteboardRepresentation,
    PasteboardSnapshot, classify,
};
use objc2::msg_send;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem};
use objc2_foundation::{MainThreadMarker, NSArray, NSData, NSString};

pub fn read_content() -> Result<PasteboardContent, PasteboardError> {
    Ok(classify(&capture()?))
}

pub fn capture() -> Result<PasteboardSnapshot, PasteboardError> {
    MainThreadMarker::new().ok_or(PasteboardError::Unavailable)?;
    snapshot(&NSPasteboard::generalPasteboard())
}

pub fn replace_with_png(contents: &[u8]) -> Result<PasteboardSnapshot, PasteboardError> {
    MainThreadMarker::new().ok_or(PasteboardError::Unavailable)?;
    let pasteboard = NSPasteboard::generalPasteboard();
    let previous = snapshot(&pasteboard)?;
    let replacement = PasteboardSnapshot {
        items: vec![PasteboardItem {
            representations: vec![PasteboardRepresentation {
                type_identifier: "public.png".to_owned(),
                data: contents.to_vec(),
            }],
        }],
    };
    if write_snapshot(&pasteboard, &replacement).is_err() {
        let _ = write_snapshot(&pasteboard, &previous);
        return Err(PasteboardError::WriteFailed);
    }
    Ok(previous)
}

pub fn restore(snapshot: &PasteboardSnapshot) -> Result<(), PasteboardError> {
    MainThreadMarker::new().ok_or(PasteboardError::Unavailable)?;
    write_snapshot(&NSPasteboard::generalPasteboard(), snapshot)
}

fn snapshot(pasteboard: &NSPasteboard) -> Result<PasteboardSnapshot, PasteboardError> {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Ok(PasteboardSnapshot::default());
    };
    let mut captured = Vec::with_capacity(items.len());
    for item in items.iter() {
        let types = item
            .types()
            .iter()
            .map(|type_identifier| type_identifier.to_string())
            .collect::<Vec<_>>();
        let representations = materialize_representations(types, |type_identifier| {
            let type_identifier = NSString::from_str(type_identifier);
            item.dataForType(&type_identifier).map(|data| data.to_vec())
        })?;
        captured.push(PasteboardItem { representations });
    }
    Ok(PasteboardSnapshot { items: captured })
}

fn materialize_representations(
    types: impl IntoIterator<Item = String>,
    mut read: impl FnMut(&str) -> Option<Vec<u8>>,
) -> Result<Vec<PasteboardRepresentation>, PasteboardError> {
    types
        .into_iter()
        .map(|type_identifier| {
            let data = read(&type_identifier).ok_or(PasteboardError::CaptureFailed)?;
            Ok(PasteboardRepresentation {
                type_identifier,
                data,
            })
        })
        .collect()
}

fn write_snapshot(
    pasteboard: &NSPasteboard,
    snapshot: &PasteboardSnapshot,
) -> Result<(), PasteboardError> {
    let mut items = Vec::with_capacity(snapshot.items.len());
    for item in &snapshot.items {
        let native = NSPasteboardItem::new();
        for representation in &item.representations {
            let type_identifier = NSString::from_str(&representation.type_identifier);
            let data = NSData::with_bytes(&representation.data);
            if !native.setData_forType(&data, &type_identifier) {
                return Err(PasteboardError::WriteFailed);
            }
        }
        items.push(native);
    }
    pasteboard.clearContents();
    if items.is_empty() {
        return Ok(());
    }
    let items = NSArray::<NSPasteboardItem>::from_retained_slice(&items);
    let written: bool = unsafe { msg_send![pasteboard, writeObjects: &*items] };
    written.then_some(()).ok_or(PasteboardError::WriteFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_snapshot_aborts_when_an_advertised_representation_cannot_materialize() {
        let result = materialize_representations(
            [
                "public.utf8-plain-text".to_owned(),
                "com.muxy.lazy".to_owned(),
            ],
            |type_identifier| {
                (type_identifier == "public.utf8-plain-text").then(|| b"text".to_vec())
            },
        );
        assert_eq!(result, Err(PasteboardError::CaptureFailed));
    }

    #[test]
    fn native_snapshot_round_trips_every_item_and_representation() {
        let pasteboard = NSPasteboard::pasteboardWithUniqueName();
        let expected = PasteboardSnapshot {
            items: vec![
                PasteboardItem {
                    representations: vec![
                        PasteboardRepresentation {
                            type_identifier: "public.file-url".to_owned(),
                            data: b"file:///tmp/a%20b.txt".to_vec(),
                        },
                        PasteboardRepresentation {
                            type_identifier: "com.muxy.custom".to_owned(),
                            data: vec![0, 1, 2, 255],
                        },
                    ],
                },
                PasteboardItem {
                    representations: vec![PasteboardRepresentation {
                        type_identifier: "public.utf8-plain-text".to_owned(),
                        data: b"text".to_vec(),
                    }],
                },
            ],
        };
        write_snapshot(&pasteboard, &expected).unwrap();
        let captured = snapshot(&pasteboard).unwrap();
        assert_eq!(captured, expected);
        let replacement = PasteboardSnapshot {
            items: vec![PasteboardItem {
                representations: vec![PasteboardRepresentation {
                    type_identifier: "public.png".to_owned(),
                    data: vec![9, 8, 7],
                }],
            }],
        };
        write_snapshot(&pasteboard, &replacement).unwrap();
        assert_eq!(snapshot(&pasteboard).unwrap(), replacement);
        write_snapshot(&pasteboard, &captured).unwrap();
        assert_eq!(snapshot(&pasteboard).unwrap(), expected);
        pasteboard.clearContents();
    }
}
