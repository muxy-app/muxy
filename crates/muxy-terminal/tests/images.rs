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
    assert!(image.rgba.len() <= muxy_terminal::MAX_GRAPHICS_BYTES / 2);
    assert!(image.width > 1500 && image.width.abs_diff(3 * image.height) <= 3);
    assert_eq!(&image.rgba[..4], &[0, 0, 255, 255]);
    assert_eq!(graphics.placements[0].size[0], 10 * u32::from(CELL.width));
    Ok(())
}

#[test]
fn kitty_png_format_also_reads_other_image_files() -> Result {
    let folder = tempfile::tempdir()?;
    for name in ["photo.jpg", "photo.gif", "photo.bmp"] {
        let path = folder.path().join(name);
        image::RgbImage::from_pixel(8, 8, image::Rgb([0, 255, 0])).save(&path)?;
        let mut terminal = terminal()?;
        terminal.feed(format!("\x1b_Gi=3,a=T,f=100,t=f;{}\x1b\\", encoded(&path)).as_bytes());
        assert_eq!(terminal.take_pty_output(), b"\x1b_Gi=3;OK\x1b\\", "{name}");
        let graphics = terminal.graphics()?;
        let green = &graphics.images[0].rgba[..4];
        assert!(
            green[1] > 240 && green[0] < 16 && green[2] < 16,
            "{name}: {green:?}"
        );
    }
    Ok(())
}

#[test]
fn two_large_pictures_show_together() -> Result {
    let folder = tempfile::tempdir()?;
    let mut terminal = terminal()?;
    for (index, color) in [[255, 0, 0, 255], [0, 0, 255, 255]].into_iter().enumerate() {
        let path = folder.path().join(format!("{index}.png"));
        image::RgbaImage::from_pixel(3000, 1000, image::Rgba(color)).save(&path)?;
        terminal.feed(format!("\x1b[1;{}H", 1 + 10 * index).as_bytes());
        terminal.feed(format!("\x1b_Ga=T,f=100,t=f,c=8,C=1;{}\x1b\\", encoded(&path)).as_bytes());
    }
    let graphics = terminal.graphics()?;
    assert_eq!(graphics.images.len(), 2);
    assert_eq!(graphics.placements.len(), 2);
    Ok(())
}

#[test]
fn large_grayscale_pictures_are_shrunk_before_taking_colour() -> Result {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("gray.png");
    image::GrayImage::from_pixel(6000, 4000, image::Luma([200])).save(&path)?;
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Gi=4,a=T,f=100,t=f,c=8;{}\x1b\\", encoded(&path)).as_bytes());
    assert_eq!(terminal.take_pty_output(), b"\x1b_Gi=4;OK\x1b\\");
    let image = &terminal.graphics()?.images[0];
    assert!(image.rgba.len() <= muxy_terminal::MAX_GRAPHICS_BYTES / 2);
    assert_eq!(&image.rgba[..4], &[200, 200, 200, 255]);
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

/// A GIF of solid frames, each showing for its delay in milliseconds.
fn gif(colors: &[([u8; 3], u32)]) -> Vec<u8> {
    gif_of(2, 2, colors)
}

fn gif_of(width: u32, height: u32, colors: &[([u8; 3], u32)]) -> Vec<u8> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        let _ = encoder.set_repeat(Repeat::Infinite);
        for &([red, green, blue], delay) in colors {
            let picture =
                image::RgbaImage::from_pixel(width, height, image::Rgba([red, green, blue, 255]));
            let delay = image::Delay::from_numer_denom_ms(delay, 1);
            let _ = encoder.encode_frame(image::Frame::from_parts(picture, 0, 0, delay));
        }
    }
    bytes
}

const RED_THEN_BLUE: [([u8; 3], u32); 2] = [([255, 0, 0], 70), ([0, 0, 255], 130)];

