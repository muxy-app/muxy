use muxy_terminal::{Size, Terminal, TerminalError};

type Result = std::result::Result<(), TerminalError>;

#[test]
fn kitty_pixels_placements_replacement_png_and_deletion_survive_fragmented_output() -> Result {
    let mut terminal = Terminal::new(Size { cols: 20, rows: 3 }, 65536)?;
    for part in b"\x1b_Ga=T,f=32,s=1,v=1,i=1,c=2,r=1;/wAA/w==\x1b\\".chunks(3) {
        terminal.feed(part);
    }
    let first = terminal.graphics()?;
    assert_eq!(first.images.len(), 1);
    assert_eq!(first.images[0].rgba.as_ref(), &[255, 0, 0, 255]);
    assert_eq!(first.placements[0].size, [16, 16]);
    assert_eq!(first.placements[0].source, [0, 0, 1, 1]);
    // A restored copy numbers image generations afresh; pixels and placements must match.
    let mut archived = terminal.archive()?.graphics;
    archived.images[0].generation = first.images[0].generation;
    assert_eq!(archived, first);
    terminal.feed(b"\x1b[H\x1b_Ga=T,f=32,s=1,v=1,i=1,c=2,r=1;AAD//w==\x1b\\");
    let replacement = terminal.graphics()?;
    assert_eq!(replacement.images[0].rgba.as_ref(), &[0, 0, 255, 255]);
    assert_ne!(replacement.images[0].generation, first.images[0].generation);
    terminal.feed(b"\x1b[H\x1b_Ga=T,f=100,i=2,c=1,r=1;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+M/wHwAEAQH/cetH5QAAAABJRU5ErkJggg==\x1b\\");
    let png = terminal.graphics()?;
    assert_eq!(
        png.images
            .iter()
            .find(|image| image.id == 2)
            .map(|image| image.rgba.as_ref()),
        Some(&[0, 255, 0, 255][..])
    );
    terminal.feed(b"\x1b_Ga=d,d=A\x1b\\");
    assert!(terminal.graphics()?.placements.is_empty());
    Ok(())
}

#[test]
fn empty_image_crops_do_not_escape_into_snapshots() -> Result {
    let mut terminal = Terminal::new(Size { cols: 20, rows: 3 }, 65536)?;
    terminal.feed(b"\x1b_Ga=T,f=32,s=1,v=1,i=1,c=1,r=1,x=1;/wAA/w==\x1b\\");
    assert!(terminal.graphics()?.placements.is_empty());
    terminal.feed(b"visible");
    assert!(
        terminal.screen()?[0]
            .runs
            .iter()
            .any(|run| run.text.contains("visible"))
    );
    Ok(())
}

#[test]
fn kitty_unicode_placeholders_place_the_image_and_draw_blank_cells() -> Result {
    let mut terminal = Terminal::new(Size { cols: 20, rows: 4 }, 65536)?;
    let red = "/wAA".repeat(4);
    terminal.feed(format!("\x1b_Ga=T,U=1,i=42,f=24,s=2,v=2,c=4,r=2,q=2;{red}\x1b\\").as_bytes());
    let cells = ["\u{0305}", "\u{030D}"]
        .map(|row| {
            ["\u{0305}", "\u{030D}", "\u{030E}", "\u{0310}"]
                .map(|col| format!("\u{10EEEE}{row}{col}"))
                .concat()
        })
        .join("\r\n");
    terminal.feed(format!("\x1b[38;5;42m{cells}\x1b[39m").as_bytes());

    let graphics = terminal.graphics()?;
    assert_eq!(graphics.images.len(), 1);
    assert_eq!(graphics.images[0].id, 42);
    let placed: Vec<_> = graphics
        .placements
        .iter()
        .map(|p| (p.image, p.column, p.row, p.offset, p.size, p.source, p.z))
        .collect();
    // One placement per row of cells, below the text, together drawing the whole image.
    assert_eq!(
        placed,
        [
            (42, 0, 0, [0, 0], [32, 16], [0, 0, 2, 1], -1),
            (42, 0, 1, [0, 0], [32, 16], [0, 1, 2, 1], -1),
        ]
    );
    for row in &terminal.screen()?[..2] {
        let text: String = row.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text, "    ");
    }
    Ok(())
}

#[test]
fn kitty_placeholder_rows_stay_inside_an_odd_sized_image() -> Result {
    let mut terminal = Terminal::new(Size { cols: 20, rows: 4 }, 65536)?;
    let red = "/wAA".repeat(10);
    terminal.feed(format!("\x1b_Ga=T,U=1,i=7,f=24,s=2,v=5,c=2,r=2,q=2;{red}\x1b\\").as_bytes());
    let cells = ["\u{0305}", "\u{030D}"]
        .map(|row| {
            ["\u{0305}", "\u{030D}"]
                .map(|col| format!("\u{10EEEE}{row}{col}"))
                .concat()
        })
        .join("\r\n");
    terminal.feed(format!("\x1b[38;5;7m{cells}\x1b[39m").as_bytes());

    let graphics = terminal.graphics()?;
    let sources: Vec<_> = graphics.placements.iter().map(|p| p.source).collect();
    assert_eq!(sources, [[0, 0, 2, 3], [0, 3, 2, 2]]);
    assert_eq!(graphics.validate(), Ok(()));
    Ok(())
}
