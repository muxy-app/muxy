import Foundation
import MuxyServer
import Network
import os

private let logger = Logger(subsystem: "app.muxy", category: "MobileServerService")

@MainActor
@Observable
final class MobileServerService {
    static let shared = MobileServerService()

    static let defaultPort: UInt16 = AppEnvironment.isDevelopment
        ? MuxyRemoteServer.defaultPort + 1
        : MuxyRemoteServer.defaultPort
    static let minPort: UInt16 = 1024
    static let maxPort: UInt16 = 65535

    private static var enabledKey: String {
        AppEnvironment.isDevelopment
            ? "app.muxy.mobile.serverEnabled.dev"
            : "app.muxy.mobile.serverEnabled"
    }

    private static var portKey: String {
        AppEnvironment.isDevelopment
            ? "app.muxy.mobile.serverPort.dev"
            : "app.muxy.mobile.serverPort"
    }

    private(set) var isEnabled: Bool {
        didSet {
            UserDefaults.standard.set(isEnabled, forKey: Self.enabledKey)
        }
    }

    var port: UInt16 {
        didSet {
            guard port != oldValue else { return }
            UserDefaults.standard.set(Int(port), forKey: Self.portKey)
            if isEnabled {
                setEnabled(false)
            }
            lastError = nil
        }
    }

    private(set) var lastError: String?
    private(set) var isPortInUse = false

    private var server: MuxyRemoteServer?
    private var delegate: MuxyRemoteServerDelegate?
    private var delegateBuilder: ((MuxyRemoteServer) -> MuxyRemoteServerDelegate)?
    private var pendingServers: [MuxyRemoteServer] = []

    private init() {
        if AppEnvironment.isDevelopment {
            isEnabled = true
            port = Self.defaultPort
        } else {
            isEnabled = UserDefaults.standard.bool(forKey: Self.enabledKey)
            let storedPort = UserDefaults.standard.object(forKey: Self.portKey) as? Int
            if let storedPort, let value = UInt16(exactly: storedPort), Self.isValid(port: value) {
                port = value
            } else {
                port = Self.defaultPort
            }
        }
        ApprovedDevicesStore.shared.onRevoke = { [weak self] deviceID in
            self?.server?.disconnect(deviceID: deviceID)
        }
    }

    func configure(_ delegateBuilder: @escaping (MuxyRemoteServer) -> MuxyRemoteServerDelegate) {
        self.delegateBuilder = delegateBuilder
        if isEnabled {
            start()
        }
    }

    func setEnabled(_ enabled: Bool) {
        if enabled, isEnabled, server != nil { return }
        if !enabled, !isEnabled { return }
        isEnabled = enabled
        if enabled {
            start()
        } else {
            retireCurrentServer()
            lastError = nil
            isPortInUse = false
        }
    }

    func stop() {
        setEnabled(false)
    }

    func stopForTermination() {
        retireCurrentServer()
    }

    static func isValid(port: UInt16) -> Bool {
        port >= minPort && port <= maxPort
    }

    private func retireCurrentServer() {
        guard let current = server else { return }
        server = nil
        delegate = nil
        retire(current)
    }

    private func retire(_ server: MuxyRemoteServer) {
        pendingServers.append(server)
        server.stop { [weak self, weak server] in
            Task { @MainActor in
                guard let self, let server else { return }
                self.pendingServers.removeAll { $0 === server }
                self.launchIfReady()
            }
        }
    }

    private func start() {
        retireCurrentServer()
        launchIfReady()
    }

    private func launchIfReady() {
        guard isEnabled, server == nil, pendingServers.isEmpty, let delegateBuilder else { return }
        launchServer(port: port, delegateBuilder: delegateBuilder)
    }

    private func launchServer(port: UInt16, delegateBuilder: (MuxyRemoteServer) -> MuxyRemoteServerDelegate) {
        let newServer = MuxyRemoteServer(port: port)
        let newDelegate = delegateBuilder(newServer)
        newServer.delegate = newDelegate
        server = newServer
        delegate = newDelegate
        newServer.start { [weak self, weak newServer] result in
            Task { @MainActor in
                guard let self, let newServer, self.server === newServer else { return }
                self.handleStartResult(result, port: port, server: newServer)
            }
        }
        logger.info("Mobile server starting on port \(port)")
    }

    private func handleStartResult(_ result: Result<Void, Error>, port: UInt16, server: MuxyRemoteServer) {
        switch result {
        case .success:
            lastError = nil
            isPortInUse = false
            logger.info("Mobile server started on port \(port)")
        case let .failure(error):
            logger.error("Mobile server failed to start on port \(port): \(error.localizedDescription)")
            isPortInUse = Self.isAddressInUseError(error)
            retireCurrentServer()
            lastError = friendlyMessage(for: error, port: port)
        }
    }

    private func friendlyMessage(for error: Error, port: UInt16) -> String {
        if Self.isAddressInUseError(error) {
            return "Port \(port) is already in use."
        }
        if let localized = (error as? LocalizedError)?.errorDescription {
            return localized
        }
        return "Could not start server on port \(port): \(error.localizedDescription)"
    }

    private static func isAddressInUseError(_ error: Error) -> Bool {
        if case let .posix(code) = error as? NWError, code == .EADDRINUSE {
            return true
        }
        return false
    }

    func freePort() {
        let port = self.port
        logger.info("Attempting to free port \(port)")
        Task.detached {
            let killed = await Self.terminateListeners(on: port)
            if killed {
                try? await Task.sleep(for: .milliseconds(500))
            }
            let stillInUse = !Self.pidsListening(on: port).isEmpty
            await MainActor.run {
                if stillInUse {
                    self.lastError = "Port \(port) is still in use. The process may not have exited."
                    self.isPortInUse = true
                } else {
                    self.lastError = nil
                    self.isPortInUse = false
                    self.setEnabled(true)
                }
            }
        }
    }

    nonisolated private static func terminateListeners(on port: UInt16) async -> Bool {
        let pids = pidsListening(on: port)
        guard !pids.isEmpty else {
            logger.info("No process found on port \(port)")
            return false
        }
        for pid in pids {
            logger.info("Killing PID \(pid) on port \(port)")
            kill(pid, SIGTERM)
        }
        return true
    }

    nonisolated private static func pidsListening(on port: UInt16) -> [pid_t] {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
        process.arguments = ["-ti", "TCP:\(port)", "-sTCP:LISTEN"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return []
        }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        guard let output = String(data: data, encoding: .utf8) else { return [] }
        return output
            .split(whereSeparator: \.isWhitespace)
            .compactMap { Int32($0) }
            .filter { $0 > 0 }
    }
}
