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
    assert_eq!(terminal.archive()?.graphics, first);
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
