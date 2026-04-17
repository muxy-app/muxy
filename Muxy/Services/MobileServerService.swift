import Foundation
import MuxyServer
import os

private let logger = Logger(subsystem: "app.muxy", category: "MobileServerService")

@MainActor
@Observable
final class MobileServerService {
    static let shared = MobileServerService()

    private static let enabledKey = "app.muxy.mobile.serverEnabled"

    private(set) var isEnabled: Bool {
        didSet {
            UserDefaults.standard.set(isEnabled, forKey: Self.enabledKey)
        }
    }

    private var server: MuxyRemoteServer?
    private var delegate: MuxyRemoteServerDelegate?
    private var delegateBuilder: ((MuxyRemoteServer) -> MuxyRemoteServerDelegate)?

    private init() {
        isEnabled = UserDefaults.standard.bool(forKey: Self.enabledKey)
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
            stop()
        }
    }

    func stop() {
        server?.stop()
        server = nil
        delegate = nil
    }

    private func start() {
        guard server == nil, let delegateBuilder else { return }
        let newServer = MuxyRemoteServer()
        let newDelegate = delegateBuilder(newServer)
        newServer.delegate = newDelegate
        newServer.start()
        server = newServer
        delegate = newDelegate
        logger.info("Mobile server started")
    }
}