/// The first pixel of each frame the terminal hands out for the only image.
fn frame_pixels(
    terminal: &mut Terminal,
) -> std::result::Result<Vec<[u8; 4]>, Box<dyn std::error::Error>> {
    let graphics = terminal.graphics()?;
    let image = &graphics.images[0];
    let keys: Vec<u64> = image.frame_keys().collect();
    let frames = terminal.image_frames(image.id, &keys)?;
    assert_eq!(
        frames.iter().map(|frame| frame.key).collect::<Vec<_>>(),
        keys
    );
    Ok(frames
        .iter()
        .map(|frame| [frame.rgba[0], frame.rgba[1], frame.rgba[2], frame.rgba[3]])
        .collect())
}

#[test]
fn kitty_animations_list_their_frames_and_hand_out_each_one() -> Result {
    use muxy_terminal::PlaybackState;
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b_Ga=T,i=5,f=32,s=1,v=1,q=2;/wAA/w==\x1b\\");
    let still = terminal.graphics()?.images[0].clone();
    assert!(still.frames.is_empty());
    let pixels = terminal.image_frames(5, &[still.generation])?;
    assert_eq!(pixels.len(), 1);
    assert_eq!(&pixels[0].rgba[..], &[255, 0, 0, 255]);
    // Asked for pixels it no longer has, a still image answers with its own.
    let late = terminal.image_frames(5, &[still.generation + 1])?;
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].key, still.generation);

    terminal.feed(b"\x1b_Ga=f,i=5,f=32,s=1,v=1,z=100,q=2;AAD//w==\x1b\\");
    terminal.feed(b"\x1b_Ga=a,i=5,r=1,z=50,q=2\x1b\\");
    let loading = terminal.graphics()?.images[0].clone();
    assert_eq!(
        loading
            .frames
            .iter()
            .map(|frame| frame.gap)
            .collect::<Vec<_>>(),
        [50, 100]
    );
    assert_eq!(loading.playback.state, PlaybackState::Stopped);
    assert_eq!(
        frame_pixels(&mut terminal)?,
        [[255, 0, 0, 255], [0, 0, 255, 255]]
    );

    // Starting it, and one more frame, leave the frames already read as they were.
    terminal.feed(b"\x1b_Ga=a,i=5,s=3,v=3,c=2,q=2\x1b\\");
    terminal.feed(b"\x1b_Ga=f,i=5,f=32,s=1,v=1,z=30,q=2;AP8A/w==\x1b\\");
    let playing = terminal.graphics()?.images[0].clone();
    assert_eq!(playing.playback.state, PlaybackState::Running);
    assert_eq!((playing.playback.frame, playing.playback.loops), (1, 2));
    assert_eq!(playing.frames.len(), 3);
    assert_eq!(playing.frames[..2], loading.frames[..]);

    // Drawing over a frame gives it a new key.
    terminal.feed(b"\x1b_Ga=f,i=5,r=1,f=32,s=1,v=1,q=2;AP8A/w==\x1b\\");
    let edited = terminal.graphics()?.images[0].clone();
    assert_ne!(edited.frames[0].key, playing.frames[0].key);
    assert_eq!(edited.frames[1..], playing.frames[1..]);
    assert!(
        terminal
            .image_frames(5, &[playing.frames[0].key])?
            .is_empty()
    );
    Ok(())
}

#[test]
fn iterm_gifs_keep_every_frame_and_play_forever() -> Result {
    use muxy_terminal::PlaybackState;
    let file = STANDARD.encode(gif(&RED_THEN_BLUE));
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b]1337;File=inline=1;width=4;height=2:{file}\x07").as_bytes());
    let graphics = terminal.graphics()?;
    assert_eq!(graphics.images.len(), 1);
    let image = &graphics.images[0];
    assert_eq!(
        image
            .frames
            .iter()
            .map(|frame| frame.gap)
            .collect::<Vec<_>>(),
        [70, 130]
    );
    assert_eq!(image.playback.state, PlaybackState::Running);
    assert_eq!((image.playback.frame, image.playback.loops), (0, 0));
    assert_eq!(&image.rgba[..4], &[255, 0, 0, 255]);
    assert_eq!(graphics.placements[0].size, [40, 40]);
    assert_eq!(
        frame_pixels(&mut terminal)?,
        [[255, 0, 0, 255], [0, 0, 255, 255]]
    );
    let cursor = terminal.cursor()?;
    assert_eq!((cursor.row, cursor.col), (1, 4));
    Ok(())
}

