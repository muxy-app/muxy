use super::{
    Arc, AtomicBool, Duration, Language, Mutex, Ordering, Phase, PoisonError, Recorder, Shared,
};
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::{
    AnyThread, DefinedClass, define_class, msg_send,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
};
use objc2_av_foundation::{
    AVCaptureAudioDataOutput, AVCaptureAudioDataOutputSampleBufferDelegate, AVCaptureConnection,
    AVCaptureDevice, AVCaptureDeviceInput, AVCaptureOutput, AVCaptureSession, AVMediaTypeAudio,
};
use objc2_core_audio::{
    AudioObjectAddPropertyListenerBlock, AudioObjectPropertyAddress,
    AudioObjectRemovePropertyListenerBlock, kAudioHardwarePropertyDefaultInputDevice,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{
    NSArray, NSBundle, NSError, NSLocale, NSObject, NSObjectProtocol, NSString,
};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognitionTaskHint, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Instant;

use crate::tr;

const FINALIZATION: Duration = Duration::from_secs(2);
const DEFAULT_LANGUAGE: &str = "en-US";

pub const CAPABILITY: muxy_core::capability::Capability =
    muxy_core::capability::Capability::Supported;
pub const LOCAL_MODEL: bool = false;

pub fn languages() -> Vec<Language> {
    unsafe {
        let current = NSLocale::currentLocale();
        let mut languages = Vec::new();
        for locale in &SFSpeechRecognizer::supportedLocales().allObjects() {
            let Some(recognizer) =
                SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            else {
                continue;
            };
            if !recognizer.supportsOnDeviceRecognition() {
                continue;
            }
            let id = locale.localeIdentifier().to_string();
            let name = current
                .localizedStringForLocaleIdentifier(&locale.localeIdentifier())
                .to_string();
            languages.push(Language {
                id,
                name,
                preferred: false,
            });
        }
        languages.sort_by_key(|language| language.name.to_lowercase());
        let system = system_language(&languages, &NSLocale::preferredLanguages());
        for language in &mut languages {
            language.preferred = system.as_deref() == Some(language.id.as_str());
        }
        languages
    }
}

fn system_language(available: &[Language], preferences: &NSArray<NSString>) -> Option<String> {
    let ids: Vec<_> = available
        .iter()
        .map(|language| NSString::from_str(&language.id))
        .collect();
    let matched = NSBundle::preferredLocalizationsFromArray_forPreferences(
        &NSArray::from_retained_slice(&ids),
        Some(preferences),
    )
    .firstObject()?;
    let requested = preferences.containsObject(&matched);
    let matched = matched.to_string();
    let language = |id: &str| id.split('-').next().map(str::to_owned);
    let default = available
        .iter()
        .any(|candidate| candidate.id == DEFAULT_LANGUAGE);
    if default && !requested && language(&matched) == language(DEFAULT_LANGUAGE) {
        return Some(DEFAULT_LANGUAGE.into());
    }
    Some(matched)
}

pub(super) fn start(language: String, _model: String) -> Result<Recorder, String> {
    for key in [
        "NSMicrophoneUsageDescription",
        "NSSpeechRecognitionUsageDescription",
    ] {
        if NSBundle::mainBundle()
            .objectForInfoDictionaryKey(&NSString::from_str(key))
            .is_none()
        {
            return Err(tr!(
                "Open the packaged Muxy app to enable microphone and speech permissions."
            )
            .to_string());
        }
    }
    let shared = Arc::new(Shared::default());
    let worker = shared.clone();
    std::thread::Builder::new()
        .name("composer-dictation".into())
        .spawn(move || {
            objc2::rc::autoreleasepool(|_| {
                if let Err(error) = record(&language, &worker) {
                    let mut snapshot = worker
                        .snapshot
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    snapshot.phase = Phase::Failed;
                    snapshot.error = Some(error);
                }
            });
        })
        .map_err(|error| error.to_string())?;
    Ok(Recorder { shared })
}

fn authorize(shared: &Shared) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let audio = RcBlock::new(move |granted: Bool| {
        let _ = sender.send(granted.as_bool());
    });
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(
            AVMediaTypeAudio.ok_or_else(|| tr!("Audio capture is unavailable").to_string())?,
            &audio,
        );
    }
    if !permission(&receiver, shared)? {
        return Err(tr!("Microphone access is disabled. Enable Muxy in System Settings → Privacy & Security → Microphone.").to_string());
    }
    if shared.cancelled.load(Ordering::Acquire) {
        return Err(tr!("Dictation cancelled").to_string());
    }
    let (sender, receiver) = mpsc::channel();
    let speech = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = sender.send(status == SFSpeechRecognizerAuthorizationStatus::Authorized);
    });
    unsafe {
        SFSpeechRecognizer::requestAuthorization(&speech);
    }
    if !permission(&receiver, shared)? {
        return Err(tr!("Speech recognition access is disabled. Enable Muxy in System Settings → Privacy & Security → Speech Recognition.").to_string());
    }
    Ok(())
}

