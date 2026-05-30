import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TmuxCaptureService")

@MainActor
final class TmuxCaptureService {
    static let shared = TmuxCaptureService()

    private var streamingProcesses: [UUID: TmuxControlModeProcess] = [:]
    private var streamHandlers: [UUID: (Data) -> Void] = [:]
    private static let maxConcurrentStreams = 32

    private init() {}

    func captureSnapshot(paneID: UUID) async -> Data? {
        guard let tmux = TmuxConfiguration.findBinary() else { return nil }
        let session = TmuxConfiguration.sessionName(for: paneID)
        let socket = TmuxConfiguration.socketName

        return await Task.detached {
            let captureProcess = Process()
            let pipe = Pipe()
            captureProcess.executableURL = URL(fileURLWithPath: tmux)
            captureProcess.arguments = [
                "-L", socket,
                "capture-pane",
                "-t", session,
                "-p", "-e",
                "-S", "-5000", "-E", "50",
            ]
            captureProcess.standardOutput = pipe
            captureProcess.standardError = FileHandle.nullDevice

            do {
                try captureProcess.run()
                captureProcess.waitUntilExit()
            } catch {
                logger.error("tmux capture-pane failed: \(error.localizedDescription)")
                return nil
            }

            guard captureProcess.terminationStatus == 0 else {
                logger.warning("tmux capture-pane exited with status \(captureProcess.terminationStatus)")
                return nil
            }

            let outputData = pipe.fileHandleForReading.readDataToEndOfFile()
            guard !outputData.isEmpty else { return nil }

            let cursorData = Self.captureCursorPosition(session: session, socket: socket)
            return Self.buildAnsiSnapshot(outputData: outputData, cursorData: cursorData)
        }.value
    }

    nonisolated private static func captureCursorPosition(session: String, socket: String) -> (x: Int, y: Int)? {
        guard let tmux = TmuxConfiguration.findBinary() else { return nil }
        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: tmux)
        process.arguments = [
            "-L", socket,
            "display-message",
            "-t", session,
            "-p", "#{cursor_x} #{cursor_y}",
        ]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }

        guard process.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        guard let output = String(data: data, encoding: .utf8) else { return nil }

        let parts = output.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: " ")
        guard parts.count == 2,
              let x = Int(parts[0]),
              let y = Int(parts[1])
        else { return nil }
        return (x, y)
    }

    nonisolated private static func buildAnsiSnapshot(outputData: Data, cursorData: (x: Int, y: Int)?) -> Data {
        var result = outputData

        if let cursor = cursorData {
            let cursorSeq = "\u{1B}[\(cursor.y + 1);\(cursor.x + 1)H"
            if let cursorData = cursorSeq.data(using: .utf8) {
                result.append(cursorData)
            }
        }

        return result
    }

    func startStreaming(paneID: UUID, handler: @escaping (Data) -> Void) {
        guard streamingProcesses[paneID] == nil else { return }
        guard let tmux = TmuxConfiguration.findBinary() else { return }
        guard streamingProcesses.count < Self.maxConcurrentStreams else {
            logger.warning("Streaming limit reached (\(Self.maxConcurrentStreams)), cannot stream pane \(paneID)")
            return
        }

        streamHandlers[paneID] = handler

        let session = TmuxConfiguration.sessionName(for: paneID)
        let socket = TmuxConfiguration.socketName
        let controlProcess = TmuxControlModeProcess(
            tmuxBinary: tmux,
            socket: socket,
            session: session
        ) { [weak self] outputData in
            MainActor.assumeIsolated {
                self?.handleControlOutput(paneID: paneID, data: outputData)
            }
        }
        streamingProcesses[paneID] = controlProcess
        controlProcess.start()
    }

    func stopStreaming(paneID: UUID) {
        streamingProcesses[paneID]?.stop()
        streamingProcesses.removeValue(forKey: paneID)
        streamHandlers.removeValue(forKey: paneID)
    }

    private func handleControlOutput(paneID: UUID, data: Data) {
        guard let handler = streamHandlers[paneID] else { return }
        handler(data)
    }

    func sendInput(paneID: UUID, bytes: Data) {
        guard let tmux = TmuxConfiguration.findBinary() else { return }
        let session = TmuxConfiguration.sessionName(for: paneID)
        let socket = TmuxConfiguration.socketName

        guard let text = String(data: bytes, encoding: .utf8) ?? String(data: bytes, encoding: .ascii) else {
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: tmux)
        process.arguments = [
            "-L", socket,
            "send-keys",
            "-t", session,
            "-l", text,
        ]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        DispatchQueue.global(qos: .userInteractive).async {
            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                logger.error("tmux send-keys failed: \(error.localizedDescription)")
            }
        }
    }

    func resizeSession(paneID: UUID, cols: UInt32, rows: UInt32) {
        guard let tmux = TmuxConfiguration.findBinary() else { return }
        let session = TmuxConfiguration.sessionName(for: paneID)
        let socket = TmuxConfiguration.socketName

        let process = Process()
        process.executableURL = URL(fileURLWithPath: tmux)
        process.arguments = [
            "-L", socket,
            "resize-window",
            "-t", session,
            "-x", "\(cols)",
            "-y", "\(rows)",
        ]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        DispatchQueue.global(qos: .utility).async {
            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                logger.error("tmux resize-window failed: \(error.localizedDescription)")
            }
        }
    }

    func scroll(paneID: UUID, deltaY: Double) {
        guard let tmux = TmuxConfiguration.findBinary() else { return }
        let session = TmuxConfiguration.sessionName(for: paneID)
        let socket = TmuxConfiguration.socketName

        let key = deltaY > 0 ? "Up" : "Down"
        let lines = min(max(1, Int(abs(deltaY))), 20)

        let process = Process()
        process.executableURL = URL(fileURLWithPath: tmux)
        process.arguments = [
            "-L", socket,
            "send-keys",
            "-t", session,
            "-N", "\(lines)",
            key,
        ]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        DispatchQueue.global(qos: .utility).async {
            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                logger.error("tmux scroll failed: \(error.localizedDescription)")
            }
        }
    }

    nonisolated static func parseControlOutput(_ data: Data) -> [Data] {
        guard let output = String(data: data, encoding: .utf8) else { return [] }
        var results: [Data] = []

        for line in output.split(separator: "\n") {
            let lineStr = String(line)
            guard lineStr.hasPrefix("%output") else { continue }

            let withoutPrefix = lineStr.dropFirst("%output ".count)
            let parts = withoutPrefix.split(separator: " ", maxSplits: 1)
            guard parts.count == 2 else { continue }

            let payload = String(parts[1])
            results.append(decodeOctalEscapes(payload))
        }

        return results
    }

    nonisolated private static func decodeOctalEscapes(_ input: String) -> Data {
        var result = Data()
        var chars = input[...]

        while !chars.isEmpty {
            if chars.first == "\\" {
                let afterBackslash = chars.dropFirst()
                guard afterBackslash.count >= 3 else {
                    result.append(Data(afterBackslash.utf8))
                    break
                }
                let octalChars = afterBackslash.prefix(3)
                if let byte = UInt8(octalChars, radix: 8) {
                    result.append(byte)
                    chars = afterBackslash.dropFirst(3)
                } else {
                    result.append(0x5C)
                    chars = afterBackslash
                }
            } else {
                let char = chars.first!
                result.append(Data(String(char).utf8))
                chars = chars.dropFirst()
            }
        }

        return result
    }
}

