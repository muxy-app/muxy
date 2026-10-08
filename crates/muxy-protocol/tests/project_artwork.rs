use muxy_protocol::{ErrorCode, ProjectPatch};

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