fn permission(receiver: &mpsc::Receiver<bool>, shared: &Shared) -> Result<bool, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if shared.cancelled.load(Ordering::Acquire) {
            return Err(tr!("Dictation cancelled").to_string());
        }
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(granted) => return Ok(granted),
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
            Err(_) => {
                return Err(
                    tr!("Speech permission request did not complete. Try again.").to_string(),
                );
            }
        }
    }
}

#[derive(Debug)]
struct CaptureSink {
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    shared: Arc<Shared>,
    active: Arc<AtomicBool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MuxyComposerAudioCapture"]
    #[ivars = CaptureSink]
    #[derive(Debug)]
    struct CaptureDelegate;

    unsafe impl NSObjectProtocol for CaptureDelegate {}
    unsafe impl AVCaptureAudioDataOutputSampleBufferDelegate for CaptureDelegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn capture(
            &self,
            _: &AVCaptureOutput,
            buffer: &CMSampleBuffer,
            connection: &AVCaptureConnection,
        ) {
            let sink = self.ivars();
            if !sink.active.load(Ordering::Acquire)
                || sink.shared.cancelled.load(Ordering::Acquire)
                || sink.shared.paused.load(Ordering::Acquire)
            {
                return;
            }
            unsafe {
                sink.request.appendAudioSampleBuffer(buffer);
            }
            let power = unsafe {
                connection
                    .audioChannels()
                    .iter()
                    .map(|channel| channel.averagePowerLevel())
                    .fold(-160.0_f32, f32::max)
            };
            let mut snapshot = sink
                .shared
                .snapshot
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            snapshot.level = if power.is_finite() {
                (power.clamp(-50.0, 0.0) + 50.0) / 50.0
            } else {
                0.0
            };
        }
    }
);

struct Capture {
    session: Retained<AVCaptureSession>,
    output: Retained<AVCaptureAudioDataOutput>,
    input: Retained<AVCaptureDeviceInput>,
    request: Option<Retained<SFSpeechAudioBufferRecognitionRequest>>,
    task: Retained<SFSpeechRecognitionTask>,
    _delegate: Retained<CaptureDelegate>,
    queue: DispatchRetained<DispatchQueue>,
    active: Arc<AtomicBool>,
}

impl Capture {
    fn end_audio(&mut self) {
        let Some(request) = self.request.take() else {
            return;
        };
        unsafe {
            self.output.setSampleBufferDelegate_queue(None, None);
        }
        self.queue.exec_sync(|| {});
        unsafe {
            request.endAudio();
            self.session.stopRunning();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        self.end_audio();
        unsafe {
            self.task.cancel();
            self.session.removeInput(&self.input);
            self.session.removeOutput(&self.output);
        }
    }
}

fn language_id(locale: &str) -> &str {
    match locale.split(['-', '_']).next().unwrap_or_default() {
        "nb" => "no",
        "iw" => "he",
        "jv" => "jw",
        id => id,
    }
}

fn select_language<'a>(available: &'a [Language], requested: &str) -> Option<&'a Language> {
    let requested = requested.replace('_', "-").to_ascii_lowercase();
    available
        .iter()
        .find(|candidate| {
            candidate
                .id
                .replace('_', "-")
                .eq_ignore_ascii_case(&requested)
        })
        .or_else(|| {
            if requested.is_empty() || requested.contains('-') {
                return None;
            }
            available
                .iter()
                .filter(|candidate| {
                    language_id(&candidate.id).eq_ignore_ascii_case(language_id(&requested))
                })
                .min_by_key(|candidate| !candidate.preferred)
        })
        .or_else(|| available.iter().find(|candidate| candidate.preferred))
        .or_else(|| {
            available.iter().find(|candidate| {
                candidate
                    .id
                    .replace('_', "-")
                    .eq_ignore_ascii_case(DEFAULT_LANGUAGE)
            })
        })
        .or_else(|| available.first())
}

