import AVFoundation
import CoreAudio
import Foundation
import os
import Speech

private let logger = Logger(subsystem: "app.muxy", category: "VoiceRecorder")

enum VoiceRecorderError: Error {
    case recognizerUnavailable
    case engineFailure(String)
}

struct TranscriptMergeResult {
    let committed: String
    let partial: String
    let transcript: String
}

@MainActor
@Observable
final class VoiceRecorder {
    private(set) var isRecording = false
    private(set) var isPaused = false
    private(set) var elapsed: TimeInterval = 0
    private(set) var level: Float = 0
    private(set) var transcript: String = ""
    var onFailure: (@MainActor (String) -> Void)?

    @ObservationIgnored private let captureSession = AVCaptureSession()
    @ObservationIgnored private let captureQueue = DispatchQueue(label: "app.muxy.voice-recorder.capture")
    @ObservationIgnored private var recognizer: SFSpeechRecognizer?
    @ObservationIgnored private var request: SFSpeechAudioBufferRecognitionRequest?
    @ObservationIgnored private var task: SFSpeechRecognitionTask?
    @ObservationIgnored private var startedAt: Date?
    @ObservationIgnored private var accumulatedBeforePause: TimeInterval = 0
    @ObservationIgnored private var elapsedTimer: Timer?
    @ObservationIgnored private var levelSink: LevelSink?
    @ObservationIgnored private var transcriptSink: TranscriptSink?
    @ObservationIgnored private var inputDeviceObserver: AudioInputDeviceObserver?
    @ObservationIgnored private var captureSink: AudioCaptureSink?
    @ObservationIgnored private var committedTranscript = ""
    @ObservationIgnored private var currentPartialTranscript = ""

    func start(locale: Locale) throws {
        guard let recognizer = SFSpeechRecognizer(locale: locale),
              recognizer.isAvailable
        else {
            throw VoiceRecorderError.recognizerUnavailable
        }
        guard recognizer.supportsOnDeviceRecognition else {
            throw VoiceRecorderError.engineFailure(
                "On-device speech recognition is unavailable for this language. Open Settings → Recording to pick another."
            )
        }
        self.recognizer = recognizer
        recognizer.defaultTaskHint = .dictation

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        request.requiresOnDeviceRecognition = true
        self.request = request
        let levelSink = LevelSink { [weak self] normalized in
            guard let self else { return }
            self.level = normalized
        }
        self.levelSink = levelSink
        let captureSink = AudioCaptureSink(request: request, sink: levelSink)
        self.captureSink = captureSink
        do {
            try configureCaptureSession(sink: captureSink)
        } catch {
            teardown()
            throw error
        }
        observeInputDeviceChanges()
        let transcriptSink = TranscriptSink { [weak self] text in
            guard let self else { return }
            self.updateTranscript(with: text)
        }
        self.transcriptSink = transcriptSink
        task = Self.startRecognitionTaskNonisolated(
            recognizer: recognizer,
            request: request,
            sink: transcriptSink
        )

        captureQueue.sync {
            captureSession.startRunning()
        }
        guard captureSession.isRunning else {
            teardown()
            throw VoiceRecorderError.engineFailure("Microphone capture could not start.")
        }

        startedAt = Date()
        accumulatedBeforePause = 0
        elapsed = 0
        level = 0
        committedTranscript = ""
        currentPartialTranscript = ""
        transcript = ""
        isRecording = true
        isPaused = false
        startElapsedTimer()
    }

    func pause() {
        guard isRecording, !isPaused else { return }
        captureSink?.setAcceptsBuffers(false)
        if let startedAt {
            accumulatedBeforePause += Date().timeIntervalSince(startedAt)
        }
        startedAt = nil
        isPaused = true
        level = 0
        stopElapsedTimer()
    }

    func resume() {
        guard isRecording, isPaused else { return }
        captureSink?.setAcceptsBuffers(true)
        startedAt = Date()
        isPaused = false
        startElapsedTimer()
    }

