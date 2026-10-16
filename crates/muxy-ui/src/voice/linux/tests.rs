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

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        let script = format!(
            "#!/usr/bin/python3\nimport os, sys, time\nfrom pathlib import Path\nroot = Path(__file__).parent\n(root / '{name}.pid').write_text(str(os.getpid()))\n{body}\n"
        );
        std::fs::write(&path, script).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .expect("executable");
        path
    }

    fn pid(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(format!("{name}.pid"))).expect("child PID")
    }

    fn tools(&self, capture: &str, transcribe: &str) -> Tools {
        Tools {
            capture: self.script("capture", capture),
            transcribe: self.script("transcribe", transcribe),
            speech_model: self.0.join("speech model.bin"),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "voice worker did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn gone(pid: &str) -> bool {
    !Path::new("/proc").join(pid).exists()
}

const AUDIO: &str = "assert sys.argv[sys.argv.index('--server') + 1].startswith('unix:/')\nassert '--device=@DEFAULT_SOURCE@' in sys.argv\nwhile True:\n    os.write(1, b'\\x00\\x10' * 1600)\n    time.sleep(0.01)";
const TRANSCRIPT: &str = "data = sys.stdin.buffer.read()\nassert data[:4] == b'RIFF' and data[8:16] == b'WAVEfmt '\nassert int.from_bytes(data[40:44], 'little') == len(data) - 44\nassert data[44:] and len(data[44:]) % 2 == 0\nassert sys.argv[sys.argv.index('--file') + 1] == '-'\nassert sys.argv[sys.argv.index('--language') + 1] == 'de'\nassert sys.argv[sys.argv.index('--model') + 1].endswith('model $(touch injected).bin')\nassert '--no-gpu' in sys.argv and '--translate' not in sys.argv\nprint(' Hallo Welt. ')";

#[test]
fn pause_releases_the_device_resume_reopens_and_finish_delivers_local_audio() {
    let fixture = Fixture::new();
    let tools = fixture.tools(AUDIO, TRANSCRIPT);
    let model = fixture.0.join("model $(touch injected).bin");
    let shared = Arc::new(Shared::default());
    let recorder = Recorder {
        shared: shared.clone(),
    };
    let worker = std::thread::spawn(move || record(&tools, &model, "de", &shared));
    until(|| recorder.snapshot().phase == Phase::Recording);
    let first = fixture.pid("capture");
    recorder.pause(true);
    until(|| recorder.snapshot().phase == Phase::Paused);
    assert!(gone(&first));
    recorder.pause(false);
    until(|| recorder.snapshot().phase == Phase::Recording);
    let second = fixture.pid("capture");
    assert_ne!(first, second);
    recorder.finish();
    until(|| worker.is_finished());
    assert_eq!(worker.join().expect("worker"), Ok(()));
    assert_eq!(recorder.finished(), Some("Hallo Welt.".into()));
    assert!(gone(&second));
    assert!(gone(&fixture.pid("transcribe")));
    assert!(!fixture.0.join("injected").exists());
}

#[test]
fn the_recording_limit_releases_the_device_and_preserves_audio_for_finish() {
    let fixture = Fixture::new();
    let tools = fixture.tools(AUDIO, "sys.exit(1)");
    let shared = Arc::new(Shared::default());
    let recorder = Recorder {
        shared: shared.clone(),
    };
    let worker = std::thread::spawn(move || capture(&tools, &shared, CAPTURE_IDLE, 8192));
    until(|| recorder.snapshot().phase == Phase::Paused);
    assert!(gone(&fixture.pid("capture")));
    assert!(recorder.snapshot().error.is_some());
    recorder.finish();
    until(|| worker.is_finished());
    assert_eq!(worker.join().expect("worker").expect("audio").len(), 8192);
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

#[test]
#[ignore = "requires whisper-cli, multilingual and Silero models, and the bundled 16 kHz JFK WAV sample"]
fn real_models_reject_keyboard_noise_and_transcribe_speech_from_memory() {
    let model = PathBuf::from(std::env::var_os("MUXY_TEST_VOICE_MODEL").expect("model"));
    let speech_model =
        PathBuf::from(std::env::var_os("MUXY_TEST_VOICE_VAD_MODEL").expect("Silero model"));
    let sample = std::env::var_os("MUXY_TEST_VOICE_SAMPLE").expect("sample");
    let wav = std::fs::read(sample).expect("WAV sample");
    assert_eq!(&wav[..4], b"RIFF");
    let mut offset = 12;
    let mut pcm = Vec::new();
    while offset + 8 <= wav.len() {
        let length =
            u32::from_le_bytes(wav[offset + 4..offset + 8].try_into().expect("length")) as usize;
        let data = &wav[offset + 8..offset + 8 + length];
        match &wav[offset..offset + 4] {
            b"fmt " => {
                assert_eq!(&data[..4], &[1, 0, 1, 0]);
                assert_eq!(&data[4..8], &SAMPLE_RATE.to_le_bytes());
                assert_eq!(&data[14..16], &16_u16.to_le_bytes());
            }
            b"data" => pcm = data.to_vec(),
            _ => {}
        }
        offset += 8 + length + length % 2;
    }
    assert!(!pcm.is_empty());
    let tools = Tools {
        capture: PathBuf::new(),
        transcribe: executable("whisper-cli").expect("whisper-cli"),
        speech_model: validate_speech_model(&speech_model).expect("speech detection model"),
    };
    check_version(&tools.transcribe, &Shared::default()).expect("supported CLI");
    assert!(
        transcribe(
            &tools,
            &validate_model(&model).expect("multilingual model"),
            "en",
            keyboard_noise(),
            &Shared::default(),
            TRANSCRIPTION_TIMEOUT,
        )
        .expect("noise detection")
        .is_empty()
    );
    let text = transcribe(
        &tools,
        &validate_model(&model).expect("multilingual model"),
        "en",
        pcm,
        &Shared::default(),
        TRANSCRIPTION_TIMEOUT,
    )
    .expect("transcript");
    assert!(
        text.to_lowercase()
            .contains("ask not what your country can do for you"),
        "{text}"
    );
}

fn keyboard_noise() -> Vec<u8> {
    let mut state = 508_u32;
    let mut random = |limit| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state % limit
    };
    let mut samples: Vec<i16> = (0..SAMPLE_RATE * 8)
        .map(|_| i16::try_from(random(25)).expect("background level") - 12)
        .collect();
    let mut position = SAMPLE_RATE as usize / 2;
    while position < samples.len() - SAMPLE_RATE as usize / 2 {
        let mut amplitude = 9000;
        for sample in &mut samples[position..position + 480] {
            let noise = i32::try_from(random(20_001)).expect("impulse level") - 10_000;
            *sample += i16::try_from(amplitude * noise / 10_000).expect("PCM sample");
            amplitude = amplitude * 98 / 100;
        }
        position += usize::try_from(1800 + random(4201)).expect("impulse spacing");
    }
    samples.into_iter().flat_map(i16::to_le_bytes).collect()
}

#[test]
fn versions_without_stdout_text_support_are_rejected_before_capture() {
    let fixture = Fixture::new();
    for (version, supported) in [("1.9.5-dev", true), ("1.8.2", false), ("unknown", false)] {
        let program = fixture.script(
            "transcribe",
            &format!("print('whisper.cpp version: {version}')"),
        );
        assert_eq!(
            check_version(&program, &Shared::default()).is_ok(),
            supported
        );
        assert!(gone(&fixture.pid("transcribe")));
    }
}
