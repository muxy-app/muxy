//! API for the images that programs send with the
//! [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/).
//!
//! The terminal keeps every image a program sends, and every placement of an
//! image on screen, in an image storage. Each screen (main and alternate) has
//! its own. [`Graphics`] is a handle to the storage of the active screen. Use
//! it to:
//!
//!   - Read the pixels of an image so you can draw it.
//!   - Play animated images (with [`Terminal::kitty_graphics_animation_tick`]).
//!   - Inspect the images and placements the storage holds.
//!
//! This API doesn't tell you where to draw images. The render state does
//! that. See [Drawing images](#drawing-images) below.
//!
//! ## Setting up
//!
//! Kitty graphics are off until you give the terminal a storage limit with
//! [`Terminal::set_kitty_image_storage_limit`]. To accept PNG images, also
//! install a PNG decoder with [`set_png_decoder`].
//!
//! Get the storage handle with [`Terminal::kitty_graphics`]. The handle is
//! borrowed from the terminal. It stays valid until the next call that
//! changes the terminal, such as [`Terminal::vt_write`] or
//! [`Terminal::reset`], so get it again after each of those.
//!
//! ## Drawing images
//!
//! Use the render state's image list
//! ([`ImageIterator`](crate::render::ImageIterator)) to find what to draw. It
//! lists every Kitty image on screen, however the program placed it:
//!
//!   - At a cell, so the image scrolls with the text.
//!   - With placeholder cells (see [Unicode placeholders](#unicode-placeholders)
//!     below).
//!   - Relative to another image.
//!
//! Each entry already has its position, size, source rectangle, layer and
//! z-index worked out, so one drawing routine handles all of them. See
//! [Images](crate::RenderState#images) in the render state documentation for
//! the details.
//!
//! The storage only provides the pixels. Each entry in the list has an
//! [image ID](crate::render::ImageIteration::image_id). Look it up with
//! [`Graphics::image`], then read the image's [size](Image::width),
//! [pixel format](Image::format) and [pixels](Image::data). The pixels are
//! always decoded and uncompressed, so you can upload them as they are.
//!
//! Don't also draw images from this API's [`PlacementIterator`]. The render
//! state's list already includes those placements, so they would be drawn
//! twice. See [Inspecting the storage](#inspecting-the-storage) below for what
//! the iterator is for.
//!
//! ## Knowing what changed
//!
//! Most frames don't change any image, so a renderer can skip most of its
//! image work. Two values tell you what to redo:
//!
//!   - [`Snapshot::image_placements_generation`](crate::render::Snapshot::image_placements_generation)
//!     belongs to the render state's whole list. It tells you when to read
//!     the list again, for example after an image moved because the text
//!     scrolled.
//!   - [`Image::generation`] belongs to a single image. It tells you when to
//!     upload that image's pixels again. It changes when a program sends the
//!     image again and on every animation frame. The render state reports
//!     the same value for each entry as
//!     [`ImageIteration::image_generation`](crate::render::ImageIteration::image_generation),
//!     so you can check your texture cache without touching the storage.
//!
//! Use the image generation as part of your texture cache key. Don't rely on
//! the image's size or data length instead: a program can send new pixels
//! with the same size under the same ID.
//!
//! For a frame loop that reads the list only when it changed, see
//! [Knowing when images changed](crate::RenderState#knowing-when-images-changed)
//! in the render state documentation, or the [example](#example) below.
//!
//! The storage also has a generation of its own, [`Graphics::generation`]. It
//! changes whenever a program sends, places or deletes an image, but not when
//! images only move on screen. It can't tell you when to read the render
//! state's list again. It is useful for work that only depends on what the
//! storage holds, such as freeing textures of images that were deleted.
//!
//! The two storage generations are never reused, not even across the main and
//! alternate screens, terminal resets, or different terminals. That makes
//! either one safe to use as a cache key on its own. The render state's
//! placements generation is different: only compare it with the last value
//! you read from the same render state.
//!
//! ## Animation
//!
//! The Kitty graphics protocol lets programs send images with several frames
//! and ask the terminal to play them, much like an animated GIF. The terminal
//! stores every frame, but it does not keep its own timer. Your application
//! drives playback by calling [`Terminal::kitty_graphics_animation_tick`].
//!
//! Each call shows whichever frame is due at the time you pass in. When an
//! image moves to a new frame, two things change:
//!
//!   - [`Image::data`] returns the pixels of the new frame.
//!   - [`Image::generation`] returns a new value.
//!
//! If you already cache textures by image generation (see
//! [Knowing what changed](#knowing-what-changed) above), new frames appear
//! without any extra work.
//!
//! The tick also tells you when the next frame is due. Use that time to
//! schedule a single redraw instead of calling the tick in a loop:
//!
//!   1. Call the tick each time you prepare a frame to draw, as long as the
//!      storage holds at least one image.
//!   2. If it returns `Some` time, schedule one redraw at the returned time.
//!      Don't schedule it sooner than your shortest frame interval (for
//!      example 8 ms for a 120 Hz display).
//!   3. If it returns `None`, nothing is playing. Don't schedule anything.
//!
//! You never need to watch for animations starting. A program starts or
//! resumes playback by writing a command to the terminal, and that write
//! already gives you a reason to redraw. The tick on that redraw picks up the
//! animation. A terminal with no playing animations costs nothing extra.
//!
//! Here is a helper, `tick_animations`, that follows these steps. A renderer
//! calls it once per frame and uses the result to schedule the next redraw.
//! The example plays a two-frame animation with that helper. It uses a
//! pretend clock so that it runs instantly:
//!
//! ```
//! use libghostty_vt::{Terminal, error};
//!
//! /// The shortest time between two redraws. About 8 ms suits a 120 Hz display.
//! const MIN_FRAME_INTERVAL_MS: u64 = 8;
//!
//! /// Advance any playing animations while preparing a frame to draw.
//! ///
//! /// Call this once per frame, before drawing images. It returns how many
//! /// milliseconds to wait before the next redraw, or `None` if no animation
//! /// is playing. In that case, don't schedule a redraw for animations at
//! /// all. A program that starts an animation does so by writing to the
//! /// terminal, which already causes a redraw.
//! ///
//! /// `now_ms` comes from a clock you choose, such as milliseconds since your
//! /// app started. Always use the same clock with the same starting point.
//! fn tick_animations(terminal: &mut Terminal<'_, '_>, now_ms: u64) -> error::Result<Option<u64>> {
//!     let Some(next_ms) = terminal.kitty_graphics_animation_tick(now_ms)? else {
//!         return Ok(None);
//!     };
//!
//!     // `next_ms` is an absolute time. Turn it into a delay from now.
//!     let delay_ms = next_ms.saturating_sub(now_ms);
//!     Ok(Some(delay_ms.max(MIN_FRAME_INTERVAL_MS)))
//! }
//!
//! /// Print the first pixel and the generation of an image.
//! fn print_image(
//!     terminal: &Terminal<'_, '_>,
//!     image_id: u32,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let graphics = terminal.kitty_graphics()?;
//!     let image = graphics.image(image_id).ok_or("no such image")?;
//!     let pixels = image.data()?.ok_or("pixels still loading")?;
//!     let generation = image.generation()?;
//!     print!(
//!         "pixel=({},{},{}) generation={generation}",
//!         pixels[0], pixels[1], pixels[2],
//!     );
//!     Ok(())
//! }
//!
//! # /// The first pixel and the generation of image 2, for the asserts.
//! # fn image(terminal: &Terminal<'_, '_>) -> ([u8; 3], u64) {
//! #     let graphics = terminal.kitty_graphics().unwrap();
//! #     let image = graphics.image(2).unwrap();
//! #     let pixels = image.data().unwrap().unwrap();
//! #     ([pixels[0], pixels[1], pixels[2]], image.generation().unwrap())
//! # }
//! #
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut terminal = Terminal::new(80, 24)?;
//! terminal.set_kitty_image_storage_limit(64 * 1024 * 1024)?;
//!
//! // These commands set up the animation:
//! //
//! //   a=T  Send a red 1x1 RGBA image with ID 2 and display it.
//! //   a=f  Add a blue second frame that shows for 40 ms.
//! //   a=a  Make the first frame show for 30 ms. It starts at 0 ms,
//! //        which would mean the frame is skipped.
//! //   a=a  Start playing on a loop (s=3).
//! //
//! // q=2 tells the terminal not to send replies.
//! terminal.vt_write(
//!     b"\x1b_Ga=T,f=32,i=2,s=1,v=1,q=2;/wAA/w==\x1b\\\
//!       \x1b_Ga=f,f=32,i=2,s=1,v=1,z=40,q=2;AAD//w==\x1b\\\
//!       \x1b_Ga=a,i=2,r=1,z=30,q=2\x1b\\\
//!       \x1b_Ga=a,i=2,s=3,q=2\x1b\\",
//! );
//!
//! // Draw three frames, following each delay. A real app waits for the
//! // delay that `tick_animations` returns. To keep this example quick, a
//! // pretend clock jumps ahead by that delay instead.
//! let mut now_ms = 0;
//! # let mut frames = Vec::new();
//! for _ in 0..3 {
//!     let delay_ms = tick_animations(&mut terminal, now_ms)?;
//!
//!     // The tick invalidates image handles, so `print_image` looks the
//!     // image up again. The generation changes whenever the frame does.
//!     print!("  t={now_ms} ms: ");
//!     print_image(&terminal, 2)?;
//! #   frames.push((now_ms, image(&terminal), delay_ms));
//!     let Some(delay_ms) = delay_ms else {
//!         println!(", nothing playing");
//!         break;
//!     };
//!     println!(", next redraw in {delay_ms} ms");
//!
//!     now_ms += delay_ms;
//! }
//! # // The first tick starts the red frame's 30 ms timer without changing
//! # // the image. At 30 ms the blue frame shows for 40 ms, then at 70 ms
//! # // playback loops back to red, each time with a new generation.
//! # let [(t0, (p0, g0), d0), (t1, (p1, g1), d1), (t2, (p2, g2), d2)] = frames[..] else {
//! #     panic!("expected three frames, got {frames:?}");
//! # };
//! # assert_eq!((t0, p0, d0), (0, [255, 0, 0], Some(30)));
//! # assert_eq!((t1, p1, d1), (30, [0, 0, 255], Some(40)));
//! # assert_eq!((t2, p2, d2), (70, [255, 0, 0], Some(30)));
//! # assert!(g0 != g1 && g1 != g2 && g0 != g2);
//!
//! // Stop playback (s=1). Once stopped, the tick asks for no redraws.
//! terminal.vt_write(b"\x1b_Ga=a,i=2,s=1,q=2\x1b\\");
//! let delay_ms = tick_animations(&mut terminal, now_ms)?;
//! if delay_ms.is_none() {
//!     println!("  t={now_ms} ms: stopped, no redraw needed");
//! }
//! # assert_eq!(delay_ms, None);
//! # // Stopping leaves the image on the frame it was showing.
//! # assert_eq!(image(&terminal), (p2, g2));
//! # Ok(())
//! # }
//! ```
//!
//! ## Unicode placeholders
//!
//! A program can place an image with text instead of at a cell. It creates a
//! virtual placement, which has no position of its own, and prints
//! placeholder characters (U+10EEEE) where the image should appear. The image
//! then moves with those characters, like text. Programs such as tmux use
//! this so that images scroll, move and disappear along with the text around
//! them.
//!
//! Only the cells know where these images are, so the storage can't tell
//! you. The render state finds the placeholder cells and lists each part of
//! the image like any other entry, so you draw them the same way as every
//! other image. See
//! [Kitty graphics placeholders](crate::RenderState#kitty-graphics-placeholders)
//! in the render state documentation.
//!
//! ## Inspecting the storage
//!
//! A [`PlacementIterator`] lists every placement in the storage, including
//! placements that are off screen and virtual placements. Use it for tools
//! such as debuggers or tests that need to see what the storage holds. A
//! renderer doesn't need it.
//!
//!   1. Create an iterator with [`PlacementIterator::new`].
//!   2. Fill it from the storage with [`PlacementIterator::update`], which
//!      returns a [`PlacementIteration`].
//!   3. Optionally limit it to one layer with
//!      [`PlacementIteration::set_layer`].
//!   4. Move to each placement with [`PlacementIteration::next`] and read its
//!      values with the methods on [`PlacementIteration`], such as
//!      [`PlacementIteration::image_id`].
//!
//! These functions work out the geometry of the iterator's current placement
//! from the storage:
//!
//!   - [`PlacementIteration::pixel_size`]: the size to draw the image at, in
//!     pixels.
//!   - [`PlacementIteration::grid_size`]: the number of columns and rows of
//!     cells it covers.
//!   - [`PlacementIteration::viewport_pos`]: the top-left cell, relative to
//!     the viewport.
//!   - [`PlacementIteration::source_rect`]: the part of the image to draw, in
//!     image pixels.
//!   - [`PlacementIteration::rect`]: the cells it covers, as a [`Selection`].
//!   - [`PlacementIteration::placement_render_info`]: all of the above except
//!     the selection, in one call.
//!
//! They only know about placements at a cell, and placements relative to
//! one. They can't find virtual placements, or placements relative to a
//! virtual placement, because those depend on the cells. The render state
//! gives the same values for every entry in its list, so prefer it whenever
//! you are drawing.
//!
//! ## Lifetimes and thread-safety
//!
//! All handles borrowed from the terminal ([`Graphics`], [`Image`]) become
//! invalid after any call that changes the terminal. The placement iterator
//! belongs to you and is freed when dropped, but the values it returns are
//! only valid until the terminal changes.
//!
//! ## Example
//!
//! The following example creates a terminal, places one image at a cell and
//! another with placeholder cells, lists both from the render state, and reads
//! each image's pixels from the storage:
//!
//! ```
//! use libghostty_vt::{
//!     RenderState, Terminal,
//!     alloc::{Allocator, Bytes},
//!     kitty::graphics,
//!     render::{ImageIterator, ImageProtocol},
//! };
//! # use libghostty_vt::render::{ImageLayer, KittyPlacementKind};
//!
//! /// Minimal PNG decoder.
//! ///
//! /// A real implementation would use a PNG library (libpng, stb_image, etc.)
//! /// to decode the PNG data. This example uses a hardcoded 1x1 red pixel
//! /// since we know exactly what image we're sending.
//! ///
//! /// WARNING: This is only an example for providing a callback, it DOES NOT
//! /// actually decode the PNG it is passed. It hardcodes a response.
//! struct StubPngDecoder;
//!
//! impl graphics::DecodePng for StubPngDecoder {
//!     fn decode_png<'alloc, 'ctx>(
//!         &mut self,
//!         alloc: &'alloc Allocator<'ctx>,
//!         _data: &[u8],
//!     ) -> Option<graphics::DecodedImage<'alloc>> {
//!         // Allocate RGBA pixel data through the provided allocator.
//!         let mut data = Bytes::new_with_alloc(alloc, 4).ok()?;
//!
//!         // Fill with red (R=255, G=0, B=0, A=255).
//!         data.copy_from_slice(&[255, 0, 0, 255]);
//!
//!         Some(graphics::DecodedImage {
//!             width: 1,
//!             height: 1,
//!             data,
//!         })
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // SAFETY: No other thread uses libghostty yet.
//!     unsafe { graphics::set_png_decoder(Some(Box::new(StubPngDecoder))) }?;
//!
//!     // Create a terminal with Kitty graphics enabled.
//!     let mut terminal = Terminal::new(80, 24)?;
//!
//!     // Set the cell size in pixels. Image sizes depend on it.
//!     terminal.resize(80, 24, 8, 16)?;
//!
//!     // Set a storage limit (64 MiB) to enable Kitty graphics.
//!     terminal.set_kitty_image_storage_limit(64 * 1024 * 1024)?;
//!
//!     // Install pty_write to see the protocol response.
//!     terminal.on_pty_write(|_, data| println!("  response: {}", data.escape_ascii()))?;
//!
//!     // Move the cursor to row 10 so the images have room to scroll up.
//!     terminal.vt_write(b"\x1b[10;1H");
//!
//!     // Send a 1x1 PNG image and place it at the cursor. The image is
//!     // attached to that cell, so it moves when the text moves.
//!     //
//!     // The escape sequence is:
//!     //   ESC _G a=T,f=100,q=1; <base64 PNG data> ESC \
//!     //
//!     // Where:
//!     //   a=T    transmit the image and place it
//!     //   f=100  the data is a PNG
//!     //   q=1    only reply on errors (q=0 would also reply on success)
//!     println!("Sending Kitty graphics PNG image:");
//!     terminal.vt_write(
//!         b"\x1b_Ga=T,f=100,q=1;\
//!           iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA\
//!           DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\
//!           \x1b\\",
//!     );
//!
//!     // Show a second image with placeholder cells. First send a green 1x1
//!     // image with ID 3 and give it a virtual placement 2 columns wide and
//!     // 1 row tall (U=1). Then, on the next row, print two placeholder
//!     // characters (U+10EEEE) where the image should appear:
//!     //
//!     //   ESC[38;5;3m    the foreground color says which image: ID 3
//!     //   U+0305 U+0305  the first cell shows row 0, column 0 of the image
//!     //   U+0305 U+030D  the second cell shows row 0, column 1
//!     //
//!     // The image moves with those cells, like text.
//!     terminal.vt_write(
//!         "\x1b_Ga=T,U=1,f=32,i=3,s=1,v=1,c=2,r=1,q=2;AP8A/w==\x1b\\\
//!          \r\n\x1b[38;5;3m\
//!          \u{10EEEE}\u{0305}\u{0305}\
//!          \u{10EEEE}\u{0305}\u{030D}\
//!          \x1b[39m"
//!             .as_bytes(),
//!     );
//!
//!     // The storage has a generation of its own. It changes when a program
//!     // sends, places or deletes an image, but not when images only move on
//!     // screen. That is why the frames below use the render state's
//!     // placements generation to know when to read the list again.
//!     let storage_generation = terminal.kitty_graphics()?.generation()?;
//!     println!("Storage generation: {storage_generation}");
//! #   assert_ne!(storage_generation, 0);
//!
//!     // Find what to draw from the render state, over three frames.
//!     //
//!     // Each frame updates the render state and compares the placements
//!     // generation with the one from the last time the list was read. The
//!     // first frame always reads the list. The second frame changes nothing,
//!     // so it keeps the list it has. Before the third frame, new output
//!     // scrolls the screen by two rows, which moves both images, so the third
//!     // frame reads the list again.
//!
//!     // Create the render state and the image iterator once, and reuse them.
//!     let mut render_state = RenderState::new()?;
//!     let mut images = ImageIterator::new()?;
//!
//!     // Starting at `None` makes sure the first frame always reads the list.
//!     let mut last_generation: Option<u64> = None;
//! #   let mut lists_read = 0;
//!
//!     for frame in 1..=3 {
//!         if frame == 3 {
//!             // Move to the bottom row and print two newlines to scroll.
//!             terminal.vt_write(b"\x1b[24;1H\r\n\r\n");
//!         }
//!
//!         let snapshot = render_state.update(&terminal)?;
//!         let generation = snapshot.image_placements_generation()?;
//!         if last_generation == Some(generation) {
//!             println!("Frame {frame}: no image changed, keep drawing the last list");
//!             continue;
//!         }
//!         last_generation = Some(generation);
//! #       lists_read += 1;
//!
//!         // Read the render state's image list, the way a renderer builds its
//!         // list of images to draw.
//!         //
//!         // The list holds every Kitty image on screen, however the program
//!         // placed it, with its position, size and draw order worked out. The
//!         // Kitty graphics storage is only used to read pixels.
//!         //
//!         // A real renderer would find or upload a texture for each image and
//!         // queue it for drawing, sorted by layer and then z. This example
//!         // prints what it would draw instead.
//!         println!("Frame {frame}: images changed, reading the list:");
//!         let mut placements = images.update(&snapshot)?;
//!
//!         // The storage is only needed to read pixels.
//!         let graphics = terminal.kitty_graphics()?;
//!
//!         let mut count = 0;
//!         while let Some(placement) = placements.next() {
//!             // Skip protocols this renderer doesn't support.
//!             let Ok(ImageProtocol::Kitty) = placement.protocol() else {
//!                 continue;
//!             };
//!
//!             count += 1;
//!             let image_id = placement.image_id()?;
//!             let generation = placement.image_generation()?;
//!             println!(
//!                 "  image #{count}: kind={:?} image_id={image_id} placement_id={:?}",
//!                 placement.kitty_kind()?,
//!                 placement.kitty_placement_id()?,
//!             );
//!             println!(
//!                 "    layer={:?} z={} cell=({},{}) cells={}x{} offset=({},{})",
//!                 placement.layer()?,
//!                 placement.z()?,
//!                 placement.viewport_col()?,
//!                 placement.viewport_row()?,
//!                 placement.grid_cols()?,
//!                 placement.grid_rows()?,
//!                 placement.cell_offset_x()?,
//!                 placement.cell_offset_y()?,
//!             );
//!             println!(
//!                 "    size={}x{} source=({},{} {}x{})",
//!                 placement.pixel_width()?,
//!                 placement.pixel_height()?,
//!                 placement.source_x()?,
//!                 placement.source_y()?,
//!                 placement.source_width()?,
//!                 placement.source_height()?,
//!             );
//!
//!             // Read the pixels to upload them as a texture. A real renderer
//!             // keys its textures by protocol, image ID and image generation,
//!             // and only gets here when it has no texture for that key yet.
//!             // The generation changes whenever the pixels do, so most frames
//!             // skip this.
//!             let Some(image) = graphics.image(image_id) else {
//!                 continue;
//!             };
//!             let pixels = image.data()?.unwrap_or_default();
//!             println!(
//!                 "    pixels: {}x{} format={:?} data_len={} generation={generation}",
//!                 image.width()?,
//!                 image.height()?,
//!                 image.format()?,
//!                 pixels.len(),
//!             );
//! #           // Both images are 1x1 RGBA, and the render state reports the
//! #           // storage's image generation.
//! #           assert_eq!((image.width()?, image.height()?), (1, 1));
//! #           assert_eq!(image.format()?, graphics::ImageFormat::Rgba);
//! #           assert_eq!(image.generation()?, generation);
//! #           // Neither command gave a placement ID, and nothing is cropped.
//! #           assert_eq!(placement.kitty_placement_id()?, Some(0));
//! #           assert_eq!(placement.source_x()?, 0);
//! #           assert_eq!(placement.source_y()?, 0);
//! #           assert_eq!(placement.source_width()?, 1);
//! #           assert_eq!(placement.source_height()?, 1);
//! #           // The scroll before frame 3 moves both images up two rows.
//! #           let scrolled = if frame == 3 { 2 } else { 0 };
//! #           let at = (placement.viewport_col()?, placement.viewport_row()?);
//! #           let cells = (placement.grid_cols()?, placement.grid_rows()?);
//! #           let offset = (placement.cell_offset_x()?, placement.cell_offset_y()?);
//! #           let size = (placement.pixel_width()?, placement.pixel_height()?);
//! #           if image_id == 3 {
//! #               // The green image, shown by the two placeholder cells on
//! #               // row 11 (10 from 0). Virtual placements have z -1, which is
//! #               // below the text. The 1x1 image is scaled to fit its 2x1
//! #               // cells (16x16 pixels), which it fills exactly.
//! #               assert_eq!(placement.kitty_kind()?, Some(KittyPlacementKind::Virtual));
//! #               assert_eq!(placement.z()?, -1);
//! #               assert_eq!(placement.layer()?, ImageLayer::BelowText);
//! #               assert_eq!(at, (0, 10 - scrolled));
//! #               assert_eq!(cells, (2, 1));
//! #               assert_eq!(offset, (0, 0));
//! #               assert_eq!(size, (16, 16));
//! #               assert_eq!(pixels, [0, 255, 0, 255]);
//! #           } else {
//! #               // The red PNG at the cursor on row 10 (9 from 0), drawn at
//! #               // its own size of 1x1 pixels in one cell. z defaults to 0,
//! #               // which is above the text.
//! #               assert_eq!(placement.kitty_kind()?, Some(KittyPlacementKind::Fixed));
//! #               assert_eq!(placement.z()?, 0);
//! #               assert_eq!(placement.layer()?, ImageLayer::AboveText);
//! #               assert_eq!(at, (0, 9 - scrolled));
//! #               assert_eq!(cells, (1, 1));
//! #               assert_eq!(offset, (0, 0));
//! #               assert_eq!(size, (1, 1));
//! #               assert_eq!(pixels, [255, 0, 0, 255]);
//! #           }
//!         }
//!         println!("Total images: {count}");
//! #       assert_eq!(count, 2);
//!     }
//! #   // Frame 2 kept the list from frame 1.
//! #   assert_eq!(lists_read, 2);
//!     Ok(())
//! }
//! ```
//!

