import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "TmuxCaptureService")

@MainActor
final class TmuxCaptureService {
    static let shared = TmuxCaptureService()

    private var streamingProcesses: [UUID: TmuxControlModeProcess] = [:]
    private var streamHandlers: [UUID: (Data) -> Void] = [:]

    private init() {}

    private static let socketName = "muxy"
    private static let sessionPrefix = "muxy-"

    private static let binarySearchPaths = [
        "/opt/homebrew/bin/tmux",
        "/usr/local/bin/tmux",
        "/opt/local/bin/tmux",
        "/usr/bin/tmux",
    ]

    static func findBinary() -> String? {
        binarySearchPaths.first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    static func sessionName(for paneID: UUID) -> String {
        "\(sessionPrefix)\(paneID.uuidString.prefix(8))"
    }

    func captureSnapshot(paneID: UUID) -> Data? {
        guard let tmux = Self.findBinary() else { return nil }
        let session = Self.sessionName(for: paneID)
        let socket = Self.socketName

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

        let cursorData = captureCursorPosition(tmux: tmux, socket: socket, session: session)
        return buildAnsiSnapshot(outputData: outputData, cursorData: cursorData)
    }

    private func captureCursorPosition(tmux: String, socket: String, session: String) -> (x: Int, y: Int)? {
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

    private func buildAnsiSnapshot(outputData: Data, cursorData: (x: Int, y: Int)?) -> Data {
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
        guard let tmux = Self.findBinary() else { return }

        streamHandlers[paneID] = handler

        let session = Self.sessionName(for: paneID)
        let socket = Self.socketName
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
        guard let tmux = Self.findBinary() else { return }
        let session = Self.sessionName(for: paneID)
        let socket = Self.socketName

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
        guard let tmux = Self.findBinary() else { return }
        let session = Self.sessionName(for: paneID)
        let socket = Self.socketName

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
        guard let tmux = Self.findBinary() else { return }
        let session = Self.sessionName(for: paneID)
        let socket = Self.socketName

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
}

private final class TmuxControlModeProcess: Sendable {
    private let tmuxBinary: String
    private let socket: String
    private let session: String
    private let handler: @Sendable (Data) -> Void
    nonisolated(unsafe) private var process: Process?
    nonisolated(unsafe) private var outputPipe: Pipe?
    nonisolated(unsafe) private var isRunning = false

    init(tmuxBinary: String, socket: String, session: String, handler: @escaping @Sendable (Data) -> Void) {
        self.tmuxBinary = tmuxBinary
        self.socket = socket
        self.session = session
        self.handler = handler
    }

    func start() {
        guard !isRunning else { return }

        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: tmuxBinary)
        process.arguments = ["-L", socket, "-C", "attach-session", "-t", session]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice
        process.standardInput = FileHandle.nullDevice

        self.process = process
        outputPipe = pipe
        isRunning = true

        let capturedHandler = handler
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                self?.stop()
                return
            }
            Self.parseControlOutput(data, handler: capturedHandler)
        }

        DispatchQueue.global(qos: .userInteractive).async { [weak self] in
            do {
                try process.run()
                process.waitUntilExit()
                DispatchQueue.main.async {
                    self?.isRunning = false
                }
            } catch {
                logger.error("tmux control mode failed to start: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self?.isRunning = false
                }
            }
        }
    }

    func stop() {
        isRunning = false
        outputPipe?.fileHandleForReading.readabilityHandler = nil
        outputPipe = nil
        if let process, process.isRunning {
            process.terminate()
        }
        self.process = nil
    }

    private static func parseControlOutput(_ data: Data, handler: @Sendable @escaping (Data) -> Void) {
        guard let output = String(data: data, encoding: .utf8) else { return }

        for line in output.split(separator: "\n") {
            let lineStr = String(line)
            guard lineStr.hasPrefix("%output") else { continue }

            let withoutPrefix = lineStr.dropFirst("%output ".count)
            let parts = withoutPrefix.split(separator: " ", maxSplits: 1)
            guard parts.count == 2 else { continue }

            let encodedPayload = String(parts[1])
            guard let decoded = Data(base64Encoded: encodedPayload) else { continue }

            DispatchQueue.main.async {
                handler(decoded)
            }
        }
    }
}