fn capture(language: &str, shared: &Arc<Shared>) -> Result<Capture, String> {
    unsafe {
        let available = languages();
        let selected = select_language(&available, language)
            .ok_or_else(|| tr!("No on-device speech language is installed. Add one in System Settings → Keyboard → Dictation.").to_string())?;
        let locale = NSLocale::initWithLocaleIdentifier(
            NSLocale::alloc(),
            &NSString::from_str(&selected.id),
        );
        let recognizer = SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            .ok_or_else(|| {
            tr!("This language is unavailable for speech recognition").to_string()
        })?;
        if !recognizer.supportsOnDeviceRecognition() || !recognizer.isAvailable() {
            return Err(tr!("On-device speech recognition is unavailable for this language. Choose another dictation language.").to_string());
        }
        let device = AVCaptureDevice::defaultDeviceWithMediaType(
            AVMediaTypeAudio.ok_or_else(|| tr!("Audio capture is unavailable").to_string())?,
        )
        .ok_or_else(|| tr!("No microphone is available").to_string())?;
        let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
            .map_err(|error| error.to_string())?;
        let session = AVCaptureSession::new();
        let output = AVCaptureAudioDataOutput::new();
        if !session.canAddInput(&input) || !session.canAddOutput(&output) {
            return Err(tr!("Could not configure microphone capture").to_string());
        }
        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.setShouldReportPartialResults(true);
        request.setRequiresOnDeviceRecognition(true);
        recognizer.setDefaultTaskHint(SFSpeechRecognitionTaskHint::Dictation);
        let active = Arc::new(AtomicBool::new(true));
        let sink = CaptureDelegate::alloc().set_ivars(CaptureSink {
            request: request.clone(),
            shared: shared.clone(),
            active: active.clone(),
        });
        let delegate: Retained<CaptureDelegate> = msg_send![super(sink), init];
        let queue = DispatchQueue::new("app.muxy.composer.audio", None);
        output.setSampleBufferDelegate_queue(
            Some(ProtocolObject::from_ref(&*delegate)),
            Some(&queue),
        );
        session.addInput(&input);
        session.addOutput(&output);
        let results = shared.clone();
        let accepting = active.clone();
        let transcript = Mutex::new(Transcript::default());
        let callback = RcBlock::new(
            move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                if !accepting.load(Ordering::Acquire) || results.cancelled.load(Ordering::Acquire) {
                    return;
                }
                let mut snapshot = results
                    .snapshot
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if let Some(result) = result.as_ref() {
                    snapshot.transcript = transcript
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .update(&result.bestTranscription().formattedString().to_string());
                    if result.isFinal() {
                        snapshot.phase = Phase::Finished;
                    }
                }
                if let Some(error) = error.as_ref() {
                    if results.finishing.load(Ordering::Acquire) {
                        snapshot.phase = Phase::Finished;
                    } else {
                        snapshot.phase = Phase::Failed;
                        snapshot.error = Some(error.localizedDescription().to_string());
                    }
                }
            },
        );
        let task = recognizer.recognitionTaskWithRequest_resultHandler(&request, &callback);
        Ok(Capture {
            session,
            output,
            input,
            request: Some(request),
            task,
            _delegate: delegate,
            queue,
            active,
        })
    }
}

