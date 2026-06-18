import Foundation
import Testing

@testable import Muxy
import MuxySSH

@Suite("SSHImplementationMode", .serialized)
@MainActor
struct SSHImplementationModeTests {
    @Test("defaults to CLI implementation")
    func defaultsToCLIImplementation() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.removeObject(forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.removeObject(forKey: SSHImplementationMode.pendingStorageKey)

        #expect(SSHImplementationMode.current == .cli)
    }

    @Test("requested change is pending until launch")
    func requestedChangeIsPendingUntilLaunch() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.removeObject(forKey: SSHImplementationMode.pendingStorageKey)

        SSHImplementationMode.requestChange(to: .native)

        #expect(SSHImplementationMode.current == .cli)
        #expect(SSHImplementationMode.selectedForNextLaunch == .native)
        #expect(SSHImplementationMode.hasPendingRestart)
    }

    @Test("requesting current mode cancels pending change")
    func requestingCurrentModeCancelsPendingChange() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.pendingStorageKey)

        SSHImplementationMode.requestChange(to: .cli)

        #expect(SSHImplementationMode.selectedForNextLaunch == .cli)
        #expect(!SSHImplementationMode.hasPendingRestart)
        #expect(UserDefaults.standard.object(forKey: SSHImplementationMode.pendingStorageKey) == nil)
    }

    @Test("cancel pending change returns selection to current mode")
    func cancelPendingChangeReturnsSelectionToCurrentMode() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.pendingStorageKey)

        SSHImplementationMode.cancelPendingChange()

        #expect(SSHImplementationMode.selectedForNextLaunch == .cli)
        #expect(!SSHImplementationMode.hasPendingRestart)
    }

    @Test("pending selection applies at launch")
    func pendingSelectionAppliesAtLaunch() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.pendingStorageKey)

        SSHImplementationMode.applyPendingSelectionAtLaunch()

        #expect(SSHImplementationMode.current == .native)
        #expect(SSHImplementationMode.selectedForNextLaunch == .native)
        #expect(UserDefaults.standard.object(forKey: SSHImplementationMode.pendingStorageKey) == nil)
    }

    @Test("rejects password auth in CLI mode")
    func rejectsPasswordAuthInCLIMode() {
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.pendingStorageKey)

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
        let snapshot = SSHImplementationModeSnapshot.capture()
        defer { snapshot.restore() }

        UserDefaults.standard.set(SSHImplementationMode.native.rawValue, forKey: SSHImplementationMode.storageKey)
        UserDefaults.standard.set(SSHImplementationMode.cli.rawValue, forKey: SSHImplementationMode.pendingStorageKey)

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

private struct SSHImplementationModeSnapshot {
    let values: [String: Any]

    @MainActor
    static func capture() -> SSHImplementationModeSnapshot {
        let keys = [
            SSHImplementationMode.storageKey,
            SSHImplementationMode.pendingStorageKey,
        ]
        return SSHImplementationModeSnapshot(values: Dictionary(uniqueKeysWithValues: keys.map { key in
            (key, UserDefaults.standard.object(forKey: key) ?? NSNull())
        }))
    }

    @MainActor
    func restore() {
        for (key, value) in values {
            if value is NSNull {
                UserDefaults.standard.removeObject(forKey: key)
            } else {
                UserDefaults.standard.set(value, forKey: key)
            }
        }
    }
}
