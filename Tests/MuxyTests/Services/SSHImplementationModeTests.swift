import Foundation
import Testing

@testable import Muxy
import MuxySSH

@Suite("SSHImplementationMode")
@MainActor
struct SSHImplementationModeTests {
    @Test("defaults to CLI implementation")
    func defaultsToCLIImplementation() {
        let previous = UserDefaults.standard.object(forKey: SSHImplementationMode.storageKey)
        defer {
            if let previous {
                UserDefaults.standard.set(previous, forKey: SSHImplementationMode.storageKey)
            } else {
                UserDefaults.standard.removeObject(forKey: SSHImplementationMode.storageKey)
            }
        }

        UserDefaults.standard.removeObject(forKey: SSHImplementationMode.storageKey)

        #expect(SSHImplementationMode.current == .cli)
    }

    @Test("rejects password auth in CLI mode")
    func rejectsPasswordAuthInCLIMode() {
        let previous = UserDefaults.standard.object(forKey: SSHImplementationMode.storageKey)
        defer {
            if let previous {
                UserDefaults.standard.set(previous, forKey: SSHImplementationMode.storageKey)
            } else {
                UserDefaults.standard.removeObject(forKey: SSHImplementationMode.storageKey)
            }
        }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)

        #expect(throws: SSHConnectionError.self) {
            try SSHImplementationSelection.validate(
                destination: SSHDestination(
                    host: "prod",
                    authenticationMethod: .password
                )
            )
        }
    }

    @Test("allows password auth in built-in mode")
    func allowsPasswordAuthInBuiltInMode() throws {
        let previous = UserDefaults.standard.object(forKey: SSHImplementationMode.storageKey)
        defer {
            if let previous {
                UserDefaults.standard.set(previous, forKey: SSHImplementationMode.storageKey)
            } else {
                UserDefaults.standard.removeObject(forKey: SSHImplementationMode.storageKey)
            }
        }

        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.storageKey)

        #expect(throws: Never.self) {
            try SSHImplementationSelection.validate(
                destination: SSHDestination(
                    host: "prod",
                    authenticationMethod: .password
                )
            )
        }
    }
}
