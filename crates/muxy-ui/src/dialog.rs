use std::cell::Cell;
use std::io;
use std::rc::Rc;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle, NSApplication,
    NSControlStateValueOn, NSModalResponse, NSWindow,
};
use objc2_foundation::NSString;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConfirmationResponse {
    #[default]
    Cancelled,
    Confirmed {
        dont_ask_again: bool,
    },
}

#[derive(Debug)]
pub struct Confirmation {
    alert: Retained<NSAlert>,
    parent: Retained<NSWindow>,
    completed: Rc<Cell<bool>>,
}

impl Drop for Confirmation {
    fn drop(&mut self) {
        if !self.completed.replace(true) {
            self.parent
                .endSheet_returnCode(&self.alert.window(), NSAlertSecondButtonReturn);
        }
    }
}

pub fn confirm(
    window: &gpui::Window,
    title: &str,
    message: &str,
    confirm_label: &str,
    suppression_label: Option<&str>,
    on_complete: impl FnOnce(ConfirmationResponse) + 'static,
) -> io::Result<Confirmation> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| io::Error::other("native dialogs require the main thread"))?;
    let app = NSApplication::sharedApplication(main_thread);
    let windows = app.windows();
    let window_title = window.window_title();
    let mut matches = (0..windows.count())
        .map(|index| windows.objectAtIndex(index))
        .filter(|window| window.title().to_string() == window_title);
    let parent = matches
        .next()
        .ok_or_else(|| io::Error::other("native dialog parent window is closed"))?;
    if matches.next().is_some() {
        return Err(io::Error::other("native dialog parent window is ambiguous"));
    }
    if parent.attachedSheet().is_some() {
        return Err(io::Error::other("an application dialog is already open"));
    }
    let alert = NSAlert::new(main_thread);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert
        .addButtonWithTitle(&NSString::from_str(confirm_label))
        .setKeyEquivalent(&NSString::from_str("\r"));
    alert
        .addButtonWithTitle(&NSString::from_str("Cancel"))
        .setKeyEquivalent(&NSString::from_str("\u{1b}"));
    alert.setShowsSuppressionButton(suppression_label.is_some());
    let suppression = alert.suppressionButton();
    if let (Some(button), Some(label)) = (&suppression, suppression_label) {
        button.setTitle(&NSString::from_str(label));
    }
    let completed = Rc::new(Cell::new(false));
    let finished = Rc::clone(&completed);
    let callback = Cell::new(Some(on_complete));
    let handler = RcBlock::new(move |response| {
        finished.set(true);
        let dont_ask_again = suppression
            .as_ref()
            .is_some_and(|button| button.state() == NSControlStateValueOn);
        if let Some(callback) = callback.take() {
            callback(classify(response, dont_ask_again));
        }
    });
    alert.beginSheetModalForWindow_completionHandler(&parent, Some(&handler));
    Ok(Confirmation {
        alert,
        parent,
        completed,
    })
}

