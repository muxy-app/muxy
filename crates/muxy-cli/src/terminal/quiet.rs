use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

/// A backend that leaves out writes that wouldn't change the screen.
///
/// Ratatui shows and moves the cursor, and resets colors, after every draw,
/// even when no cell changed. The terminal UI draws many times a second, so
/// without this it writes to the terminal all the time while idle.
#[derive(Debug)]
pub(crate) struct Quiet<B> {
    inner: B,
    shown: Option<bool>,
    position: Option<Position>,
}

impl<B> Quiet<B> {
    pub(crate) const fn new(inner: B) -> Self {
        Self {
            inner,
            shown: None,
            position: None,
        }
    }

    /// Forgets what the terminal shows, so the next cursor change is written.
    fn forget(&mut self) {
        self.shown = None;
        self.position = None;
    }
}

impl<B: Backend> Backend for Quiet<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut content = content.peekable();
        if content.peek().is_none() {
            return Ok(());
        }
        self.position = None;
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.position = None;
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        if self.shown == Some(false) {
            return Ok(());
        }
        self.inner.hide_cursor()?;
        self.shown = Some(false);
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        if self.shown == Some(true) {
            return Ok(());
        }
        self.inner.show_cursor()?;
        self.shown = Some(true);
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        if self.position == Some(position) {
            return Ok(());
        }
        self.inner.set_cursor_position(position)?;
        self.position = Some(position);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.forget();
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.forget();
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io::{self, Write};
    use std::rc::Rc;

    use ratatui::backend::{Backend, CrosstermBackend};
    use ratatui::layout::Rect;
    use ratatui::{Terminal, TerminalOptions, Viewport};

    use super::Quiet;

    #[derive(Clone, Default)]
    struct Output(Rc<RefCell<Vec<u8>>>);

    impl Output {
        fn take(&self) -> String {
            String::from_utf8(std::mem::take(&mut *self.0.borrow_mut())).expect("ascii output")
        }
    }

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    type Recorded = Terminal<Quiet<CrosstermBackend<Output>>>;

    fn terminal() -> (Recorded, Output) {
        let output = Output::default();
        let terminal = Terminal::with_options(
            Quiet::new(CrosstermBackend::new(output.clone())),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 20, 4)),
            },
        )
        .expect("a fixed viewport needs no real terminal");
        (terminal, output)
    }

    fn draw(terminal: &mut Recorded, output: &Output, text: &str) -> String {
        output.take();
        terminal
            .draw(|frame| {
                frame.render_widget(text, frame.area());
                frame.set_cursor_position((3, 1));
            })
            .expect("drawing into memory can't fail");
        output.take()
    }

    #[test]
    fn an_unchanged_frame_writes_nothing() {
        let (mut terminal, output) = terminal();
        assert!(!draw(&mut terminal, &output, "idle").is_empty());
        assert_eq!(draw(&mut terminal, &output, "idle"), "");
        assert_eq!(draw(&mut terminal, &output, "idle"), "");
    }

    #[test]
    fn a_changed_cell_is_written_and_the_cursor_put_back() {
        let (mut terminal, output) = terminal();
        draw(&mut terminal, &output, "idle");
        let written = draw(&mut terminal, &output, "busy");
        assert!(written.contains("busy"));
        assert!(written.ends_with("\u{1b}[2;4H"));
    }

    #[test]
    fn the_cursor_is_written_again_after_a_clear() {
        let (mut terminal, output) = terminal();
        draw(&mut terminal, &output, "idle");
        terminal
            .backend_mut()
            .clear()
            .expect("clearing memory can't fail");
        let written = draw(&mut terminal, &output, "idle");
        assert!(written.contains("\u{1b}[?25h"));
        assert!(written.contains("\u{1b}[2;4H"));
    }
}