#![cfg(feature = "kitty-graphics")]

use std::{
    ffi::OsStr,
    mem::{ManuallyDrop, MaybeUninit},
    path::Path,
    sync::{Mutex, PoisonError},
};

use crate::{
    Terminal,
    alloc::{Allocator, Bytes, Object, Ref},
    error::{Error, Result, from_optional_result_uninit, from_result},
    ffi,
    selection::Selection,
};

#[doc(inline)]
pub use ffi::KittyGraphicsPlacementRenderInfo as PlacementRenderInfo;

/// Opaque reference to a Kitty graphics image storage.
///
/// Obtained via [`Terminal::kitty_graphics`]. The reference is borrowed from
/// the terminal with lifetime `'t` and remains valid until the next mutating
/// terminal call (e.g. [`Terminal::vt_write`] or [`Terminal::reset`]).
#[derive(Debug)]
pub struct Graphics<'t> {
    inner: Ref<'t, ffi::KittyGraphicsImpl>,
}

/// Opaque reference to a Kitty graphics image.
///
/// Obtained via [`Graphics::image`] with an image ID. The reference is
/// borrowed from the storage with lifetime `'t` and remains valid until
/// the next mutating terminal call.
#[derive(Debug)]
pub struct Image<'t> {
    inner: Ref<'t, ffi::KittyGraphicsImageImpl>,
}

