import Foundation
import MuxySSH

enum SSHImplementationMode: String, CaseIterable, Identifiable {
    case cli
    case native

    static let storageKey = "muxy.ssh.implementation"
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
}

enum SSHImplementationSelection {
    static func validate(destination: SSHDestination) throws {
        guard SSHImplementationMode.current == .cli, destination.authenticationMethod == .password else { return }
        throw SSHConnectionError
            .authFailed("System SSH (OpenSSH) does not support password authentication. Switch to Built-in SSH to use this device.")
    }
}
