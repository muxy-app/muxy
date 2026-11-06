use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use muxy_terminal::{CellSize, Size, Terminal};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

const SIZE: Size = Size { cols: 20, rows: 5 };
const CELL: CellSize = CellSize {
    width: 10,
    height: 20,
};
/// A 1x1 green PNG.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+M/wHwAEAQH/cetH5QAAAABJRU5ErkJggg==";
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn terminal() -> std::result::Result<Terminal, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(SIZE, 65536)?;
    terminal.set_cell_size(CELL)?;
    Ok(terminal)
}

fn write_png(path: &Path) -> Result {
    std::fs::write(path, STANDARD.decode(PNG)?)?;
    Ok(())
}

fn encoded(path: &Path) -> String {
    STANDARD.encode(path.as_os_str().as_encoded_bytes())
}

fn only_pixels(
    terminal: &mut Terminal,
) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    let graphics = terminal.graphics()?;
    assert_eq!(graphics.images.len(), 1);
    assert_eq!(graphics.placements.len(), 1);
    Ok(graphics.images[0].rgba.to_vec())
}

#[test]
fn kitty_file_transfer_shows_the_image_from_the_issue_repro() -> Result {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("apple.png");
    write_png(&path)?;
    let mut terminal = terminal()?;
    let command = format!("\x1b_GX=1,a=T,f=100,t=f,c=5;{}\x1b\\", encoded(&path));
    for part in command.as_bytes().chunks(7) {
        terminal.feed(part);
    }
    assert_eq!(only_pixels(&mut terminal)?, GREEN);
    // X=1 starts the image one pixel into its cell; it still ends at column 5.
    let placement = &terminal.graphics()?.placements[0];
    assert_eq!(placement.offset[0], 1);
    assert_eq!(
        placement.offset[0] + placement.size[0],
        5 * u32::from(CELL.width)
    );
    // The file medium never deletes the file.
    assert!(path.exists());
    Ok(())
}

#[test]
fn kitty_pictures_over_the_image_budget_are_shrunk_to_fit() -> Result {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("header.png");
    image::RgbaImage::from_pixel(3000, 1000, image::Rgba([0, 0, 255, 255])).save(&path)?;
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Gi=9,a=T,f=100,t=f,c=10;{}\x1b\\", encoded(&path)).as_bytes());
    assert_eq!(terminal.take_pty_output(), b"\x1b_Gi=9;OK\x1b\\");
    let graphics = terminal.graphics()?;
    let image = &graphics.images[0];
    assert!(image.rgba.len() <= muxy_terminal::MAX_GRAPHICS_BYTES);
    assert!(image.width > 2000 && image.width.abs_diff(3 * image.height) <= 3);
    assert_eq!(&image.rgba[..4], &[0, 0, 255, 255]);
    assert_eq!(graphics.placements[0].size[0], 10 * u32::from(CELL.width));
    Ok(())
}