/// Opaque reference to a Kitty graphics placement iterator.
///
/// It lists every placement in the storage, including virtual placements and
/// placements that are off screen. Use it to inspect the storage, not to draw.
/// See [Inspecting the storage](crate::kitty::graphics#inspecting-the-storage).
///
/// # Example
///
/// ```
/// use libghostty_vt::{
///     Terminal,
///     kitty::graphics::{GridSize, PixelSize, PlacementIterator, ViewportPos},
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut terminal = Terminal::new(80, 24)?;
/// # // Cells are 8x16 pixels. Image sizes depend on it.
/// # terminal.resize(80, 24, 8, 16)?;
/// # terminal.set_kitty_image_storage_limit(64 * 1024 * 1024)?;
/// // Send a 20x24 RGBA image with ID 1 (all zero bytes, which base64-encode
/// // to "A"s) and place it at row 10 (9 from 0) with placement ID 5 and
/// // z-index 2.
/// let (width, height) = (20, 24);
/// let pixels = "A".repeat(width * height * 4 / 3 * 4);
/// terminal.vt_write(b"\x1b[10;1H");
/// terminal.vt_write(
///     format!("\x1b_Ga=T,f=32,s={width},v={height},i=1,p=5,z=2,q=2;{pixels}\x1b\\").as_bytes(),
/// );
///
/// // Send a 1x1 image with ID 3 and give it a virtual placement, which a
/// // program shows with Unicode placeholder cells. The storage lists it even
/// // though it has no position of its own.
/// terminal.vt_write(b"\x1b_Ga=T,U=1,f=32,i=3,s=1,v=1,c=2,r=1,q=2;AP8A/w==\x1b\\");
///
/// let graphics = terminal.kitty_graphics()?;
/// let mut iterator = PlacementIterator::new()?;
/// let mut placements = iterator.update(&graphics)?;
///
/// let mut count = 0;
/// while let Some(placement) = placements.next() {
///     count += 1;
///     let image_id = placement.image_id()?;
///     let image = graphics.image(image_id).ok_or("placement without image")?;
///     let viewport_pos = placement.viewport_pos(&image, &terminal)?;
///
///     if placement.is_virtual()? {
///         // Only the placeholder cells know where a virtual placement is.
///         assert_eq!(image_id, 3);
///         assert_eq!(viewport_pos, None);
///     } else {
///         // Without columns and rows, the image is drawn at its own size and
///         // covers as many cells as that takes: 20/8 rounds up to 3 columns,
///         // 24/16 rounds up to 2 rows.
///         assert_eq!(image_id, 1);
///         assert_eq!(placement.placement_id()?, 5);
///         assert_eq!(placement.z()?, 2);
///         assert_eq!(viewport_pos, Some(ViewportPos { col: 0, row: 9 }));
///         assert_eq!(
///             placement.pixel_size(&image, &terminal)?,
///             PixelSize { width: 20, height: 24 },
///         );
///         assert_eq!(
///             placement.grid_size(&image, &terminal)?,
///             GridSize { cols: 3, rows: 2 },
///         );
///     }
/// }
/// assert_eq!(count, 2);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct PlacementIterator<'alloc> {
    inner: Object<'alloc, ffi::KittyGraphicsPlacementIteratorImpl>,
}