fn record(language: &str, shared: &Arc<Shared>) -> Result<(), String> {
    if shared.cancelled.load(Ordering::Acquire) {
        return Ok(());
    }
    authorize(shared)?;
    if shared.cancelled.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut capture = capture(language, shared)?;
    let _device = DeviceObserver::new(shared.clone())?;
    unsafe {
        capture.session.startRunning();
    }
    if !unsafe { capture.session.isRunning() } {
        return Err(tr!("The microphone could not start recording").to_string());
    }
    let mut last = Instant::now();
    while !shared.finishing.load(Ordering::Acquire) {
        std::thread::sleep(Duration::from_millis(40));
        if shared.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        if shared.device_changed.load(Ordering::Acquire) {
            return Err(tr!(
                "The audio input changed. Finish or cancel dictation, then try again."
            )
            .to_string());
        }
        if !unsafe { capture.session.isRunning() } {
            return Err(tr!("Microphone recording stopped. Try again.").to_string());
        }
        let now = Instant::now();
        let mut snapshot = shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(snapshot.phase, Phase::Failed | Phase::Finished) {
            return Ok(());
        }
        if shared.paused.load(Ordering::Acquire) {
            snapshot.phase = Phase::Paused;
            snapshot.level = 0.0;
        } else {
            snapshot.phase = Phase::Recording;
            snapshot.elapsed += now.duration_since(last);
        }
        last = now;
    }
    capture.end_audio();
    finalize(shared, FINALIZATION);
    Ok(())
}

fn finalize(shared: &Shared, limit: Duration) {
    let deadline = Instant::now() + limit;
    while !shared.cancelled.load(Ordering::Acquire) {
        let mut snapshot = shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if matches!(snapshot.phase, Phase::Failed | Phase::Finished) {
            return;
        }
        snapshot.level = 0.0;
        if Instant::now() >= deadline {
            snapshot.phase = Phase::Finished;
            return;
        }
        drop(snapshot);
        std::thread::sleep(Duration::from_millis(40));
    }
}

struct DeviceObserver {
    address: AudioObjectPropertyAddress,
    queue: DispatchRetained<DispatchQueue>,
    listener: RcBlock<dyn Fn(u32, NonNull<AudioObjectPropertyAddress>)>,
}

impl DeviceObserver {
    fn new(shared: Arc<Shared>) -> Result<Self, String> {
        let listener = RcBlock::new(move |_: u32, _: NonNull<AudioObjectPropertyAddress>| {
            shared.device_changed.store(true, Ordering::Release);
        });
        let mut observer = Self {
            address: AudioObjectPropertyAddress {
                mSelector: kAudioHardwarePropertyDefaultInputDevice,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            },
            queue: DispatchQueue::new("app.muxy.composer.device", None),
            listener,
        };
        let status = unsafe {
            AudioObjectAddPropertyListenerBlock(
                1,
                NonNull::from(&mut observer.address),
                Some(&observer.queue),
                RcBlock::as_ptr(&observer.listener),
            )
        };
        if status != 0 {
            return Err(tr!("Could not observe microphone availability").to_string());
        }
        Ok(observer)
    }
}

impl Drop for DeviceObserver {
    fn drop(&mut self) {
        unsafe {
            AudioObjectRemovePropertyListenerBlock(
                1,
                NonNull::from(&mut self.address),
                Some(&self.queue),
                RcBlock::as_ptr(&self.listener),
            );
        }
    }
}

#[derive(Default)]
struct Transcript {
    committed: String,
    partial: String,
}

