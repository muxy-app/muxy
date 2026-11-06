use super::*;
use std::io::Cursor;

fn raster(format: image::ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        32,
        16,
        image::Rgb([255, 0, 0]),
    ));
    let image = if format == image::ImageFormat::Ico {
        image::DynamicImage::ImageRgba8(image.into_rgba8())
    } else {
        image
    };
    image.write_to(&mut bytes, format).expect("encode fixture");
    bytes.into_inner()
}

#[test]
fn decodes_every_requested_raster_format_and_svg() {
    for format in [
        image::ImageFormat::Ico,
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
        image::ImageFormat::Gif,
    ] {
        let decoded = decode(&raster(format)).unwrap_or_else(|| panic!("decode {format:?}"));
        assert_eq!(decoded.dimensions(), (32, 16));
        assert!(decoded.get_pixel(0, 0)[0] > 240);
    }
    let decoded = decode(br#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="red"/></svg>"#).expect("svg");
    assert_eq!(decoded.width(), 2 * decoded.height());
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn decodes_png_and_legacy_icns_and_rejects_invalid_lengths() {
    let png = raster(image::ImageFormat::Png);
    let mut bytes = b"icns".to_vec();
    bytes.extend(u32::try_from(png.len() + 16).expect("size").to_be_bytes());
    bytes.extend(b"icp5");
    bytes.extend(u32::try_from(png.len() + 8).expect("size").to_be_bytes());
    bytes.extend(png);
    assert!(decode(&bytes).is_some());
    bytes[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&bytes).is_none());
    assert!(decode(b"icns").is_none());
    let mut family = ::icns::IconFamily::new();
    let image = ::icns::Image::from_data(
        ::icns::PixelFormat::RGBA,
        16,
        16,
        [255, 0, 0, 128].repeat(256),
    )
    .expect("fixture");
    family
        .add_icon_with_type(&image, ::icns::IconType::RGB24_16x16)
        .expect("legacy icon");
    let mut bytes = Vec::new();
    family.write(&mut bytes).expect("write icns");
    assert_eq!(
        decode(&bytes).expect("legacy").get_pixel(0, 0).0,
        [255, 0, 0, 128]
    );
}

fn jpeg2000_icns(data: &[u8]) -> Vec<u8> {
    let mut bytes = b"icns".to_vec();
    bytes.extend(u32::try_from(data.len() + 16).expect("size").to_be_bytes());
    bytes.extend(b"icp4");
    bytes.extend(u32::try_from(data.len() + 8).expect("size").to_be_bytes());
    bytes.extend(data);
    bytes
}

#[test]
fn decodes_jpeg2000_icns() {
    let bytes = jpeg2000_icns(include_bytes!("fixtures/red.jp2"));
    let image = decode(&bytes).expect("JPEG 2000 ICNS");
    assert_eq!(image.dimensions(), (16, 16));
    assert!(image.get_pixel(0, 0)[0] > 240);
}

#[test]
fn rejects_jpeg2000_tile_bombs_and_keeps_discovery_fallback() {
    for (size, subsampling) in [(1024_u32, 1_u8), (65_280, 255)] {
        let mut data = include_bytes!("fixtures/red.jp2").to_vec();
        let start = data
            .windows(4)
            .position(|bytes| bytes == b"\xffO\xffQ")
            .expect("codestream");
        for offset in [8, 12] {
            data[start + offset..start + offset + 4].copy_from_slice(&size.to_be_bytes());
        }
        for offset in [24, 28] {
            data[start + offset..start + offset + 4].copy_from_slice(&1_u32.to_be_bytes());
        }
        for offset in [43, 44, 46, 47, 49, 50] {
            data[start + offset] = subsampling;
        }
        let header = data
            .windows(4)
            .position(|bytes| bytes == b"ihdr")
            .expect("image header");
        for offset in [4, 8] {
            data[header + offset..header + offset + 4].copy_from_slice(&size.to_be_bytes());
        }
        let image = hayro_jpeg2000::Image::new(&data, &hayro_jpeg2000::DecodeSettings::default())
            .expect("header");
        assert_eq!(image.width(), size / u32::from(subsampling));
        assert_eq!(image.height(), image.width());
        assert!(image.width() <= 1024);
        let bytes = jpeg2000_icns(&data);
        assert_eq!(bytes.len(), 282);
        assert!(decode(&bytes).is_none());
        let root = tempfile::tempdir().expect("project");
        std::fs::write(root.path().join("favicon.icns"), bytes).expect("icon");
        let source = Source::Local(root.path().into());
        assert!(discover(&source).is_none());
        std::fs::write(
            root.path().join("logo.png"),
            raster(image::ImageFormat::Png),
        )
        .expect("fallback");
        assert!(discover(&source).is_some());
    }
}

fn jpeg2000_with_coding_style(
    size: u32,
    marker: u8,
    tile_part: Option<u8>,
    style: &[u8],
) -> Vec<u8> {
    let mut data = include_bytes!("fixtures/red.jp2").to_vec();
    let find = |data: &[u8], signature: &[u8]| {
        data.windows(signature.len())
            .position(|bytes| bytes == signature)
            .expect("marker")
    };
    let start = find(&data, b"\xffO\xffQ");
    for offset in [8, 12, 24, 28] {
        data[start + offset..start + offset + 4].copy_from_slice(&size.to_be_bytes());
    }
    let header = find(&data, b"ihdr");
    for offset in [4, 8] {
        data[header + offset..header + offset + 4].copy_from_slice(&size.to_be_bytes());
    }
    let cod = find(&data, b"\xffR");
    data[cod + 6..cod + 8].copy_from_slice(&style[2..4]);
    let payload = if marker == 0x52 {
        style.to_vec()
    } else {
        let mut payload = vec![0, style[0]];
        payload.extend(&style[5..]);
        payload
    };
    let mut segment = vec![0xff, marker];
    segment.extend(
        u16::try_from(payload.len() + 2)
            .expect("length")
            .to_be_bytes(),
    );
    segment.extend(payload);
    match tile_part {
        None => {
            let end = cod + 2 + usize::from(u16::from_be_bytes([data[cod + 2], data[cod + 3]]));
            data.splice(if marker == 0x52 { cod..end } else { end..end }, segment);
        }
        Some(part) => {
            let start = find(&data, b"\xff\x90");
            if part == 0 {
                let length = u32::from_be_bytes(data[start + 6..start + 10].try_into().unwrap())
                    + u32::try_from(segment.len()).expect("length");
                data[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
                data.splice(start + 12..start + 12, segment);
            } else {
                let mut tile = data[start..start + 12].to_vec();
                tile[6..10].copy_from_slice(
                    &u32::try_from(segment.len() + 14)
                        .expect("length")
                        .to_be_bytes(),
                );
                tile[10] = part;
                tile.extend(segment);
                tile.extend(b"\xff\x93");
                data.splice(data.len() - 2..data.len() - 2, tile);
            }
        }
    }
    let start = find(&data, b"jp2c") - 4;
    let length = u32::try_from(data.len() - start).expect("length");
    data[start..start + 4].copy_from_slice(&length.to_be_bytes());
    data
}

#[test]
fn rejects_jpeg2000_coding_bombs_in_main_and_all_tile_headers() {
    for marker in [0x52, 0x53] {
        for tile_part in [None, Some(0), Some(1)] {
            let ordinary = [0, 0, 0, 1, 0, 6, 2, 2, 0, 0];
            let data = jpeg2000_with_coding_style(16, marker, tile_part, &ordinary);
            assert!(
                decode(&jpeg2000_icns(&data)).is_some(),
                "{marker}: {tile_part:?}"
            );
            for style in [
                &[1, 0, 0, 32, 0, 0, 4, 4, 0, 0, 0][..],
                &[0, 0, 0, 32, 0, 0, 0, 0, 0, 0],
                &[0, 0, 0, 32, 0, 0, 2, 2, 0, 0],
            ] {
                let data = jpeg2000_with_coding_style(1024, marker, tile_part, style);
                let image =
                    hayro_jpeg2000::Image::new(&data, &hayro_jpeg2000::DecodeSettings::default())
                        .expect("header");
                assert_eq!((image.width(), image.height()), (1024, 1024));
                let bytes = jpeg2000_icns(&data);
                assert!(decode(&bytes).is_none(), "{marker}: {tile_part:?}");
                let root = tempfile::tempdir().expect("project");
                std::fs::write(root.path().join("favicon.icns"), bytes).expect("icon");
                let source = Source::Local(root.path().into());
                assert!(discover(&source).is_none());
                std::fs::write(
                    root.path().join("logo.png"),
                    raster(image::ImageFormat::Png),
                )
                .expect("fallback");
                assert!(discover(&source).is_some());
            }
        }
    }
}

#[test]
fn jpeg2000_checks_extended_and_open_ended_codestream_boxes() {
    let original = include_bytes!("fixtures/red.jp2");
    let start = original
        .windows(4)
        .position(|bytes| bytes == b"jp2c")
        .expect("codestream box")
        - 4;
    let mut open_ended = original.to_vec();
    open_ended[start..start + 4].copy_from_slice(&0_u32.to_be_bytes());
    assert!(decode(&jpeg2000_icns(&open_ended)).is_some());
    let mut extended = original[..start].to_vec();
    extended.extend(1_u32.to_be_bytes());
    extended.extend(b"jp2c");
    extended.extend(
        u64::try_from(original.len() - start + 8)
            .expect("length")
            .to_be_bytes(),
    );
    extended.extend(&original[start + 8..]);
    assert!(decode(&jpeg2000_icns(&extended)).is_some());
    for length in [0_u64, 8, 15, u64::MAX] {
        let mut invalid = extended.clone();
        invalid[start + 8..start + 16].copy_from_slice(&length.to_be_bytes());
        assert!(decode(&jpeg2000_icns(&invalid)).is_none());
    }
    for length in [2_u32, 7, u32::MAX] {
        let mut invalid = original.to_vec();
        invalid[start..start + 4].copy_from_slice(&length.to_be_bytes());
        assert!(decode(&jpeg2000_icns(&invalid)).is_none());
    }
    for end in 0..original.len() {
        assert!(decode(&jpeg2000_icns(&original[..end])).is_none());
    }
}

#[test]
fn discovers_all_names_extensions_and_locations() {
    for directory in DIRECTORIES {
        for (index, extension) in EXTENSIONS.iter().enumerate() {
            let root = tempfile::tempdir().expect("directory");
            let folder = root.path().join(directory);
            std::fs::create_dir_all(&folder).expect("mkdir");
            std::fs::write(
                folder.join(format!("{}.{}", NAMES[index], extension)),
                raster(image::ImageFormat::Png),
            )
            .expect("icon");
            assert!(
                discover(&Source::Local(root.path().into())).is_some(),
                "{directory} {extension}"
            );
        }
    }
    for name in NAMES {
        assert!(rank(Path::new(&format!("{name}.png"))).is_some());
    }
    assert!(rank(Path::new("FAVICON.PNG")).is_some());
    assert!(rank(Path::new("photo.png")).is_none());
    assert!(rank(Path::new("favicon.txt")).is_none());
    assert!(rank(Path::new("../favicon.png")).is_none());
}

#[test]
fn skips_broken_candidates_and_uses_stable_priority() {
    let root = tempfile::tempdir().expect("directory");
    std::fs::write(root.path().join("favicon.svg"), b"broken").expect("write");
    std::fs::write(
        root.path().join("logo.png"),
        raster(image::ImageFormat::Png),
    )
    .expect("write");
    let source = Source::Local(root.path().into());
    assert_eq!(
        discover(&source)
            .expect("logo")
            .as_bytes(0)
            .expect("pixels")[..4],
        [0, 0, 255, 255]
    );
    image::RgbaImage::from_pixel(32, 16, image::Rgba([0, 255, 0, 255]))
        .save(root.path().join("favicon.png"))
        .expect("favicon");
    assert_eq!(
        discover(&source)
            .expect("favicon")
            .as_bytes(0)
            .expect("pixels")[..4],
        [0, 255, 0, 255]
    );
    std::fs::write(
        root.path().join("favicon.svg"),
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="blue"/></svg>"#,
    ).expect("svg");
    assert_eq!(
        discover(&source)
            .expect("svg favicon")
            .as_bytes(0)
            .expect("pixels")[..4],
        [255, 0, 0, 255]
    );
    std::fs::write(root.path().join("favicon.svg"), b"broken").expect("invalid svg");
    std::fs::remove_file(root.path().join("favicon.png")).expect("remove favicon");
    let mut names = ["logo.png", "favicon.png", "favicon.svg", "icon.ico"];
    names.sort_by_key(|name| rank(Path::new(name)));
    assert_eq!(
        names,
        ["favicon.svg", "favicon.png", "icon.ico", "logo.png"]
    );
    std::fs::remove_file(root.path().join("logo.png")).expect("remove");
    assert!(discover(&source).is_none());
}

#[test]
fn skips_files_outside_project_and_oversized_or_nonregular_candidates() {
    let root = tempfile::tempdir().expect("directory");
    let outside = tempfile::NamedTempFile::new().expect("file");
    std::fs::write(outside.path(), raster(image::ImageFormat::Png)).expect("write");
    let icon = root.path().join("favicon.png");
    std::os::unix::fs::symlink(outside.path(), &icon).expect("symlink");
    let source = Source::Local(root.path().into());
    assert!(discover(&source).is_none());
    std::fs::remove_file(&icon).expect("remove");
    std::fs::File::create(&icon)
        .expect("file")
        .set_len(MAX_BYTES + 1)
        .expect("length");
    assert!(discover(&source).is_none());
    std::fs::remove_file(&icon).expect("remove");
    std::fs::create_dir(&icon).expect("directory");
    assert!(discover(&source).is_none());
}

#[test]
fn remote_discovery_reads_the_owning_project_through_its_server() {
    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{
        CONTROL, FileBytes, FileEntry, FileInfo, FilesAction, FilesReply, Message, ReplyBody,
        RequestBody, ServerInfo,
    };
    let project = ProjectId::new();
    let bytes = raster(image::ImageFormat::Png);
    let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
    let worker = std::thread::spawn(move || {
        let mut decoder = Decoder::new(server.try_clone().expect("clone"));
        let mut encoder = Encoder::new(server);
        decoder.next().expect("hello");
        encoder
            .send(
                CONTROL,
                &Message::HelloReply {
                    versions: muxy_protocol::SUPPORTED.to_vec(),
                    server: ServerInfo::current(),
                    features: Vec::new(),
                },
            )
            .expect("hello reply");
        let path = ServerPath::from(Path::new("public/favicon.png"));
        let replies = [
            (
                FilesAction::List(ServerPath::from(Path::new(""))),
                FilesReply::Entries(vec![]),
            ),
            (
                FilesAction::List(ServerPath::from(Path::new("public"))),
                FilesReply::Entries(vec![FileEntry {
                    name: ServerPath::from(Path::new("favicon.png")),
                    path: path.clone(),
                    is_directory: false,
                    is_ignored: false,
                }]),
            ),
            (
                FilesAction::Stat(path.clone()),
                FilesReply::Info(FileInfo {
                    name: ServerPath::from(Path::new("favicon.png")),
                    path: path.clone(),
                    is_directory: false,
                    size: bytes.len() as u64,
                }),
            ),
            (
                FilesAction::ReadBytes(path.clone()),
                FilesReply::Bytes(FileBytes { path, bytes }),
            ),
        ];
        for (action, reply) in replies {
            let (
                _,
                Message::Request {
                    id,
                    body: RequestBody::Files(request),
                },
            ) = decoder.next().expect("file request")
            else {
                panic!("expected file request")
            };
            assert_eq!(request.project, project);
            assert_eq!(request.action, action);
            encoder
                .send(
                    CONTROL,
                    &Message::Reply {
                        id,
                        body: ReplyBody::Files(reply),
                    },
                )
                .expect("file reply");
        }
    });
    let client = Client::from_stream(Box::new(client)).expect("client");
    assert!(discover(&Source::Remote(client, project)).is_some());
    worker.join().expect("server");
}