/// Obtained via [`PlacementIterator::update`]. The reference is
/// borrowed from the storage with lifetime `'t` and remains valid until
/// the next mutating terminal call.
#[derive(Debug)]
pub struct PlacementIteration<'t, 'alloc>(&'t mut PlacementIterator<'alloc>);

/// Methods related to the [Kitty graphics protocol](crate::kitty::graphics).
impl Terminal<'_, '_> {
    /// The Kitty graphics image storage for the active screen.
    ///
    /// Returns a borrowed reference to the image storage.
    /// The pointer is valid until the next mutating terminal call (e.g.
    /// [`Terminal::vt_write`] or [`Terminal::reset`]).
    pub fn kitty_graphics(&self) -> Result<Graphics<'_>> {
        let inner = self.get::<ffi::KittyGraphics>(ffi::TerminalData::KITTY_GRAPHICS)?;
        Ok(Graphics {
            inner: Ref::new(inner)?,
        })
    }

    /// Advance playing animations and get the time of the next frame.
    ///
    /// Every playing animation on the active screen moves to the frame that is
    /// due at `now_ms`.
    /// An animation moves forward at most one frame per call. If calls arrive
    /// late, playback slows down rather than skipping frames.
    ///
    /// Images that moved to a new frame get a new
    /// [generation](Image::generation), and their [`Image::data`] returns the
    /// new frame's pixels.
    ///
    /// Time is measured in milliseconds on a clock you choose, such as the
    /// time since your application started. The clock must only move forward,
    /// and you must keep using the same clock with the same starting point for
    /// as long as the terminal exists. The returned time uses the same clock.
    ///
    /// This call changes the image storage. Like [`Terminal::vt_write`], it
    /// invalidates any [`Image`] handles and image data you are holding, so
    /// look images up again afterwards. Because this method takes
    /// `&mut self`, any [`Graphics`], [`Image`] or pixel slice must be dropped
    /// first and looked up again afterwards with [`Terminal::kitty_graphics`],
    /// [`Graphics::image`] and [`Image::data`]:
    ///
    /// ```compile_fail,E0502
    /// # use libghostty_vt::Terminal;
    /// let mut terminal = Terminal::new(80, 24).unwrap();
    /// let graphics = terminal.kitty_graphics().unwrap();
    /// let image = graphics.image(1).unwrap();
    /// let pixels = image.data().unwrap().unwrap();
    /// // The tick invalidates `pixels`, so it can't run while they're held.
    /// terminal.kitty_graphics_animation_tick(0).unwrap();
    /// println!("{}", pixels[0]);
    /// ```
    ///
    /// Call this once each time you prepare a frame to draw, not in a loop.
    /// The [Animation](crate::kitty::graphics#animation) section explains how
    /// to schedule redraws with the returned time and includes an example.
    ///
    /// Returns `Some` with the time the next frame is due, in milliseconds on
    /// the same clock as `now_ms`, if an animation is playing. Returns `None`
    /// if nothing is playing or Kitty graphics are disabled at build time.
    //
    // This lives on `Terminal` rather than `Graphics` on purpose: upstream
    // takes the storage handle, but `Graphics` and the `Image`s and pixel
    // slices it hands out all borrow the terminal immutably, and any number of
    // them can exist at once. Taking `&mut self` here is what lets the borrow
    // checker guarantee none of them survive the tick that invalidates them.
    pub fn kitty_graphics_animation_tick(&mut self, now_ms: u64) -> Result<Option<u64>> {
        // Not `Terminal::kitty_graphics`: when Kitty graphics are disabled in
        // the Zig build (which our Cargo features don't control), upstream
        // reports the handle as `NO_VALUE`, and that getter would turn it into
        // `Err(Error::InvalidValue)`. Here it just means nothing is playing.
        let Some(graphics) =
            self.get_optional::<ffi::KittyGraphics>(ffi::TerminalData::KITTY_GRAPHICS)?
        else {
            return Ok(None);
        };
        let mut next_ms = MaybeUninit::<u64>::zeroed();
        // SAFETY: `graphics` was just obtained from this terminal and nothing
        // else can be borrowing it, as we hold `&mut self`. `next_ms` is a
        // valid, non-NULL out-pointer, which is only written on success.
        let result = unsafe {
            ffi::ghostty_kitty_graphics_animation_tick(graphics, now_ms, next_ms.as_mut_ptr())
        };
        from_optional_result_uninit(result, next_ms)
    }

    /// The Kitty image storage limit in bytes for the active screen.
    ///
    /// A value of zero means the Kitty graphics protocol is disabled.
    pub fn kitty_image_storage_limit(&self) -> Result<u64> {
        self.get(ffi::TerminalData::KITTY_IMAGE_STORAGE_LIMIT)
    }
    /// Whether the file medium is enabled for Kitty image loading on the
    /// active screen.
    pub fn is_kitty_image_from_file_allowed(&self) -> Result<bool> {
        self.get(ffi::TerminalData::KITTY_IMAGE_MEDIUM_FILE)
    }
    /// The directory allowed for Kitty image loading via the temporary file
    /// medium on the active screen, or `None` when the medium is disabled.
    ///
    /// Returns a borrowed path, valid until the next mutating terminal call
    /// (e.g. [`Terminal::vt_write`] or [`Terminal::reset`]).
    pub fn kitty_image_temp_file_dir(&self) -> Result<Option<&Path>> {
        let str = self.get::<ffi::String>(ffi::TerminalData::KITTY_IMAGE_MEDIUM_TEMP_FILE)?;
        if str.len == 0 {
            return Ok(None);
        }
        // SAFETY: We trust libghostty to return a valid borrowed string,
        // while we uphold that no mutation could happen during its lifetime.
        let bytes = unsafe { std::slice::from_raw_parts(str.ptr, str.len) };
        // SAFETY:
        // Whilst the `OsStr::from_encoded_bytes_unchecked` function states that the
        // exact OsStr encoding format is an implementation detail and no external bytes
        // should rely on which exact encoding is used... In reality this should always
        // work without issue, since both Zig and Rust basically share the same underlying
        // string format.
        //
        // On Unix-like platforms, both Zig and Rust agree that a file path is just an
        // arbitrary sequence of bytes without any NULs in between. The tricky part is
        // that Windows uses (noncompliant) UTF-16 file paths natively and it just so
        // happens both Zig and Rust use WTF-8 as a workaround against that whole can
        // of worms, though in Rust it's more of an implementation detail.
        // The Kitty graphics protocol itself is poorly defined on Windows, but for all
        // intents and purposes, the bytes are always valid `OsStr`s and `Path`s.
        let os_str = unsafe { OsStr::from_encoded_bytes_unchecked(bytes) };
        Ok(Some(Path::new(os_str)))
    }
    /// Whether the shared memory medium is enabled for Kitty image loading
    /// on the active screen.
    pub fn is_kitty_image_from_shared_mem_allowed(&self) -> Result<bool> {
        self.get(ffi::TerminalData::KITTY_IMAGE_MEDIUM_SHARED_MEM)
    }
    /// Set the Kitty image storage limit in bytes.
    ///
    /// Applied to all initialized screens (primary and alternate).
    /// A value of zero disables the Kitty graphics protocol entirely,
    /// deleting all stored images and placements.
    pub fn set_kitty_image_storage_limit(&mut self, limit: u64) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_STORAGE_LIMIT, &limit)?;
        Ok(self)
    }
    /// Enable or disable Kitty image loading via the file medium.
    ///
    /// Has no effect when Kitty graphics are disabled at build time.
    pub fn set_kitty_image_from_file_allowed(&mut self, allowed: bool) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_MEDIUM_FILE, &allowed)?;
        Ok(self)
    }
    /// Enable Kitty image loading via the temporary file medium, restricted
    /// to the given directory. A `None` value disables the medium. The
    /// string is copied into the terminal.
    ///
    /// Has no effect when Kitty graphics are disabled at build time.
    pub fn set_kitty_image_temp_file_dir(&mut self, dir: Option<&Path>) -> Result<&mut Self> {
        // GhosttyString carries UTF-8, so reject paths that
        // [`Terminal::kitty_image_temp_file_dir`] could never read back.
        let dir = dir
            .map(|dir| dir.to_str().ok_or(Error::InvalidValue))
            .transpose()?
            .map(ffi::String::from);
        self.set_optional(
            ffi::TerminalOption::KITTY_IMAGE_MEDIUM_TEMP_FILE,
            dir.as_ref(),
        )?;
        Ok(self)
    }
    /// Enable or disable Kitty image loading via the shared memory medium.
    ///
    /// Has no effect when Kitty graphics are disabled at build time.
    pub fn set_kitty_image_from_shared_mem_allowed(&mut self, allowed: bool) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_MEDIUM_SHARED_MEM, &allowed)?;
        Ok(self)
    }

    /// Set the maximum bytes the APC handler will buffer for Kitty graphics
    /// protocol data.
    ///
    /// This prevents malicious input from causing unbounded memory allocation.
    /// A `None` value removes all overrides, reverting to the built-in defaults.
    pub fn set_apc_max_bytes_kitty(&mut self, max: Option<usize>) -> Result<&mut Self> {
        self.set_optional(ffi::TerminalOption::APC_MAX_BYTES_KITTY, max.as_ref())?;
        Ok(self)
    }
}

