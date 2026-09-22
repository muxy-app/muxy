use muxy_protocol::{ErrorCode, ProjectPatch};
use std::sync::Arc;

#[test]
fn artwork_preserves_emoji_and_round_trips_symbols_logos_and_removal() {
    for icon in ["👩🏽‍💻", "🇩🇪", "sf:folder.fill", "sf:square.grid.2x2"] {
        let patch = ProjectPatch::Icon(Some(icon.into()));
        assert_eq!(patch.validate(), Ok(()));
        let bytes = postcard::to_allocvec(&patch).expect("encode");
        assert_eq!(
            postcard::from_bytes::<ProjectPatch>(&bytes).expect("decode"),
            patch
        );
    }
    for icon in ["", "ab", "sf:", "sf:../path", "sf:two words"] {
        assert_eq!(
            ProjectPatch::Icon(Some(icon.into())).validate(),
            Err(ErrorCode::BadRequest)
        );
    }
    for patch in [
        ProjectPatch::Logo(Some(Arc::from(
            include_bytes!("fixtures/project-logo.png").as_slice(),
        ))),
        ProjectPatch::Logo(None),
    ] {
        assert_eq!(patch.validate(), Ok(()));
        let bytes = postcard::to_allocvec(&patch).expect("encode");
        assert_eq!(
            postcard::from_bytes::<ProjectPatch>(&bytes).expect("decode"),
            patch
        );
    }
}

#[test]
fn logo_bounds_reject_non_images_large_images_and_rectangles() {
    let valid = include_bytes!("fixtures/project-logo.png");
    let mut too_wide = valid.to_vec();
    too_wide[16..20].copy_from_slice(&8192_u32.to_be_bytes());
    let mut rectangle = valid.to_vec();
    rectangle[20..24].copy_from_slice(&1_u32.to_be_bytes());
    for bytes in [
        vec![],
        vec![0; 64],
        too_wide,
        rectangle,
        vec![0; muxy_protocol::MAX_PROJECT_LOGO_BYTES + 1],
    ] {
        assert_eq!(
            ProjectPatch::Logo(Some(bytes.into())).validate(),
            Err(ErrorCode::BadRequest)
        );
    }
}