    func finish() -> String {
        let final = transcript
        teardown()
        return final
    }

    func cancel() {
        teardown()
    }

    nonisolated static func requestPermissions() async -> Bool {
        let mic = await withCheckedContinuation { continuation in
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                continuation.resume(returning: granted)
            }
        }
        guard mic else { return false }
        return await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { status in
                continuation.resume(returning: status == .authorized)
            }
        }
    }

    nonisolated static func currentPermissionStatus() -> Bool {
        let mic = AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
        let speech = SFSpeechRecognizer.authorizationStatus() == .authorized
        return mic && speech
    }

    private func teardown() {
        stopElapsedTimer()
        removeInputDeviceObserver()
        captureSink?.setAcceptsBuffers(false)
        captureQueue.sync {
            if captureSession.isRunning {
                captureSession.stopRunning()
            }
        }
        captureSession.beginConfiguration()
        captureSession.inputs.forEach { captureSession.removeInput($0) }
        captureSession.outputs.forEach { captureSession.removeOutput($0) }
        captureSession.commitConfiguration()
        request?.endAudio()
        task?.cancel()
        levelSink?.detach()
        levelSink = nil
        transcriptSink?.detach()
        transcriptSink = nil
        captureSink = nil
        request = nil
        task = nil
        recognizer = nil
        startedAt = nil
        accumulatedBeforePause = 0
        isRecording = false
        isPaused = false
        elapsed = 0
        level = 0
    }

    private func observeInputDeviceChanges() {
        removeInputDeviceObserver()
        let observer = AudioInputDeviceObserver { [weak self] in
            Task { @MainActor in
                self?.handleInputDeviceChange()
            }
        }
        observer.start()
        inputDeviceObserver = observer
    }

    private func removeInputDeviceObserver() {
        inputDeviceObserver?.stop()
        inputDeviceObserver = nil
    }

    private func handleInputDeviceChange() {
        guard isRecording else { return }
        logger.error("Audio input device changed during recording")
        teardown()
        onFailure?("Microphone changed. Start recording again to use the new input device.")
    }

    private func configureCaptureSession(sink: AudioCaptureSink) throws {
        captureSession.beginConfiguration()
        defer { captureSession.commitConfiguration() }
        captureSession.inputs.forEach { captureSession.removeInput($0) }
        captureSession.outputs.forEach { captureSession.removeOutput($0) }
        guard let device = AVCaptureDevice.default(for: .audio) else {
            throw VoiceRecorderError.engineFailure("No microphone input is available.")
        }
        let input = try AVCaptureDeviceInput(device: device)
        guard captureSession.canAddInput(input) else {
            throw VoiceRecorderError.engineFailure("The selected microphone cannot be used.")
        }
        let output = AVCaptureAudioDataOutput()
        output.setSampleBufferDelegate(sink, queue: captureQueue)
        guard captureSession.canAddOutput(output) else {
            throw VoiceRecorderError.engineFailure("Microphone audio output cannot be used.")
        }
        captureSession.addInput(input)
        captureSession.addOutput(output)
    }

    private func startElapsedTimer() {
        stopElapsedTimer()
        let timer = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        RunLoop.main.add(timer, forMode: .common)
        elapsedTimer = timer
    }

    private func stopElapsedTimer() {
        elapsedTimer?.invalidate()
        elapsedTimer = nil
    }

    private func tick() {
        guard let startedAt else { return }
        elapsed = accumulatedBeforePause + Date().timeIntervalSince(startedAt)
    }

    private func updateTranscript(with text: String) {
        let updated = Self.mergeTranscript(
            committed: committedTranscript,
            partial: currentPartialTranscript,
            incoming: text
        )
        committedTranscript = updated.committed
        currentPartialTranscript = updated.partial
        transcript = updated.transcript
    }

    nonisolated static func mergeTranscript(committed: String, partial: String, incoming: String) -> TranscriptMergeResult {
        let cleanedIncoming = incoming.trimmingCharacters(in: .whitespacesAndNewlines)
        var updatedCommitted = committed
        var updatedPartial = partial

        if cleanedIncoming.isEmpty {
            updatedCommitted = joinedTranscript(committed, partial)
            updatedPartial = ""
        } else if shouldCommitPartial(partial, before: cleanedIncoming) {
            updatedCommitted = joinedTranscript(committed, partial)
            updatedPartial = cleanedIncoming
        } else {
            updatedPartial = cleanedIncoming
        }

        return TranscriptMergeResult(
            committed: updatedCommitted,
            partial: updatedPartial,
            transcript: joinedTranscript(updatedCommitted, updatedPartial)
        )
    }

    nonisolated private static func joinedTranscript(_ leading: String, _ trailing: String) -> String {
        let cleanedLeading = leading.trimmingCharacters(in: .whitespacesAndNewlines)
        let cleanedTrailing = trailing.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleanedLeading.isEmpty else { return cleanedTrailing }
        guard !cleanedTrailing.isEmpty else { return cleanedLeading }
        return "\(cleanedLeading) \(cleanedTrailing)"
    }

    nonisolated private static func shouldCommitPartial(_ partial: String, before incoming: String) -> Bool {
        guard !partial.isEmpty, !incoming.hasPrefix(partial) else { return false }
        let partialWords = words(in: partial)
        let incomingWords = words(in: incoming)
        guard partialWords.count > 1, !incomingWords.isEmpty else { return false }
        guard partialWords.first == incomingWords.first else { return true }
        guard partialWords.count > 2, incomingWords.count > 1 else { return false }
        return partialWords[1] != incomingWords[1]
    }

    nonisolated private static func words(in text: String) -> [String] {
        text.lowercased()
            .components(separatedBy: CharacterSet.alphanumerics.inverted)
            .filter { !$0.isEmpty }
    }

    nonisolated static func averagePower(in samples: UnsafeBufferPointer<Float>) -> Float {
        guard !samples.isEmpty else { return -160 }
        var sum: Float = 0
        for sample in samples {
            sum += sample * sample
        }
        let rms = sqrt(sum / Float(samples.count))
        guard rms > 0 else { return -160 }
        return 20 * log10(rms)
    }

    nonisolated static func startRecognitionTaskNonisolated(
        recognizer: SFSpeechRecognizer,
        request: SFSpeechAudioBufferRecognitionRequest,
        sink: TranscriptSink
    ) -> SFSpeechRecognitionTask {
        let handler: @Sendable (SFSpeechRecognitionResult?, Error?) -> Void = { result, _ in
            guard let result else { return }
            sink.publish(result.bestTranscription.formattedString)
        }
        return recognizer.recognitionTask(with: request, resultHandler: handler)
    }

    nonisolated static func normalize(power db: Float) -> Float {
        let floor: Float = -50
        guard db.isFinite else { return 0 }
        let clamped = max(min(db, 0), floor)
        return (clamped - floor) / -floor
    }
}

