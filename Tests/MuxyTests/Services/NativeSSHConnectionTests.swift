import Foundation
import CryptoKit
import NIOSSH
import Testing

@testable import Muxy

@Suite("Native SSH connection")
struct NativeSSHConnectionTests {
    @Test("Private key authentication is selected before keychain password")
    func privateKeyAuthenticationWins() {
        let host = RemoteHost(
            name: "Prod",
            host: "example.com",
            port: 2222,
            user: "deploy",
            identityFile: "~/.ssh/id_ed25519",
            useKeychain: true
        )

        #expect(NativeSSHConnectionConfiguration.authentication(for: host) == .privateKey(path: "~/.ssh/id_ed25519"))
    }

    @Test("Missing authentication returns nil")
    func missingAuthentication() {
        let host = RemoteHost(name: "Prod", host: "example.com", user: "deploy")

        #expect(NativeSSHConnectionConfiguration.authentication(for: host) == nil)
    }

    @Test("Remote shell command changes directory and starts login shell")
    func remoteShellCommand() {
        let host = RemoteHost(name: "Prod", host: "example.com", user: "deploy")
        let remoteConfig = RemoteProjectConfig(
            hostID: host.id,
            remotePath: "/srv/app path",
            displayName: "App",
            icon: nil,
            iconColor: nil
        )

        let configuration = NativeSSHConnectionConfiguration.make(host: host, remoteConfig: remoteConfig)

        #expect(configuration.remoteCommand == "cd '/srv/app path'; exec $SHELL -l")
    }

    @Test("Remote command tab runs provided command in remote directory")
    func remoteCommandTabCommand() {
        let host = RemoteHost(name: "Prod", host: "example.com", user: "deploy")
        let remoteConfig = RemoteProjectConfig(
            hostID: host.id,
            remotePath: "/srv/app",
            displayName: "App",
            icon: nil,
            iconColor: nil
        )

        let configuration = NativeSSHConnectionConfiguration.make(
            host: host,
            remoteConfig: remoteConfig,
            command: "swift test"
        )

        #expect(configuration.remoteCommand == "cd /srv/app; swift test")
    }

    @Test("FD bridge allocates usable descriptors")
    func fdBridgeAllocatesDescriptors() throws {
        let bridge = try NativeSSHFileDescriptorBridge.make()
        defer { bridge.closeAllBeforeSurfaceCreation() }

        #expect(bridge.ghosttyReadFD >= 0)
        #expect(bridge.ghosttyWriteFD >= 0)
        #expect(bridge.sshReadFD >= 0)
        #expect(bridge.sshWriteFD >= 0)
    }

    @Test("Error mapper handles auth and network failures")
    func errorMapper() {
        if case .hostKeyChanged = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.hostKeyChanged, host: "example.com") {
        } else {
            Issue.record("Host key changes should map to host-key changed")
        }

        if case .authFailed = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.unsupportedPrivateKey, host: "example.com") {
        } else {
            Issue.record("Unsupported key should map to auth failure")
        }

        if case .refused("example.com") = SSHConnectionErrorMapper.map(POSIXError(.ECONNREFUSED), host: "example.com") {
        } else {
            Issue.record("Connection refused should map to refused")
        }

        if case .timeout("example.com") = SSHConnectionErrorMapper.map(POSIXError(.ETIMEDOUT), host: "example.com") {
        } else {
            Issue.record("Timeout should map to timeout")
        }
    }

    @Test("Known hosts validation detects trusted changed and unknown hosts")
    func knownHostsValidation() throws {
        let firstKey = try NIOSSHPublicKey(
            openSSHPublicKey: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f"
        )
        let secondKey = try NIOSSHPublicKey(
            openSSHPublicKey: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4/"
        )
        let knownHosts = """
        example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f
        [example.net]:2222 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4/
        """

        #expect(NativeSSHKnownHosts.validate(host: "example.com", port: 22, hostKey: firstKey, knownHosts: knownHosts) == .trusted)
        #expect(NativeSSHKnownHosts.validate(host: "example.com", port: 22, hostKey: secondKey, knownHosts: knownHosts) == .changed)
        #expect(NativeSSHKnownHosts.validate(host: "missing.example", port: 22, hostKey: firstKey, knownHosts: knownHosts) == .unknown)
        #expect(NativeSSHKnownHosts.validate(host: "example.net", port: 2222, hostKey: secondKey, knownHosts: knownHosts) == .trusted)
    }

    @Test("Loads unencrypted OpenSSH Ed25519 private key")
    func loadsOpenSSHEd25519PrivateKey() throws {
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(0 ..< 32))
        let keyText = openSSHPrivateKey(privateKey: privateKey)
        let fileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-test-\(UUID().uuidString)")
        try keyText.write(to: fileURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: fileURL) }

        _ = try NativeSSHPrivateKeyLoader.load(path: fileURL.path)
    }

    private func openSSHPrivateKey(privateKey: Curve25519.Signing.PrivateKey) -> String {
        let publicKey = privateKey.publicKey.rawRepresentation
        let privateAndPublic = privateKey.rawRepresentation + publicKey
        var privateBlob = Data()
        privateBlob.appendUInt32(0x1234_5678)
        privateBlob.appendUInt32(0x1234_5678)
        privateBlob.appendSSHString("ssh-ed25519")
        privateBlob.appendSSHString(publicKey)
        privateBlob.appendSSHString(privateAndPublic)
        privateBlob.appendSSHString("")
        while privateBlob.count % 8 != 0 {
            privateBlob.append(UInt8(privateBlob.count % 255 + 1))
        }

        var publicBlob = Data()
        publicBlob.appendSSHString("ssh-ed25519")
        publicBlob.appendSSHString(publicKey)

        var envelope = Data("openssh-key-v1\0".utf8)
        envelope.appendSSHString("none")
        envelope.appendSSHString("none")
        envelope.appendSSHString(Data())
        envelope.appendUInt32(1)
        envelope.appendSSHString(publicBlob)
        envelope.appendSSHString(privateBlob)

        let encoded = envelope.base64EncodedString()
        return """
        -----BEGIN OPENSSH PRIVATE KEY-----
        \(encoded)
        -----END OPENSSH PRIVATE KEY-----
        """
    }
}

private extension Data {
    mutating func appendUInt32(_ value: UInt32) {
        append(UInt8((value >> 24) & 0xff))
        append(UInt8((value >> 16) & 0xff))
        append(UInt8((value >> 8) & 0xff))
        append(UInt8(value & 0xff))
    }

    mutating func appendSSHString(_ value: String) {
        appendSSHString(Data(value.utf8))
    }

    mutating func appendSSHString(_ value: Data) {
        appendUInt32(UInt32(value.count))
        append(value)
    }
}