impl<'t> Graphics<'t> {
    fn get<T>(&self, tag: ffi::KittyGraphicsData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    /// Look up a Kitty graphics image by its image ID.
    ///
    /// Returns `None` if no image with the given ID exists.
    #[must_use]
    pub fn image(&self, id: u32) -> Option<Image<'t>> {
        let image = unsafe { ffi::ghostty_kitty_graphics_image(self.inner.as_raw(), id) };

        Some(Image {
            inner: Ref::new(image.cast_mut()).ok()?,
        })
    }

    /// A value that changes whenever a program sends, replaces, places or
    /// deletes an image.
    ///
    /// It is 0 if nothing has changed the storage yet, which means the storage
    /// is empty.
    ///
    /// If the value matches the last one you read, the storage holds the same
    /// images and placements as before. The placements may still have moved on
    /// screen, because scrolling and resizing move them without changing the
    /// storage. So this value can't tell you when to read the render state's
    /// image list again. For that, use
    /// [`Snapshot::image_placements_generation`](crate::render::Snapshot::image_placements_generation).
    /// See [Knowing what changed](crate::kitty::graphics#knowing-what-changed).
    ///
    /// A value is never reused, not even by another screen, after a reset, or
    /// by another terminal. You can use it as a cache key on its own.
    pub fn generation(&self) -> Result<u64> {
        self.get(ffi::KittyGraphicsData::GENERATION)
    }
}

impl<'t> Image<'t> {
    fn get<T>(&self, tag: ffi::KittyGraphicsImageData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_image_get(
                self.inner.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        // Since we manually model every possible query, this should never fail.
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    /// The image ID.
    pub fn id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::ID)
    }
    /// The image number.
    pub fn number(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::NUMBER)
    }
    /// Image width in pixels.
    pub fn width(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::WIDTH)
    }
    /// Image height in pixels.
    pub fn height(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::HEIGHT)
    }

    /// A value that changes whenever this image's pixels may have changed:
    /// when a program sends the image or replaces it, and when an
    /// [animation](crate::kitty::graphics#animation) moves to a new frame.
    ///
    /// Use it as part of your texture cache key. If it matches the texture you
    /// have, you don't need to read the pixels again. The render state reports
    /// the same value for each of its entries as
    /// [`ImageIteration::image_generation`](crate::render::ImageIteration::image_generation).
    ///
    /// Don't use the image's size or data length to detect changes instead. A
    /// program can send new pixels of the same size under the same ID.
    ///
    /// A value is never reused, not even by another image, another screen, or
    /// another terminal. It is never 0 for a stored image, so you can use 0 to
    /// mean "no texture yet".
    ///
    /// One case doesn't change this value. When a program finishes sending an
    /// image whose pixels were still on the way, the image keeps its value,
    /// and [`Image::data`] starts returning the pixels.
    /// [`Graphics::generation`] changes when that happens, so read
    /// [`Image::data`] again then.
    pub fn generation(&self) -> Result<u64> {
        self.get(ffi::KittyGraphicsImageData::GENERATION)
    }
    /// Pixel format of the image.
    pub fn format(&self) -> Result<ImageFormat> {
        self.get::<ffi::KittyImageFormat::Type>(ffi::KittyGraphicsImageData::FORMAT)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    /// Compression of the image.
    pub fn compression(&self) -> Result<Compression> {
        self.get::<ffi::KittyImageCompression::Type>(ffi::KittyGraphicsImageData::COMPRESSION)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    /// Borrowed pointer to the raw pixel data.
    ///
    /// Valid as long as the underlying terminal is not mutated.
    ///
    /// Returns `None` when the image metadata is resident but its pixel
    /// payload is pending.
    ///
    /// The data is always fully decoded, uncompressed pixels in the format
    /// reported by [`Image::format`]: zlib payloads are inflated and PNG
    /// payloads are decoded to RGBA at transmission time, before the image
    /// is stored. Consumers can upload this directly to the GPU without any
    /// decode step.
    ///
    /// For an animated image (Kitty graphics animation, actions a=f/a=a) this
    /// is the pixel data of the current animation frame. The image's
    /// [generation](Image::generation) changes whenever the current frame
    /// changes, so generation-keyed caches remain coherent.
    pub fn data(&self) -> Result<Option<&'t [u8]>> {
        let ptr = self.get::<*const u8>(ffi::KittyGraphicsImageData::DATA_PTR)?;
        if ptr.is_null() {
            return Ok(None);
        }

        let len = self.get::<usize>(ffi::KittyGraphicsImageData::DATA_LEN)?;

        // SAFETY: We trust libghostty to return valid results
        Ok(Some(unsafe { std::slice::from_raw_parts(ptr, len) }))
    }
}

impl<'alloc> PlacementIterator<'alloc> {
    /// Create a new placement iterator instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }
    /// Create a new placement iterator instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }
    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut inner: ffi::KittyGraphicsPlacementIterator = std::ptr::null_mut();
        let result =
            unsafe { ffi::ghostty_kitty_graphics_placement_iterator_new(alloc, &raw mut inner) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(inner)?,
        })
    }

    /// Update the placement iterator with the given graphics storage,
    /// returning a new placement iteration.
    ///
    /// The iteration walks the storage itself, so it borrows `graphics` (and
    /// through it the terminal) as well as this iterator:
    ///
    /// ```compile_fail,E0505
    /// # use libghostty_vt::{Terminal, kitty::graphics::PlacementIterator};
    /// let terminal = Terminal::new(80, 24).unwrap();
    /// let mut iterator = PlacementIterator::new().unwrap();
    /// let graphics = terminal.kitty_graphics().unwrap();
    /// let mut placements = iterator.update(&graphics).unwrap();
    /// drop(terminal);
    /// placements.next();
    /// ```
    pub fn update<'t>(
        &'t mut self,
        graphics: &'t Graphics<'_>,
    ) -> Result<PlacementIteration<'t, 'alloc>> {
        let result = unsafe {
            ffi::ghostty_kitty_graphics_get(
                graphics.inner.as_raw(),
                ffi::KittyGraphicsData::PLACEMENT_ITERATOR,
                (&raw mut self.inner).cast(),
            )
        };
        from_result(result)?;
        Ok(PlacementIteration(self))
    }
}

impl Drop for PlacementIterator<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::ghostty_kitty_graphics_placement_iterator_free(self.inner.as_raw());
        }
    }
}