#[test]
fn kitty_temporary_file_is_loaded_and_deleted() -> Result {
    let folder = tempfile::Builder::new().tempdir_in(std::env::temp_dir())?;
    let path = folder.path().join("tty-graphics-protocol-test.png");
    write_png(&path)?;
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Ga=T,f=100,t=t;{}\x1b\\", encoded(&path)).as_bytes());
    assert_eq!(only_pixels(&mut terminal)?, GREEN);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn kitty_queries_answer_ok_for_the_file_medium() -> Result {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("probe.png");
    write_png(&path)?;
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Gi=31,a=q,t=f,f=100;{}\x1b\\", encoded(&path)).as_bytes());
    assert_eq!(terminal.take_pty_output(), b"\x1b_Gi=31;OK\x1b\\");
    assert!(terminal.graphics()?.images.is_empty());
    Ok(())
}

#[cfg(unix)]
#[test]
fn kitty_file_transfer_refuses_fifos_and_folders_without_blocking() -> Result {
    let folder = tempfile::tempdir()?;
    let fifo = folder.path().join("fifo");
    let status = std::process::Command::new("mkfifo").arg(&fifo).status()?;
    assert!(status.success());
    let paths = [fifo, folder.path().to_path_buf()];
    let (sender, receiver) = mpsc::channel();
    // A blocked open would hang this thread, so the test waits with a deadline.
    std::thread::spawn(move || {
        let replies = paths
            .iter()
            .map(|path| -> std::result::Result<_, String> {
                let mut terminal = Terminal::new(SIZE, 65536).map_err(|e| e.to_string())?;
                terminal
                    .feed(format!("\x1b_Gi=7,a=T,t=f,f=100;{}\x1b\\", encoded(path)).as_bytes());
                let images = terminal.graphics().map_err(|e| e.to_string())?.images.len();
                Ok((terminal.take_pty_output(), images))
            })
            .collect::<Vec<_>>();
        let _ = sender.send(replies);
    });
    let replies = receiver.recv_timeout(Duration::from_secs(10))?;
    for reply in replies {
        let (reply, images) = reply?;
        assert!(reply.starts_with(b"\x1b_Gi=7;"), "{reply:?}");
        assert!(!reply.starts_with(b"\x1b_Gi=7;OK"), "{reply:?}");
        assert_eq!(images, 0);
    }
    Ok(())
}

#[test]
fn sixel_is_drawn_and_leaves_the_cursor_on_its_last_row() -> Result {
    // Red, 2 pixels wide and 30 tall: two rows of 20-pixel cells.
    let sixel = b"abc\x1bP0;1;0q#1;2;100;0;0!2~-!2~-!2~-!2~-!2~\x1b\\x";
    for size in [1, 4, sixel.len()] {
        let mut terminal = terminal()?;
        for part in sixel.chunks(size) {
            terminal.feed(part);
        }
        let graphics = terminal.graphics()?;
        assert_eq!(graphics.images.len(), 1);
        let image = &graphics.images[0];
        assert_eq!((image.width, image.height), (2, 30));
        assert_eq!(&image.rgba[..4], &[255, 0, 0, 255]);
        let placement = &graphics.placements[0];
        assert_eq!((placement.column, placement.row), (3, 0));
        assert_eq!(placement.size, [2, 30]);
        // The cursor stays at the image's left edge on its last row.
        let cursor = terminal.cursor()?;
        assert_eq!((cursor.row, cursor.col), (1, 4));
        let rows = terminal.screen()?;
        let text = |row: usize| {
            rows[row]
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
        };
        assert_eq!(text(0).trim_end(), "abc");
        assert_eq!(text(1).trim_end(), "   x");
    }
    Ok(())
}

#[test]
fn iterm_inline_images_fill_the_requested_cells() -> Result {
    let mut terminal = terminal()?;
    terminal.feed(
        format!("\x1b]1337;File=inline=1;width=4;height=2;preserveAspectRatio=0:{PNG}\x07")
            .as_bytes(),
    );
    assert_eq!(only_pixels(&mut terminal)?, GREEN);
    let placement = &terminal.graphics()?.placements[0];
    assert_eq!((placement.column, placement.row), (0, 0));
    assert_eq!(placement.size, [40, 40]);
    // Like iTerm2, the cursor ends right of the image on its last row.
    let cursor = terminal.cursor()?;
    assert_eq!((cursor.row, cursor.col), (1, 4));
    Ok(())
}

#[test]
fn iterm_downloads_and_unknown_commands_show_nothing() -> Result {
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b]1337;File=name=YQ==:{PNG}\x07ok").as_bytes());
    assert!(terminal.graphics()?.images.is_empty());
    let rows = terminal.screen()?;
    assert_eq!(
        rows[0]
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>()
            .trim_end(),
        "ok"
    );
    Ok(())
}

#[test]
fn device_attributes_advertise_sixel_and_keep_the_other_answers() -> Result {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b[c");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?62;4;22c");
    terminal.feed(b"\x1b[>c");
    assert_eq!(terminal.take_pty_output(), b"\x1b[>1;0;0c");
    terminal.feed(b"\x1b[=c");
    assert_eq!(terminal.take_pty_output(), b"\x1bP!|00000000\x1b\\");
    Ok(())
}

#[test]
fn size_queries_report_cells_and_pixels() -> Result {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b[16t");
    assert_eq!(terminal.take_pty_output(), b"\x1b[6;20;10t");
    terminal.feed(b"\x1b[14t");
    assert_eq!(terminal.take_pty_output(), b"\x1b[4;100;200t");
    terminal.feed(b"\x1b[18t");
    assert_eq!(terminal.take_pty_output(), b"\x1b[8;5;20t");
    Ok(())
}

#[test]
fn other_dcs_requests_still_reach_the_terminal() -> Result {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1bP$qm\x1b\\");
    let reply = terminal.take_pty_output();
    assert!(reply.starts_with(b"\x1bP1$r"), "{reply:?}");
    Ok(())
}

#[test]
fn a_dropped_image_still_ends_an_unfinished_sequence() -> Result {
    for input in [
        &b"\x1b[1;\x1bPq\x1b\\AB"[..],
        b"\x1b[1;\x1b]1337;File=name=YQ==:AAAA\x07AB",
    ] {
        let mut terminal = terminal()?;
        terminal.feed(input);
        let rows = terminal.screen()?;
        let text: String = rows[0].runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text.trim_end(), "AB", "{input:?}");
    }
    Ok(())
}

#[test]
fn sixel_pictures_over_the_image_budget_are_shrunk_but_keep_their_width() -> Result {
    let band = "#1!3000~-".repeat(167);
    let sixel = format!("\x1bP0;1;0q#1;2;100;0;0{band}\x1b\\");
    let mut terminal = terminal()?;
    terminal.feed(sixel.as_bytes());
    let graphics = terminal.graphics()?;
    let image = &graphics.images[0];
    assert!(image.rgba.len() <= muxy_terminal::MAX_GRAPHICS_BYTES);
    assert_eq!(&image.rgba[..4], &[255, 0, 0, 255]);
    assert_eq!(graphics.placements[0].size[0], 3000);
    Ok(())
}