#[test]
fn kitty_gif_files_play_when_sent_and_shown_together() -> Result {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("spin.gif");
    std::fs::write(&path, gif(&RED_THEN_BLUE))?;
    let command = format!("\x1b_Gi=3,a=T,f=100,t=f;{}\x1b\\", encoded(&path));
    for size in [command.len(), 5] {
        let mut terminal = terminal()?;
        for part in command.as_bytes().chunks(size) {
            terminal.feed(part);
        }
        assert_eq!(terminal.take_pty_output(), b"\x1b_Gi=3;OK\x1b\\");
        let image = terminal.graphics()?.images[0].clone();
        assert_eq!(image.frames.len(), 2);
        assert_eq!(image.playback.state, muxy_terminal::PlaybackState::Running);
        assert_eq!(
            frame_pixels(&mut terminal)?,
            [[255, 0, 0, 255], [0, 0, 255, 255]]
        );
    }
    // Sent now and placed later, it is a still image of its first frame.
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Gi=3,a=t,f=100,t=f,q=2;{}\x1b\\", encoded(&path)).as_bytes());
    terminal.feed(b"\x1b_Gi=3,a=p,q=2\x1b\\");
    let graphics = terminal.graphics()?;
    assert!(graphics.images[0].frames.is_empty());
    assert_eq!(&graphics.images[0].rgba[..4], &[255, 0, 0, 255]);
    Ok(())
}

#[test]
fn one_reply_holds_only_the_frames_that_fit() -> Result {
    // Two frames of 1024 by 1100 pixels are over the size of one reply.
    let frame = |shade: u8| STANDARD.encode(vec![shade; 1024 * 1100 * 4]);
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b_Ga=T,i=2,f=32,s=1024,v=1100,q=2;{}\x1b\\", frame(255)).as_bytes());
    for shade in [128, 64] {
        let command = format!(
            "\x1b_Ga=f,i=2,f=32,s=1024,v=1100,z=40,q=2;{}\x1b\\",
            frame(shade)
        );
        terminal.feed(command.as_bytes());
    }
    let image = terminal.graphics()?.images[0].clone();
    let keys: Vec<u64> = image.frame_keys().collect();
    assert_eq!(keys.len(), 3);
    assert!(keys[0] != keys[1] && keys[1] != keys[2]);
    let first = terminal.image_frames(2, &keys)?;
    assert_eq!(first.len(), 1);
    assert_eq!((first[0].key, first[0].rgba[0]), (keys[0], 255));
    let rest = terminal.image_frames(2, &keys[1..])?;
    assert_eq!(rest.len(), 1);
    assert_eq!((rest[0].key, rest[0].rgba[0]), (keys[1], 128));
    Ok(())
}

#[test]
fn frames_too_large_for_one_command_arrive_whole() -> Result {
    // Each frame is several Kitty commands long once Muxy hands it on.
    let (width, height) = (64, 48);
    let file = STANDARD.encode(gif_of(width, height, &RED_THEN_BLUE));
    let mut terminal = terminal()?;
    terminal.feed(format!("\x1b]1337;File=inline=1:{file}\x07").as_bytes());
    let image = terminal.graphics()?.images[0].clone();
    assert_eq!((image.width, image.height), (width, height));
    let keys: Vec<u64> = image.frame_keys().collect();
    let frames = terminal.image_frames(image.id, &keys)?;
    assert_eq!(frames.len(), 2);
    for (frame, color) in frames.iter().zip([[255, 0, 0, 255], [0, 0, 255, 255]]) {
        assert_eq!(frame.rgba.len(), (width * height * 4) as usize);
        assert!(frame.rgba.chunks_exact(4).all(|pixel| pixel == color));
    }
    Ok(())
}

