import Foundation
import MuxyServer
import Network
import os

private let logger = Logger(subsystem: "app.muxy", category: "MobileServerService")

@MainActor
@Observable
final class MobileServerService {
    static let shared = MobileServerService()

    static let defaultPort: UInt16 = MuxyRemoteServer.defaultPort
    static let minPort: UInt16 = 1024
    static let maxPort: UInt16 = 65535

    private static let enabledKey = "app.muxy.mobile.serverEnabled"
    private static let portKey = "app.muxy.mobile.serverPort"

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

    private var server: MuxyRemoteServer?
    private var delegate: MuxyRemoteServerDelegate?
    private var delegateBuilder: ((MuxyRemoteServer) -> MuxyRemoteServerDelegate)?
    private var pendingServers: [MuxyRemoteServer] = []

    private init() {
        isEnabled = UserDefaults.standard.bool(forKey: Self.enabledKey)
        let storedPort = UserDefaults.standard.object(forKey: Self.portKey) as? Int
        if let storedPort, let value = UInt16(exactly: storedPort), Self.isValid(port: value) {
            port = value
        } else {
            port = Self.defaultPort
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
        guard enabled != isEnabled else { return }
        isEnabled = enabled
        if enabled {
            start()
        } else {
            retireCurrentServer()
            lastError = nil
        }
    }

    func stop() {
        setEnabled(false)
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
            logger.info("Mobile server started on port \(port)")
        case let .failure(error):
            logger.error("Mobile server failed to start on port \(port): \(error.localizedDescription)")
            stop()
            isEnabled = false
            lastError = friendlyMessage(for: error, port: port)
        }
    }

    private func friendlyMessage(for error: Error, port: UInt16) -> String {
        if case let .posix(code) = error as? NWError, code == .EADDRINUSE {
            return "Port \(port) is already in use. Choose a different port."
        }
        if let localized = (error as? LocalizedError)?.errorDescription {
            return localized
        }
        return "Could not start server on port \(port): \(error.localizedDescription)"
    }
}