final class AudioCaptureSink: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate, @unchecked Sendable {
    private let lock = NSLock()
    private let request: SFSpeechAudioBufferRecognitionRequest
    private let sink: LevelSink
    private var acceptsBuffers = true

    init(request: SFSpeechAudioBufferRecognitionRequest, sink: LevelSink) {
        self.request = request
        self.sink = sink
    }

    func setAcceptsBuffers(_ acceptsBuffers: Bool) {
        lock.lock()
        self.acceptsBuffers = acceptsBuffers
        lock.unlock()
    }

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        lock.lock()
        let acceptsBuffers = acceptsBuffers
        lock.unlock()
        guard acceptsBuffers else { return }
        request.appendAudioSampleBuffer(sampleBuffer)
        sink.publish(VoiceRecorder.normalize(power: Self.averagePower(in: sampleBuffer)))
    }

    private static func averagePower(in sampleBuffer: CMSampleBuffer) -> Float {
        guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer),
              let streamDescription = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription),
              streamDescription.pointee.mFormatID == kAudioFormatLinearPCM
        else {
            return -160
        }
        var audioBufferList = AudioBufferList()
        var blockBuffer: CMBlockBuffer?
        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: nil,
            bufferListOut: &audioBufferList,
            bufferListSize: MemoryLayout<AudioBufferList>.size,
            blockBufferAllocator: kCFAllocatorDefault,
            blockBufferMemoryAllocator: kCFAllocatorDefault,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            blockBufferOut: &blockBuffer
        )
        guard status == noErr, let data = audioBufferList.mBuffers.mData else { return -160 }
        let byteCount = Int(audioBufferList.mBuffers.mDataByteSize)
        let flags = streamDescription.pointee.mFormatFlags
        if flags & kAudioFormatFlagIsFloat != 0, streamDescription.pointee.mBitsPerChannel == 32 {
            let samples = data.assumingMemoryBound(to: Float.self)
            return VoiceRecorder.averagePower(in: UnsafeBufferPointer(start: samples, count: byteCount / MemoryLayout<Float>.stride))
        }
        if streamDescription.pointee.mBitsPerChannel == 16 {
            let samples = data.assumingMemoryBound(to: Int16.self)
            let count = byteCount / MemoryLayout<Int16>.stride
            var floatSamples = [Float]()
            floatSamples.reserveCapacity(count)
            for index in 0 ..< count {
                floatSamples.append(Float(samples[index]) / Float(Int16.max))
            }
            return floatSamples.withUnsafeBufferPointer { VoiceRecorder.averagePower(in: $0) }
        }
        return -160
    }
}