impl<'t> PlacementIteration<'t, '_> {
    /// Advance the placement iterator to the next placement.
    ///
    /// If a layer filter has been set via [`PlacementIteration::set_layer`],
    /// only placements matching that layer are returned.
    #[expect(
        clippy::should_implement_trait,
        reason = "a lending iterator: each placement borrows the iteration"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_kitty_graphics_placement_next(self.0.inner.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn set<T>(
        &self,
        tag: ffi::KittyGraphicsPlacementIteratorOption::Type,
        value: &T,
    ) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_iterator_set(
                self.0.inner.as_raw(),
                tag,
                std::ptr::from_ref(value).cast(),
            )
        };
        from_result(result)
    }
    fn get<T>(&self, tag: ffi::KittyGraphicsPlacementData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_get(
                self.0.inner.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        // Since we manually model every possible query, this should never fail.
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }

    /// Only return placements in one layer. See [`Layer`].
    pub fn set_layer(&self, layer: Layer) -> Result<()> {
        self.set::<ffi::KittyPlacementLayer::Type>(
            ffi::KittyGraphicsPlacementIteratorOption::LAYER,
            &layer.into(),
        )
    }

    /// Compute the rendered pixel size of the current placement.
    ///
    /// Takes into account the placement's source rectangle, specified
    /// columns/rows, and aspect ratio to calculate the final rendered pixel
    /// dimensions.
    pub fn pixel_size(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<PixelSize> {
        let mut size = PixelSize::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_pixel_size(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut size.width,
                &raw mut size.height,
            )
        };
        from_result(result)?;
        Ok(size)
    }

    /// Compute the rendered pixel size of the current placement.
    ///
    /// Takes into account the placement's source rectangle, specified
    /// columns/rows, and aspect ratio to calculate the final rendered pixel
    /// dimensions.
    pub fn grid_size(&self, image: &Image<'t>, terminal: &'t Terminal<'_, '_>) -> Result<GridSize> {
        let mut size = GridSize::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_grid_size(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut size.cols,
                &raw mut size.rows,
            )
        };
        from_result(result)?;
        Ok(size)
    }

    /// Get the top-left cell of the current placement, relative to the
    /// viewport.
    ///
    /// This is for inspecting the storage. To draw, use the render state's
    /// image list ([`ImageIterator`](crate::render::ImageIterator)) instead.
    /// It positions every image on screen, including virtual placements and
    /// placements relative to them, and covers overscan rows. If you draw
    /// from both, images are drawn twice. See
    /// [Drawing images](crate::kitty::graphics#drawing-images).
    ///
    /// The row is negative when the top of the placement scrolled above the
    /// viewport. For example, an image 4 rows tall that scrolled up by 2 rows
    /// has a row of -2. Its top 2 rows are above the viewport and its bottom
    /// 2 rows are still visible.
    ///
    /// Returns `None` if it is entirely outside the viewport, if it is a
    /// virtual placement or relative to one, or if Kitty graphics are
    /// disabled at build time.
    pub fn viewport_pos(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<Option<ViewportPos>> {
        let mut pos = ViewportPos::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_viewport_pos(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut pos.col,
                &raw mut pos.row,
            )
        };
        from_optional_result_uninit(result, MaybeUninit::new(pos))
    }

    /// Get the resolved source rectangle for the current placement.
    ///
    /// Applies kitty protocol semantics: a width or height of 0 in the
    /// placement means "use the full image dimension", and the resulting
    /// rectangle is clamped to the actual image bounds. The returned values
    /// are in pixels and are ready to use for texture sampling.
    pub fn source_rect(&self, image: &Image<'t>) -> Result<SourceRect> {
        let mut rect = SourceRect::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_source_rect(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                &raw mut rect.x,
                &raw mut rect.y,
                &raw mut rect.width,
                &raw mut rect.height,
            )
        };
        from_result(result)?;
        Ok(rect)
    }

    /// Get the resolved source rectangle for the current placement.
    ///
    /// Applies kitty protocol semantics: a width or height of 0 in the
    /// placement means "use the full image dimension", and the resulting
    /// rectangle is clamped to the actual image bounds. The returned values
    /// are in pixels and are ready to use for texture sampling.
    pub fn rect(&self, image: &Image<'t>, terminal: &'t Terminal<'_, '_>) -> Result<Selection<'t>> {
        let mut sel = MaybeUninit::<ffi::Selection>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_rect(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                sel.as_mut_ptr(),
            )
        };
        from_result(result)?;
        // SAFETY: Selection should be initialized and valid on success
        Ok(unsafe { Selection::from_raw(sel.assume_init()) })
    }

    /// Get all of the geometry of the current placement in one call.
    ///
    /// This returns a [`PlacementRenderInfo`] with the pixel size, grid size,
    /// viewport position and source rectangle.
    ///
    /// This is for inspecting the storage. To draw, use the render state's
    /// image list ([`ImageIterator`](crate::render::ImageIterator)) instead.
    /// It gives the same values for every image on screen, including virtual
    /// placements and placements relative to them. If you draw from both,
    /// images are drawn twice. See
    /// [Drawing images](crate::kitty::graphics#drawing-images).
    ///
    /// The call succeeds even when the placement isn't in the viewport.
    /// Check `viewport_visible` before using `viewport_col` and
    /// `viewport_row`.
    pub fn placement_render_info(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<PlacementRenderInfo> {
        let mut info = ffi::sized!(PlacementRenderInfo);
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_render_info(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut info,
            )
        };
        from_result(result)?;
        Ok(info)
    }

    /// The image ID this placement belongs to.
    pub fn image_id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::IMAGE_ID)
    }
    /// The image ID this placement belongs to.
    pub fn placement_id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::PLACEMENT_ID)
    }
    /// Whether this is a virtual placement, which a program shows with
    /// Unicode placeholder cells.
    ///
    /// A virtual placement appears wherever its placeholder cells are, so the
    /// storage has no position for it. The render state's image list
    /// positions it from its cells like any other placement, so a renderer
    /// doesn't need to check this. See
    /// [Unicode placeholders](crate::kitty::graphics#unicode-placeholders).
    pub fn is_virtual(&self) -> Result<bool> {
        self.get(ffi::KittyGraphicsPlacementData::IS_VIRTUAL)
    }
    /// Pixel offset from the left edge of the cell.
    pub fn x_offset(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::X_OFFSET)
    }
    /// Pixel offset from the top edge of the cell.
    pub fn y_offset(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::Y_OFFSET)
    }
    /// Source rectangle x origin in pixels.
    pub fn source_x(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_X)
    }
    /// Source rectangle y origin in pixels.
    pub fn source_y(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_Y)
    }
    /// Source rectangle width in pixels (0 = full image width).
    pub fn source_width(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_WIDTH)
    }
    /// Source rectangle height in pixels (0 = full image height).
    pub fn source_height(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_HEIGHT)
    }
    /// Number of columns this placement occupies.
    pub fn columns(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::COLUMNS)
    }
    /// Number of rows this placement occupies.
    pub fn rows(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::ROWS)
    }
    /// Z-index for this placement.
    pub fn z(&self) -> Result<i32> {
        self.get(ffi::KittyGraphicsPlacementData::Z)
    }
}

/// The size of an image in pixel coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelSize {
    /// The width in number of pixels.
    pub width: u32,
    /// The height in number of pixels.
    pub height: u32,
}

/// The size of an image in grid coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridSize {
    /// The number of columns.
    pub cols: u32,
    /// The number of rows.
    pub rows: u32,
}

/// The top-left cell of a storage placement, relative to the viewport.
///
/// Returned by [`PlacementIteration::viewport_pos`], which is for inspecting
/// the storage. To draw, use the render state's image list
/// ([`ImageIterator`](crate::render::ImageIterator)) instead.
///
/// The row is negative when the top of the placement scrolled above the
/// viewport. For example, an image 4 rows tall that scrolled up by 2 rows has
/// a row of -2. Its top 2 rows are above the viewport and its bottom 2 rows
/// are still visible.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportPos {
    /// The column index relative to the viewport.
    pub col: i32,
    /// The row index relative to the viewport.
    pub row: i32,
}

/// The pixel position and size of a source rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceRect {
    /// The x origin in pixels.
    pub x: u32,
    /// The y origin in pixels.
    pub y: u32,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

/// A layer to limit the storage placement iterator to, set with
/// [`PlacementIteration::set_layer`].
///
/// The Kitty graphics protocol picks a placement's layer from its z-index, as
/// each value below describes. This filter is only for the storage iterator.
/// To draw, use the layer the render state gives each entry
/// ([`ImageIteration::layer`](crate::render::ImageIteration::layer)). It
/// follows the same rules, and it also covers virtual placements, which are
/// always drawn below the text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum Layer {
    /// Every placement. This is the default.
    #[default]
    All = ffi::KittyPlacementLayer::ALL,
    /// Placements drawn below the cell backgrounds: z < [`i32::MIN`] / 2.
    BelowBg = ffi::KittyPlacementLayer::BELOW_BG,
    /// Placements drawn above the cell backgrounds and below the text:
    /// [`i32::MIN`] / 2 ≤ z < 0.
    BelowText = ffi::KittyPlacementLayer::BELOW_TEXT,
    /// Placements drawn above the text: z ≥ 0.
    AboveText = ffi::KittyPlacementLayer::ABOVE_TEXT,
}