private final class TmuxControlModeProcess: @unchecked Sendable {
    private let tmuxBinary: String
    private let socket: String
    private let session: String
    private let handler: @Sendable (Data) -> Void
    private let lock = NSLock()
    private var _process: Process?
    private var _outputPipe: Pipe?
    private var _inputPipe: Pipe?
    private var _isRunning = false

    init(tmuxBinary: String, socket: String, session: String, handler: @escaping @Sendable (Data) -> Void) {
        self.tmuxBinary = tmuxBinary
        self.socket = socket
        self.session = session
        self.handler = handler
    }

    deinit {
        stop()
    }

    func start() {
        lock.lock()
        defer { lock.unlock() }
        guard !_isRunning else { return }

        let process = Process()
        let pipe = Pipe()
        let inputPipe = Pipe()
        process.executableURL = URL(fileURLWithPath: tmuxBinary)
        process.arguments = ["-L", socket, "-C", "attach-session", "-t", session]
        process.standardInput = inputPipe
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        _process = process
        _outputPipe = pipe
        _inputPipe = inputPipe
        _isRunning = true

        let capturedHandler = handler
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                self?.performStop()
                return
            }
            for decoded in TmuxCaptureService.parseControlOutput(data) {
                DispatchQueue.main.async {
                    capturedHandler(decoded)
                }
            }
        }

        DispatchQueue.global(qos: .userInteractive).async { [weak self] in
            do {
                try process.run()
                process.waitUntilExit()
                DispatchQueue.main.async {
                    self?.clearRunningFlag()
                }
            } catch {
                logger.error("tmux control mode failed to start: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self?.clearRunningFlag()
                }
            }
        }
    }

    func stop() {
        lock.lock()
        let pipe = _outputPipe
        let inputPipe = _inputPipe
        let proc = _process
        _outputPipe = nil
        _inputPipe = nil
        _process = nil
        _isRunning = false
        lock.unlock()

        pipe?.fileHandleForReading.readabilityHandler = nil
        inputPipe?.fileHandleForWriting.closeFile()
        if let proc, proc.isRunning {
            proc.terminate()
        }
    }

    private func performStop() {
        stop()
    }

    private func clearRunningFlag() {
        lock.lock()
        _isRunning = false
        lock.unlock()
    }

    func send(_ command: String) {
        lock.lock()
        let pipe = _inputPipe
        lock.unlock()

        guard let pipe else { return }
        guard let data = (command + "\n").data(using: .utf8) else { return }
        pipe.fileHandleForWriting.write(data)
    }
}