impl Transcript {
    fn update(&mut self, incoming: &str) -> String {
        let incoming = incoming.trim();
        if !self.partial.is_empty() && (incoming.is_empty() || self.should_commit_partial(incoming))
        {
            if !self.committed.is_empty() {
                self.committed.push(' ');
            }
            self.committed.push_str(&self.partial);
        }
        incoming.clone_into(&mut self.partial);
        [&self.committed, &self.partial]
            .into_iter()
            .filter(|part| !part.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn should_commit_partial(&self, incoming: &str) -> bool {
        if incoming.starts_with(&self.partial) {
            return false;
        }
        let partial = self.partial.to_lowercase();
        let incoming = incoming.to_lowercase();
        let words = |text: &str| {
            text.split(|character: char| !character.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let previous = words(&partial);
        let next = words(&incoming);
        if previous.len() <= 1 || next.is_empty() {
            return false;
        }
        if previous.first() != next.first() {
            return true;
        }
        previous.len() > 2 && next.len() > 1 && previous.get(1) != next.get(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{SimulatedRecorder, Snapshot};

    fn language(id: &str) -> Language {
        Language {
            id: id.into(),
            name: id.into(),
            preferred: false,
        }
    }

    #[test]
    fn linux_languages_select_matching_locales_without_changing_explicit_regions() {
        let mut available = ["de-DE", "en-US", "en-GB", "nb-NO", "he-IL", "jv-ID"].map(language);
        available[2].preferred = true;
        for (requested, expected) in [
            ("de", "de-DE"),
            ("en", "en-GB"),
            ("EN_us", "en-US"),
            ("no", "nb-NO"),
            ("he", "he-IL"),
            ("jw", "jv-ID"),
            ("", "en-GB"),
            ("unknown", "en-GB"),
        ] {
            assert_eq!(
                select_language(&available, requested).map(|selected| selected.id.as_str()),
                Some(expected)
            );
        }
    }

    fn waiting(phase: Phase) -> Shared {
        Shared {
            snapshot: Mutex::new(Snapshot {
                phase,
                transcript: "hello".into(),
                level: 0.5,
                error: (phase == Phase::Failed).then(|| "failed".into()),
                ..Snapshot::default()
            }),
            ..Shared::default()
        }
    }

    fn outcome(shared: &Shared) -> Snapshot {
        shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[test]
    fn finalization_ends_at_the_limit_and_keeps_the_transcript() {
        let shared = waiting(Phase::Recording);
        finalize(&shared, Duration::ZERO);
        let snapshot = outcome(&shared);
        assert_eq!(snapshot.phase, Phase::Finished);
        assert_eq!(snapshot.transcript, "hello");
        assert!(snapshot.level.abs() < f32::EPSILON);
    }

    #[test]
    fn finalization_preserves_failure_and_stops_when_cancelled() {
        let shared = waiting(Phase::Failed);
        finalize(&shared, Duration::ZERO);
        let snapshot = outcome(&shared);
        assert_eq!(snapshot.phase, Phase::Failed);
        assert_eq!(snapshot.error.as_deref(), Some("failed"));

        let shared = waiting(Phase::Recording);
        shared.cancelled.store(true, Ordering::Release);
        finalize(&shared, Duration::ZERO);
        assert_eq!(outcome(&shared).phase, Phase::Recording);
    }

    #[test]
    fn finished_transcript_is_available_only_after_a_requested_finish_completes() {
        let (recorder, simulated) = SimulatedRecorder::new(Snapshot {
            phase: Phase::Finished,
            transcript: " hello ".into(),
            ..Snapshot::default()
        });
        assert_eq!(recorder.finished(), None);
        recorder.finish();
        assert!(simulated.finishing());
        assert_eq!(recorder.finished().as_deref(), Some("hello"));
        simulated.update(Snapshot {
            phase: Phase::Recording,
            ..Snapshot::default()
        });
        assert_eq!(recorder.finished(), None);
    }

    #[test]
    fn recognition_corrections_replace_partial_words_without_duplicates() {
        for (partial, incoming) in [
            ("hello", "Hello world"),
            ("hello world", "Hello, world!"),
            ("weather", "Whether we go"),
            ("Let's try this", "let’s try this again"),
            ("hello world", "hello there"),
        ] {
            let mut transcript = Transcript::default();
            transcript.update(partial);
            assert_eq!(transcript.update(incoming), incoming);
        }
        let mut transcript = Transcript::default();
        transcript.update("one two three");
        assert_eq!(transcript.update("one more"), "one two three one more");
    }
}