/// Pixel format of a Kitty graphics image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
#[expect(missing_docs, reason = "missing upstream docs")]
pub enum ImageFormat {
    #[default]
    Rgb = ffi::KittyImageFormat::RGB,
    Rgba = ffi::KittyImageFormat::RGBA,
    Png = ffi::KittyImageFormat::PNG,
    GrayAlpha = ffi::KittyImageFormat::GRAY_ALPHA,
    Gray = ffi::KittyImageFormat::GRAY,
}

/// Compression of a Kitty graphics image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
#[expect(missing_docs, reason = "missing upstream docs")]
pub enum Compression {
    #[default]
    None = ffi::KittyImageCompression::NONE,
    ZlibDeflate = ffi::KittyImageCompression::ZLIB_DEFLATE,
}

// libghostty's decoder hook is process-global, so the decoder has to be too:
// a terminal on any thread may call it. Decoding takes `&mut self`, hence the
// mutex rather than the logger's read-write lock.
static DECODE_PNG: Mutex<Option<Box<dyn DecodePng>>> = Mutex::new(None);

/// Set the PNG decoder.
///
/// When set, the terminal can accept PNG images via the Kitty Graphics Protocol.
/// When cleared (`None` value), PNG decoding is unsupported and PNG image data
/// will be rejected.
///
/// This is a process-global setting. It is simplest to set it once at
/// startup, before using any terminal functionality that depends on it.
///
/// # Safety
///
/// libghostty does not synchronize this setting, so this must not be called
/// while any other thread may be inside a libghostty call.
pub unsafe fn set_png_decoder(f: Option<Box<dyn DecodePng>>) -> Result<()> {
    unsafe extern "C" fn callback(
        _userdata: *mut std::ffi::c_void,
        allocator: *const ffi::Allocator,
        data: *const u8,
        data_len: usize,
        out: *mut ffi::SysImage,
    ) -> bool {
        // A panic while decoding aborts the process (we are inside an
        // `extern "C"` callback), so the lock can't really be poisoned.
        let mut decoder = DECODE_PNG.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(decoder) = decoder.as_deref_mut() else {
            return false;
        };
        // SAFETY: We trust libghostty to return valid values.
        let alloc = unsafe { Allocator::from_raw(allocator) };
        let data = unsafe { std::slice::from_raw_parts(data, data_len) };

        match decoder.decode_png(&alloc, data) {
            // libghostty frees the buffer with the allocator it passed
            // in, so a buffer from any other allocator would be freed
            // with the wrong one. `DecodePng` is a safe trait and
            // `Bytes::new` type-checks here, so reject such an image
            // instead of trusting the decoder; dropping it frees the
            // buffer with its own allocator.
            Some(result) if result.data.allocator() != alloc.to_raw() => false,
            Some(result) => {
                // IMPORTANT: Do NOT run the Rust destructor here
                // to avoid double-freeing the byte buffer.
                let mut result = ManuallyDrop::new(result);
                unsafe {
                    *out = ffi::SysImage {
                        width: result.width,
                        height: result.height,
                        data: result.data.as_mut_ptr(),
                        data_len: result.data.len(),
                    }
                };
                true
            }
            None => false,
        }
    }

    // Write out the matches here to coerce function items into function
    // pointers, and trait impls into boxed trait objects. Yes, this is
    // the simplest way to do so.
    let ptr: ffi::SysDecodePngFn = match f {
        None => None,
        Some(_) => Some(callback),
    };
    // Keep the old decoder alive until function exit: its destructor may
    // process PNG input, so it must run after unlocking and updating the hook.
    let _old = std::mem::replace(
        &mut *DECODE_PNG.lock().unwrap_or_else(PoisonError::into_inner),
        f,
    );

    crate::sys_set(
        ffi::SysOption::DECODE_PNG,
        ptr.map_or(std::ptr::null(), |p| p as *const std::ffi::c_void),
    )
}

/// A PNG decoder that can be used by the Kitty graphics protocol
/// to decode PNG images into 8-bit RGBA pixels.
///
/// See [`set_png_decoder`] for more details.
pub trait DecodePng: Send + 'static {
    /// Decode a PNG into 8-bit RGBA pixels.
    ///
    /// The library calls this when it receives a PNG image and needs the raw
    /// pixels. The decoder decodes the PNG bytes in `data` and returns the
    /// result. See the [module-level example](self#example) for a complete
    /// callback.
    ///
    /// Decoding is serialized across all terminals. This method must not
    /// synchronously initiate or wait for another PNG decode, or call
    /// [`set_png_decoder`], as doing so would deadlock or abort the process.
    ///
    /// # On success
    ///
    /// Allocate the pixel buffer with [`Bytes::new_with_alloc`] and `alloc`,
    /// write the decoded pixels into it, and return it in a [`DecodedImage`].
    /// The library then owns the buffer and frees it with the same allocator.
    /// See [`DecodedImage`] for the expected pixel layout.
    ///
    /// The allocator limits how much memory a single image may use, so
    /// [`Bytes::new_with_alloc`] can fail for very large images. Treat that as
    /// a failure.
    ///
    /// # On failure
    ///
    /// Return `None`, and the image is rejected. An image whose buffer comes
    /// from any allocator other than `alloc` is also rejected, as if decoding
    /// failed.
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>>;
}

/// A PNG decoder for [`set_png_decoder`] using the [`png`] crate.
///
/// ```rust
/// use libghostty_vt::kitty::graphics;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // SAFETY: No other thread uses libghostty yet.
/// unsafe { graphics::set_png_decoder(Some(Box::new(graphics::RustPngDecoder::new()))) }?;
/// # Ok(())
/// # }
/// ```
#[cfg(all(feature = "kitty-graphics", feature = "png"))]
#[derive(Clone, Debug, Default)]
// Keep the decoder constructible only through `new` and `Default`, so that
// it can grow internal state later without breaking callers.
#[non_exhaustive]
pub struct RustPngDecoder;
#[cfg(all(feature = "kitty-graphics", feature = "png"))]
impl RustPngDecoder {
    /// Create a PNG decoder.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
#[cfg(all(feature = "kitty-graphics", feature = "png"))]
impl DecodePng for RustPngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        use png::{BitDepth, ColorType, Decoder, Transformations};
        use std::io::Cursor;

        let mut decoder = Decoder::new(Cursor::new(data));

        // libghostty only accepts RGBA8 data, so we have to apply some
        // transformations to accept images in other formats, namely
        // expanding palette colors, adding alpha, and stripping
        // 16-bit color depth information back down into 8-bit.
        decoder.set_transformations(Transformations::ALPHA | Transformations::STRIP_16);

        let mut reader = decoder.read_info().ok()?;
        let (width, height) = reader.info().size();
        let pixels = (width as usize).checked_mul(height as usize)?;
        let rgba_len = pixels.checked_mul(4)?;

        // Allocate the output with the provided allocator *before* decoding
        // anything. libghostty caps what the decoder may allocate, and the
        // dimensions come straight from untrusted PNG headers: decoding into
        // an image-sized buffer on the global heap would sidestep that cap
        // and let a tiny PNG claim gigabytes. (The png crate's own row and
        // zlib buffers stay on the global heap, but they are bounded by the
        // row length and the compressed size, not the image size.)
        let mut bytes = Bytes::new_with_alloc(alloc, rgba_len).ok()?;

        // `ALPHA` expands palettes and adds alpha, but keeps grayscale as two
        // channels, so the decoded output is either RGBA8 or GA8. Both fit in
        // the RGBA buffer, so decode into its prefix and expand GA in place.
        let (decoded_len, gray_alpha) = match reader.output_color_type() {
            (ColorType::Rgba, BitDepth::Eight) => (rgba_len, false),
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => (pixels.checked_mul(2)?, true),
            _ => return None,
        };
        // Bail out instead of letting slicing panic below, since a panic
        // here would abort through the `extern "C"` callback.
        if reader.output_buffer_size()? != decoded_len {
            return None;
        }

        let info = reader.next_frame(&mut bytes[..decoded_len]).ok()?;
        if info.buffer_size() != decoded_len {
            return None;
        }
        reader.finish().ok()?;

        if gray_alpha {
            // Walk backwards: pixel `i` is read from `2i..2i + 2` and written
            // to `4i..4i + 4`, which never overlaps a lower pixel that has not
            // been read yet.
            for i in (0..pixels).rev() {
                let (luma, alpha) = (bytes[2 * i], bytes[2 * i + 1]);
                bytes[4 * i..4 * i + 4].copy_from_slice(&[luma, luma, luma, alpha]);
            }
        }

        Some(DecodedImage {
            width,
            height,
            data: bytes,
        })
    }
}