#[test]
fn two_converted_animations_keep_their_own_frames() -> Result {
    let mut terminal = terminal()?;
    for (column, colors) in [
        (1, RED_THEN_BLUE),
        (9, [([0, 255, 0], 40), ([255, 255, 0], 40)]),
    ] {
        let file = STANDARD.encode(gif(&colors));
        terminal.feed(format!("\x1b[1;{column}H").as_bytes());
        terminal.feed(
            format!("\x1b]1337;File=inline=1;width=2;doNotMoveCursor=1:{file}\x07").as_bytes(),
        );
    }
    let graphics = terminal.graphics()?;
    assert_eq!(graphics.images.len(), 2);
    let mut seconds = Vec::new();
    for image in &graphics.images {
        assert_eq!(image.frames.len(), 2);
        let keys: Vec<u64> = image.frame_keys().collect();
        let frames = terminal.image_frames(image.id, &keys)?;
        assert_eq!(frames.len(), 2);
        seconds.push(frames[1].rgba[..3].to_vec());
    }
    seconds.sort();
    assert_eq!(seconds, [[0, 0, 255], [255, 255, 0]]);
    Ok(())
}

#[test]
fn the_newest_images_are_listed_when_they_do_not_all_fit() -> Result {
    // Three pictures of about 4 MB each; one screen lists 8 MB.
    let pixels = STANDARD.encode(vec![255_u8; 1024 * 1000 * 4]);
    for ids in [[4, 5, 6], [2, 3, 1], [25, 26, 27]] {
        let mut terminal = terminal()?;
        for (index, id) in ids.into_iter().enumerate() {
            terminal.feed(format!("\x1b[1;{}H", 1 + 6 * index).as_bytes());
            let command =
                format!("\x1b_Ga=T,i={id},f=32,s=1024,v=1000,c=2,r=1,C=1,q=2;{pixels}\x1b\\");
            terminal.feed(command.as_bytes());
        }
        let graphics = terminal.graphics()?;
        let mut listed: Vec<u32> = graphics.images.iter().map(|image| image.id).collect();
        listed.sort_unstable();
        let mut newest = ids[1..].to_vec();
        newest.sort_unstable();
        assert_eq!(listed, newest, "{ids:?}");
        assert_eq!(graphics.placements.len(), 2);
    }
    Ok(())
}

#[test]
fn large_animations_show_together_and_a_saved_screen_keeps_them() -> Result {
    // Two animations of 44 frames, about 26 MB each.
    let side = 400;
    let frame = |shade: u8| STANDARD.encode(vec![shade; side * side * 4]);
    let mut terminal = terminal()?;
    for (index, id) in [5, 6].into_iter().enumerate() {
        terminal.feed(format!("\x1b[1;{}H", 1 + 6 * index).as_bytes());
        let first = format!(
            "\x1b_Ga=T,i={id},f=32,s={side},v={side},c=4,r=2,C=1,q=2;{}\x1b\\",
            frame(255)
        );
        terminal.feed(first.as_bytes());
        for shade in 1..44 {
            let next = format!(
                "\x1b_Ga=f,i={id},f=32,s={side},v={side},z=90,q=2;{}\x1b\\",
                frame(shade)
            );
            terminal.feed(next.as_bytes());
        }
        terminal.feed(format!("\x1b_Ga=a,i={id},r=1,z=90,s=3,v=1,q=2\x1b\\").as_bytes());
    }
    let graphics = terminal.graphics()?;
    let frames: Vec<usize> = graphics
        .images
        .iter()
        .map(|image| image.frames.len())
        .collect();
    assert_eq!(frames, [44, 44]);
    assert_eq!(graphics.placements.len(), 2);
    let saved = terminal.archive()?.graphics;
    assert_eq!(saved.images.len(), 2);
    assert_eq!(saved.placements.len(), 2);
    assert!(
        saved
            .images
            .iter()
            .all(|image| image.rgba[0] == 255 && image.frames.is_empty())
    );
    Ok(())
}
