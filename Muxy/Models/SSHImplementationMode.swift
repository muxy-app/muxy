import Foundation
import MuxySSH

enum SSHImplementationMode: String, CaseIterable, Identifiable {
    case cli
    case native

    static let storageKey = "muxy.ssh.implementation"
    static let pendingStorageKey = "muxy.ssh.implementation.pending"
    static let defaultValue: SSHImplementationMode = .cli

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .cli: "System SSH (OpenSSH)"
        case .native: "Built-in SSH"
        }
    }

    static var current: SSHImplementationMode {
        UserDefaults.standard.string(forKey: storageKey)
            .flatMap(SSHImplementationMode.init(rawValue:)) ?? defaultValue
    }

    static var selectedForNextLaunch: SSHImplementationMode {
        UserDefaults.standard.string(forKey: pendingStorageKey)
            .flatMap(SSHImplementationMode.init(rawValue:)) ?? current
    }

    static var hasPendingRestart: Bool {
        selectedForNextLaunch != current
    }

    static func requestChange(to mode: SSHImplementationMode) {
        if mode == current {
            cancelPendingChange()
            return
        }
        UserDefaults.standard.set(mode.rawValue, forKey: pendingStorageKey)
    }

    static func cancelPendingChange() {
        UserDefaults.standard.removeObject(forKey: pendingStorageKey)
    }

    static func applyPendingSelectionAtLaunch() {
        guard let pending = UserDefaults.standard.string(forKey: pendingStorageKey)
            .flatMap(SSHImplementationMode.init(rawValue:))
        else { return }
        UserDefaults.standard.set(pending.rawValue, forKey: storageKey)
        cancelPendingChange()
    }
}

enum SSHImplementationSelection {
    static func validate(destination: SSHDestination) throws {
        guard SSHImplementationMode.current == .cli, destination.authenticationMethod == .password else { return }
        throw SSHConnectionError
            .authFailed("System SSH (OpenSSH) does not support password authentication. Switch to Built-in SSH to use this device.")
    }
}