/// A decoded image, returned by a decoder such as [`DecodePng`].
///
/// Pixels are 8-bit RGBA: four bytes per pixel, stored row by row starting
/// at the top-left corner, with no padding between rows. A complete image
/// is therefore `width * height * 4` bytes long.
///
/// The pixel buffer must be allocated with the allocator passed to
/// [`DecodePng::decode_png`]. When the decoder returns the image and its
/// buffer came from that allocator, the library takes ownership of the
/// buffer and frees it with that same allocator.
#[derive(Debug)]
pub struct DecodedImage<'alloc> {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// The decoded RGBA pixels, allocated with the allocator passed to
    /// [`DecodePng::decode_png`].
    pub data: Bytes<'alloc>,
}

#[cfg(all(test, not(miri)))]
mod animation_tests {
    use crate::Terminal;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn terminal() -> Terminal<'static, 'static> {
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .set_kitty_image_storage_limit(64 * 1024 * 1024)
            .unwrap();
        terminal
    }

    /// The first pixel and the generation of image 1.
    fn image(terminal: &Terminal<'_, '_>) -> ([u8; 4], u64) {
        let graphics = terminal.kitty_graphics().unwrap();
        let image = graphics.image(1).unwrap();
        let pixels = image.data().unwrap().unwrap();
        (pixels[..4].try_into().unwrap(), image.generation().unwrap())
    }

    #[test]
    fn nothing_playing_returns_none() {
        let mut terminal = terminal();

        // Empty storage.
        assert_eq!(terminal.kitty_graphics_animation_tick(0).unwrap(), None);

        // A placed static image.
        terminal.vt_write(b"\x1b_Ga=T,f=32,i=1,s=1,v=1,q=2;/wAA/w==\x1b\\");
        assert_eq!(terminal.kitty_graphics_animation_tick(0).unwrap(), None);
        assert_eq!(terminal.kitty_graphics_animation_tick(1000).unwrap(), None);
    }

    #[test]
    fn advances_one_frame_per_tick_and_reports_the_next_frame_time() {
        let mut terminal = terminal();

        // A red root frame shown for 25 ms and a blue second frame shown for
        // 60 ms, playing on a loop.
        terminal.vt_write(
            b"\x1b_Ga=T,f=32,i=1,s=1,v=1,q=2;/wAA/w==\x1b\\\
              \x1b_Ga=f,f=32,i=1,s=1,v=1,z=60,q=2;AAD//w==\x1b\\\
              \x1b_Ga=a,i=1,r=1,z=25,q=2\x1b\\\
              \x1b_Ga=a,i=1,s=3,q=2\x1b\\",
        );
        let (pixel, g0) = image(&terminal);
        assert_eq!(pixel, RED);

        // The first tick starts the red frame's timer and changes nothing.
        let tick = |terminal: &mut Terminal<'_, '_>, now_ms| {
            terminal.kitty_graphics_animation_tick(now_ms).unwrap()
        };
        assert_eq!(tick(&mut terminal, 1000), Some(1025));
        assert_eq!(image(&terminal), (RED, g0));

        // An early tick reports the same absolute time.
        assert_eq!(tick(&mut terminal, 1010), Some(1025));
        assert_eq!(image(&terminal), (RED, g0));

        // On time, the blue frame shows with a new generation.
        assert_eq!(tick(&mut terminal, 1025), Some(1085));
        let (pixel, g1) = image(&terminal);
        assert_eq!(pixel, BLUE);
        assert_ne!(g1, g0);

        // A very late tick still moves on by exactly one frame, back to red,
        // and counts the red frame's gap from now. 5030 ms is chosen so that
        // catching up would look different: blue started at 1025 ms and the
        // loop lasts 85 ms, so 5030 ms is 10 ms into a blue frame on the
        // original schedule.
        assert_eq!(tick(&mut terminal, 5030), Some(5055));
        let (pixel, g2) = image(&terminal);
        assert_eq!(pixel, RED);
        assert_ne!(g2, g1);

        // Stopped playback has nothing to schedule and keeps the frame.
        terminal.vt_write(b"\x1b_Ga=a,i=1,s=1,q=2\x1b\\");
        assert_eq!(tick(&mut terminal, 6000), None);
        assert_eq!(image(&terminal), (RED, g2));
    }
}

#[cfg(all(test, not(miri), feature = "kitty-graphics", feature = "png"))]
mod png_decoder_tests {
    use super::*;
    use crate::alloc::testing::Capped;

    struct Png<'a> {
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        palette: Option<&'a [u8]>,
        trns: Option<&'a [u8]>,
        pixels: &'a [u8],
    }

    impl Png<'_> {
        fn encode(&self) -> Vec<u8> {
            let mut out = Vec::new();
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(self.color);
            encoder.set_depth(self.depth);
            if let Some(palette) = self.palette {
                encoder.set_palette(palette);
            }
            if let Some(trns) = self.trns {
                encoder.set_trns(trns);
            }
            encoder
                .write_header()
                .unwrap()
                .write_image_data(self.pixels)
                .unwrap();
            out
        }
    }

    fn png(color: png::ColorType, depth: png::BitDepth, pixels: &[u8]) -> Png<'_> {
        Png {
            width: 1,
            height: 1,
            color,
            depth,
            palette: None,
            trns: None,
            pixels,
        }
    }

    fn decode(encoded: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        RustPngDecoder
            .decode_png(&Allocator::GLOBAL, encoded)
            .map(|image| (image.width, image.height, image.data.to_vec()))
    }

    #[test]
    fn decodes_every_color_type_to_rgba8() {
        use png::{BitDepth, ColorType};

        let cases: [(&str, Png<'_>, [u8; 4]); 8] = [
            (
                "rgba8",
                png(ColorType::Rgba, BitDepth::Eight, &[255, 0, 128, 7]),
                [255, 0, 128, 7],
            ),
            (
                "rgb8 gains opaque alpha",
                png(ColorType::Rgb, BitDepth::Eight, &[1, 2, 3]),
                [1, 2, 3, 255],
            ),
            (
                "rgba16 is stripped to 8 bits",
                png(
                    ColorType::Rgba,
                    BitDepth::Sixteen,
                    &[0xAB, 0xCD, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC],
                ),
                [0xAB, 0x12, 0x56, 0x9A],
            ),
            (
                "gray8",
                png(ColorType::Grayscale, BitDepth::Eight, &[128]),
                [128, 128, 128, 255],
            ),
            (
                "gray2 is expanded to 8 bits",
                png(ColorType::Grayscale, BitDepth::Two, &[0b1000_0000]),
                [0xAA, 0xAA, 0xAA, 255],
            ),
            (
                "gray+alpha8",
                png(ColorType::GrayscaleAlpha, BitDepth::Eight, &[64, 96]),
                [64, 64, 64, 96],
            ),
            (
                "palette",
                Png {
                    palette: Some(&[0, 0, 0, 10, 20, 30]),
                    ..png(ColorType::Indexed, BitDepth::Eight, &[1])
                },
                [10, 20, 30, 255],
            ),
            (
                "palette with tRNS",
                Png {
                    palette: Some(&[0, 0, 0, 10, 20, 30]),
                    trns: Some(&[255, 42]),
                    ..png(ColorType::Indexed, BitDepth::Eight, &[1])
                },
                [10, 20, 30, 42],
            ),
        ];

        for (name, image, expected) in cases {
            assert_eq!(
                decode(&image.encode()),
                Some((1, 1, expected.to_vec())),
                "{name}"
            );
        }
    }

    #[test]
    fn expands_multi_pixel_grayscale_alpha_in_place() {
        // Several pixels exercise the backwards in-place expansion; a single
        // pixel would not catch overlapping reads and writes.
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let image = Png {
            width: 3,
            height: 2,
            ..png(
                png::ColorType::GrayscaleAlpha,
                png::BitDepth::Eight,
                &pixels,
            )
        };
        let expected: Vec<u8> = pixels
            .chunks_exact(2)
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect();
        assert_eq!(decode(&image.encode()), Some((3, 2, expected)));
    }

    #[test]
    fn rejects_malformed_input() {
        let encoded = png(png::ColorType::Rgba, png::BitDepth::Eight, &[1, 2, 3, 4]).encode();
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(b"definitely not a png"), None);
        assert_eq!(decode(&encoded[..encoded.len() / 2]), None);
    }

    #[test]
    fn oversized_dimensions_are_rejected_by_the_provided_allocator() {
        // A tiny PNG whose header claims 16000x16000 RGBA8 pixels (~1 GB).
        // The decoder must ask the provided allocator for the output first,
        // so the cap rejects it before any work is done on the global heap.
        let mut encoded = png(png::ColorType::Rgba, png::BitDepth::Eight, &[0; 4]).encode();
        encoded[16..24].copy_from_slice(&[0, 0, 0x3E, 0x80, 0, 0, 0x3E, 0x80]);
        let crc = crc32(&encoded[12..29]);
        encoded[29..33].copy_from_slice(&crc.to_be_bytes());

        let capped = Capped::new(1024 * 1024);
        assert!(
            RustPngDecoder
                .decode_png(&capped.allocator(), &encoded)
                .is_none()
        );
        assert_eq!(capped.largest_request(), 16000 * 16000 * 4);
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    0xEDB8_8320 ^ (crc >> 1)
                };
            }
        }
        !crc
    }
}