fn classify(response: NSModalResponse, dont_ask_again: bool) -> ConfirmationResponse {
    if response == NSAlertFirstButtonReturn {
        ConfirmationResponse::Confirmed { dont_ask_again }
    } else {
        ConfirmationResponse::Cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_confirmation_button_can_suppress_future_prompts() {
        for dont_ask_again in [false, true] {
            assert_eq!(
                classify(NSAlertFirstButtonReturn, dont_ask_again),
                ConfirmationResponse::Confirmed { dont_ask_again }
            );
            for response in [NSAlertSecondButtonReturn, -1000, 0, 1002] {
                assert_eq!(
                    classify(response, dont_ask_again),
                    ConfirmationResponse::Cancelled
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct FolderPicker {
    panel: Retained<objc2_app_kit::NSOpenPanel>,
    completed: Rc<Cell<bool>>,
}

impl Drop for FolderPicker {
    #[allow(
        unsafe_code,
        reason = "AppKit target/action cancellation accepts a nil sender."
    )]
    fn drop(&mut self) {
        if !self.completed.replace(true) {
            // SAFETY: No sender is passed; the retained panel is confined to the main thread.
            unsafe {
                self.panel.cancel(None);
            }
        }
    }
}

pub fn choose_folder(
    message: &str,
    directory: &std::path::Path,
    on_complete: impl FnOnce(Option<std::path::PathBuf>) + 'static,
) -> io::Result<FolderPicker> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| io::Error::other("native dialogs require the main thread"))?;
    let panel = objc2_app_kit::NSOpenPanel::openPanel(main_thread);
    panel.setMessage(Some(&NSString::from_str(message)));
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setDirectoryURL(Some(&objc2_foundation::NSURL::fileURLWithPath(
        &NSString::from_str(&directory.to_string_lossy()),
    )));
    let completed = Rc::new(Cell::new(false));
    let finished = Rc::clone(&completed);
    let result_panel = panel.clone();
    let callback = Cell::new(Some(on_complete));
    let handler = RcBlock::new(move |response| {
        finished.set(true);
        let path = (response == objc2_app_kit::NSModalResponseOK)
            .then(|| {
                result_panel
                    .URL()
                    .and_then(|url| url.path())
                    .map(|path| path.to_string().into())
            })
            .flatten();
        if let Some(callback) = callback.take() {
            callback(path);
        }
    });
    panel.beginWithCompletionHandler(&handler);
    Ok(FolderPicker { panel, completed })
}

/// A labelled native alert, confirmation, or text prompt.
#[derive(Clone, Debug, Default)]
pub struct DialogOptions {
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub default_button: Option<String>,
    pub cancel_button: Option<String>,
    pub input: Option<(String, String)>,
    pub style: String,
}

pub fn present(
    window: &gpui::Window,
    options: DialogOptions,
    on_complete: impl FnOnce(Option<String>) + 'static,
) -> io::Result<Confirmation> {
    let main_thread = MainThreadMarker::new()
        .ok_or_else(|| io::Error::other("dialogs require the main thread"))?;
    let windows = NSApplication::sharedApplication(main_thread).windows();
    let title = window.window_title();
    let parent = (0..windows.count())
        .map(|index| windows.objectAtIndex(index))
        .find(|window| window.title().to_string() == title)
        .ok_or_else(|| io::Error::other("dialog parent is closed"))?;
    if parent.attachedSheet().is_some() {
        return Err(io::Error::other("an application dialog is already open"));
    }
    let alert = NSAlert::new(main_thread);
    alert.setMessageText(&NSString::from_str(&options.title));
    alert.setInformativeText(&NSString::from_str(&options.message));
    alert.setAlertStyle(match options.style.as_str() {
        "warning" => NSAlertStyle::Warning,
        "critical" => NSAlertStyle::Critical,
        _ => NSAlertStyle::Informational,
    });
    for (index, label) in options.buttons.iter().enumerate() {
        let button = alert.addButtonWithTitle(&NSString::from_str(label));
        let key = if options.default_button.as_ref() == Some(label)
            || (options.default_button.is_none() && index == 0)
        {
            "\r"
        } else if options.cancel_button.as_ref() == Some(label) {
            "\u{1b}"
        } else {
            ""
        };
        button.setKeyEquivalent(&NSString::from_str(key));
    }
    let input = options.input.map(|(value, placeholder)| {
        let input = objc2_app_kit::NSTextField::textFieldWithString(
            &NSString::from_str(&value),
            main_thread,
        );
        input.setPlaceholderString(Some(&NSString::from_str(&placeholder)));
        input.setFrame(objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(0.0, 0.0),
            objc2_foundation::NSSize::new(320.0, 24.0),
        ));
        alert.setAccessoryView(Some(&input));
        alert.window().setInitialFirstResponder(Some(&input));
        input
    });
    let completed = Rc::new(Cell::new(false));
    let finished = completed.clone();
    let callback = Cell::new(Some(on_complete));
    let handler = RcBlock::new(move |response| {
        finished.set(true);
        let label = usize::try_from(response - NSAlertFirstButtonReturn)
            .ok()
            .and_then(|index| options.buttons.get(index));
        let value = match (label, &input) {
            (Some(label), Some(input)) if options.cancel_button.as_ref() != Some(label) => {
                Some(input.stringValue().to_string())
            }
            (Some(label), None) => Some(label.clone()),
            _ => None,
        };
        if let Some(callback) = callback.take() {
            callback(value);
        }
    });
    alert.beginSheetModalForWindow_completionHandler(&parent, Some(&handler));
    Ok(Confirmation {
        alert,
        parent,
        completed,
    })
}
