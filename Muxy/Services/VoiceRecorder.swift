import AVFoundation
import Foundation
import os
import Speech

private let logger = Logger(subsystem: "app.muxy", category: "VoiceRecorder")

enum VoiceRecorderError: Error {
    case recognizerUnavailable
    case engineFailure(String)
}

@MainActor
@Observable
final class VoiceRecorder {
    private(set) var isRecording = false
    private(set) var isPaused = false
    private(set) var elapsed: TimeInterval = 0
    private(set) var level: Float = 0
    private(set) var transcript: String = ""

    @ObservationIgnored private let engine = AVAudioEngine()
    @ObservationIgnored private var recognizer: SFSpeechRecognizer?
    @ObservationIgnored private var request: SFSpeechAudioBufferRecognitionRequest?
    @ObservationIgnored private var task: SFSpeechRecognitionTask?
    @ObservationIgnored private var startedAt: Date?
    @ObservationIgnored private var accumulatedBeforePause: TimeInterval = 0
    @ObservationIgnored private var elapsedTimer: Timer?
    @ObservationIgnored private var levelSink: LevelSink?
    @ObservationIgnored private var transcriptSink: TranscriptSink?

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
        Self.installTapNonisolated(on: engine.inputNode, request: request, sink: levelSink)

        engine.prepare()
        do {
            try engine.start()
        } catch {
            engine.inputNode.removeTap(onBus: 0)
            levelSink.detach()
            self.levelSink = nil
            self.request = nil
            self.recognizer = nil
            throw VoiceRecorderError.engineFailure(error.localizedDescription)
        }

        let transcriptSink = TranscriptSink { [weak self] text in
            guard let self else { return }
            self.transcript = text
        }
        self.transcriptSink = transcriptSink
        task = Self.startRecognitionTaskNonisolated(
            recognizer: recognizer,
            request: request,
            sink: transcriptSink
        )

        startedAt = Date()
        accumulatedBeforePause = 0
        elapsed = 0
        level = 0
        transcript = ""
        isRecording = true
        isPaused = false
        startElapsedTimer()
    }

    func pause() {
        guard isRecording, !isPaused else { return }
        engine.pause()
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
        do {
            try engine.start()
        } catch {
            logger.error("Failed to resume engine: \(error.localizedDescription)")
            return
        }
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
        engine.inputNode.removeTap(onBus: 0)
        if engine.isRunning {
            engine.stop()
        }
        request?.endAudio()
        task?.cancel()
        levelSink?.detach()
        levelSink = nil
        transcriptSink?.detach()
        transcriptSink = nil
        request = nil
        task = nil
        recognizer = nil
        startedAt = nil
        accumulatedBeforePause = 0
        isRecording = false
        isPaused = false
        level = 0
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

    nonisolated static func averagePower(in buffer: AVAudioPCMBuffer) -> Float {
        guard let channel = buffer.floatChannelData?[0] else { return -160 }
        let frames = Int(buffer.frameLength)
        guard frames > 0 else { return -160 }
        var sum: Float = 0
        for i in 0 ..< frames {
            let sample = channel[i]
            sum += sample * sample
        }
        let rms = sqrt(sum / Float(frames))
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

    nonisolated static func installTapNonisolated(
        on inputNode: AVAudioInputNode,
        request: SFSpeechAudioBufferRecognitionRequest,
        sink: LevelSink
    ) {
        let format = inputNode.outputFormat(forBus: 0)
        inputNode.removeTap(onBus: 0)
        let requestBox = UncheckedBox(request)
        let block: @Sendable (AVAudioPCMBuffer, AVAudioTime) -> Void = { buffer, _ in
            requestBox.value.append(buffer)
            let normalized = normalize(power: averagePower(in: buffer))
            sink.publish(normalized)
        }
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format, block: block)
    }

    nonisolated static func normalize(power db: Float) -> Float {
        let floor: Float = -50
        guard db.isFinite else { return 0 }
        let clamped = max(min(db, 0), floor)
        return (clamped - floor) / -floor
    }
}

struct UncheckedBox<T>: @unchecked Sendable {
    let value: T
    init(_ value: T) {
        self.value = value
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
