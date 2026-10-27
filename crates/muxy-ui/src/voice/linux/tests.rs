#![allow(clippy::expect_used, reason = "Offline voice regression fixtures")]

use super::*;
use std::sync::atomic::AtomicU64;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "muxy-voice-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("fixture directory");
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn english_only_models_cannot_silently_override_the_requested_language() {
    let fixture = Fixture::new();
    let model = fixture.0.join("model.bin");
    std::fs::write(
        &model,
        [b"lmgg".as_slice(), &51_864_u32.to_le_bytes()].concat(),
    )
    .expect("model header");
    assert!(validate_model(&model).is_err());
    std::fs::write(
        &model,
        [b"lmgg".as_slice(), &51_865_u32.to_le_bytes()].concat(),
    )
    .expect("model header");
    assert_eq!(validate_model(&model), Ok(model));
}
