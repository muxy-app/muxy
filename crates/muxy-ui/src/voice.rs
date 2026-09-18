use std::ptr::NonNull;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

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
use objc2_foundation::{NSBundle, NSError, NSLocale, NSObject, NSObjectProtocol, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognitionTaskHint, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Language {
    pub id: String,
    pub name: String,
    pub preferred: bool,
}

pub fn languages() -> Vec<Language> {
    unsafe {
        let current = NSLocale::currentLocale();
        let current_id = current.localeIdentifier().to_string();
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
                preferred: id == current_id,
                id,
                name,
            });
        }
        languages.sort_by_key(|language| language.name.to_lowercase());
        languages
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    #[default]
    Starting,
    Recording,
    Paused,
    Finished,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub phase: Phase,
    pub transcript: String,
    pub elapsed: Duration,
    pub level: f32,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
struct Shared {
    snapshot: Mutex<Snapshot>,
    cancelled: AtomicBool,
    paused: AtomicBool,
    device_changed: AtomicBool,
}

#[derive(Debug)]
pub struct Recorder {
    shared: Arc<Shared>,
}

impl Recorder {
    pub fn start(language: String) -> Result<Self, String> {
        for key in [
            "NSMicrophoneUsageDescription",
            "NSSpeechRecognitionUsageDescription",
        ] {
            if NSBundle::mainBundle()
                .objectForInfoDictionaryKey(&NSString::from_str(key))
                .is_none()
            {
                return Err(
                    "Open the packaged Muxy app to enable microphone and speech permissions."
                        .into(),
                );
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
        Ok(Self { shared })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn pause(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Release);
    }

    pub fn finish(self) -> String {
        self.shared.cancelled.store(true, Ordering::Release);
        self.snapshot().transcript.trim().to_owned()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Debug)]
pub struct SimulatedRecorder(Arc<Shared>);

#[cfg(any(test, feature = "test-support"))]
impl SimulatedRecorder {
    pub fn new(snapshot: Snapshot) -> (Recorder, Self) {
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(snapshot),
            ..Shared::default()
        });
        (
            Recorder {
                shared: shared.clone(),
            },
            Self(shared),
        )
    }

    pub fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub fn paused(&self) -> bool {
        self.0.paused.load(Ordering::Acquire)
    }
    pub fn update(&self, snapshot: Snapshot) {
        *self
            .0
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = snapshot;
    }
}

fn authorize(shared: &Shared) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let audio = RcBlock::new(move |granted: Bool| {
        let _ = sender.send(granted.as_bool());
    });
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(
            AVMediaTypeAudio.ok_or("Audio capture is unavailable")?,
            &audio,
        );
    }
    if !permission(&receiver, shared)? {
        return Err("Microphone access is disabled. Enable Muxy in System Settings → Privacy & Security → Microphone.".into());
    }
    if shared.cancelled.load(Ordering::Acquire) {
        return Err("Dictation cancelled".into());
    }
    let (sender, receiver) = mpsc::channel();
    let speech = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = sender.send(status == SFSpeechRecognizerAuthorizationStatus::Authorized);
    });
    unsafe {
        SFSpeechRecognizer::requestAuthorization(&speech);
    }
    if !permission(&receiver, shared)? {
        return Err("Speech recognition access is disabled. Enable Muxy in System Settings → Privacy & Security → Speech Recognition.".into());
    }
    Ok(())
}

fn permission(receiver: &mpsc::Receiver<bool>, shared: &Shared) -> Result<bool, String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if shared.cancelled.load(Ordering::Acquire) {
            return Err("Dictation cancelled".into());
        }
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(granted) => return Ok(granted),
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
            Err(_) => return Err("Speech permission request did not complete. Try again.".into()),
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
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
    _delegate: Retained<CaptureDelegate>,
    queue: DispatchRetained<DispatchQueue>,
    active: Arc<AtomicBool>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        unsafe {
            self.output.setSampleBufferDelegate_queue(None, None);
        }
        self.queue.exec_sync(|| {});
        unsafe {
            self.request.endAudio();
            self.task.cancel();
            self.session.stopRunning();
            self.session.removeInput(&self.input);
            self.session.removeOutput(&self.output);
        }
    }
}