final class AudioInputDeviceObserver: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.muxy.voice-recorder.audio-device")
    private var address = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain
    )
    private let listener: AudioObjectPropertyListenerBlock
    private var isObserving = false

    init(handler: @escaping @Sendable () -> Void) {
        listener = { _, _ in handler() }
    }

    func start() {
        guard !isObserving else { return }
        let status = AudioObjectAddPropertyListenerBlock(
            AudioObjectID(kAudioObjectSystemObject),
            &address,
            queue,
            listener
        )
        isObserving = status == noErr
    }

    func stop() {
        guard isObserving else { return }
        AudioObjectRemovePropertyListenerBlock(
            AudioObjectID(kAudioObjectSystemObject),
            &address,
            queue,
            listener
        )
        isObserving = false
    }
}

final class TranscriptSink: @unchecked Sendable {
    private let lock = NSLock()
    private var handler: (@MainActor (String) -> Void)?

    init(handler: @escaping @MainActor (String) -> Void) {
        self.handler = handler
    }

    func publish(_ value: String) {
        lock.lock()
        let current = handler
        lock.unlock()
        guard let current else { return }
        Task { @MainActor in
            current(value)
        }
    }

    func detach() {
        lock.lock()
        handler = nil
        lock.unlock()
    }
}

final class LevelSink: @unchecked Sendable {
    private static let minInterval: TimeInterval = 1.0 / 15.0

    private let lock = NSLock()
    private var handler: (@MainActor (Float) -> Void)?
    private var lastPublishedAt: TimeInterval = 0

    init(handler: @escaping @MainActor (Float) -> Void) {
        self.handler = handler
    }

    func publish(_ value: Float) {
        let now = ProcessInfo.processInfo.systemUptime
        lock.lock()
        guard let current = handler, now - lastPublishedAt >= Self.minInterval else {
            lock.unlock()
            return
        }
        lastPublishedAt = now
        lock.unlock()
        Task { @MainActor in
            current(value)
        }
    }

    func detach() {
        lock.lock()
        handler = nil
        lock.unlock()
    }
}