fn capture(language: &str, shared: &Arc<Shared>) -> Result<Capture, String> {
    unsafe {
        let available = languages();
        let selected = available.iter().find(|candidate| candidate.id.replace('_', "-").eq_ignore_ascii_case(&language.replace('_', "-")))
            .or_else(|| available.iter().find(|candidate| candidate.preferred))
            .or_else(|| available.iter().find(|candidate| candidate.id.replace('_', "-").eq_ignore_ascii_case("en-US")))
            .or_else(|| available.first())
            .ok_or("No on-device speech language is installed. Add one in System Settings → Keyboard → Dictation.")?;
        let locale = NSLocale::initWithLocaleIdentifier(
            NSLocale::alloc(),
            &NSString::from_str(&selected.id),
        );
        let recognizer = SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            .ok_or("This language is unavailable for speech recognition")?;
        if !recognizer.supportsOnDeviceRecognition() || !recognizer.isAvailable() {
            return Err("On-device speech recognition is unavailable for this language. Choose another dictation language.".into());
        }
        let device = AVCaptureDevice::defaultDeviceWithMediaType(
            AVMediaTypeAudio.ok_or("Audio capture is unavailable")?,
        )
        .ok_or("No microphone is available")?;
        let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
            .map_err(|error| error.to_string())?;
        let session = AVCaptureSession::new();
        let output = AVCaptureAudioDataOutput::new();
        if !session.canAddInput(&input) || !session.canAddOutput(&output) {
            return Err("Could not configure microphone capture".into());
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
                    snapshot.phase = Phase::Failed;
                    snapshot.error = Some(error.localizedDescription().to_string());
                }
            },
        );
        let task = recognizer.recognitionTaskWithRequest_resultHandler(&request, &callback);
        Ok(Capture {
            session,
            output,
            input,
            request,
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
    let capture = capture(language, shared)?;
    let _device = DeviceObserver::new(shared.clone())?;
    unsafe {
        capture.session.startRunning();
    }
    if !unsafe { capture.session.isRunning() } {
        return Err("The microphone could not start recording".into());
    }
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(40));
        if shared.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        if shared.device_changed.load(Ordering::Acquire) {
            return Err(
                "The audio input changed. Finish or cancel dictation, then try again.".into(),
            );
        }
        if !unsafe { capture.session.isRunning() } {
            return Err("Microphone recording stopped. Try again.".into());
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
            return Err("Could not observe microphone availability".into());
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
    #[test]
    fn partial_transcripts_replace_and_new_segments_append() {
        let mut transcript = Transcript::default();
        assert_eq!(transcript.update("hello"), "hello");
        assert_eq!(transcript.update("hello world"), "hello world");
        assert_eq!(transcript.update(""), "hello world");
        assert_eq!(transcript.update("next phrase"), "hello world next phrase");
        assert_eq!(
            transcript.update("next phrase revised"),
            "hello world next phrase revised"
        );
    }
    #[test]
    fn cancelling_permission_wait_and_finishing_do_not_require_audio() {
        let shared = Arc::new(Shared::default());
        let recorder = Recorder {
            shared: shared.clone(),
        };
        recorder.pause(true);
        assert!(shared.paused.load(Ordering::Acquire));
        recorder.pause(false);
        assert!(!shared.paused.load(Ordering::Acquire));
        shared
            .snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .transcript = "  retained words  ".into();
        assert_eq!(recorder.finish(), "retained words");
        let (_sender, receiver) = mpsc::channel();
        assert!(permission(&receiver, &shared).is_err());
        let shared = Arc::new(Shared::default());
        drop(Recorder {
            shared: shared.clone(),
        });
        assert!(shared.cancelled.load(Ordering::Acquire));
    }
}
