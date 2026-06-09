import Foundation
import CryptoKit
import Darwin
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

    @Test("Shell mode sends cd as initial input without explicit command")
    func shellModeInitialInput() {
        let host = RemoteHost(name: "Prod", host: "example.com", user: "deploy")
        let remoteConfig = RemoteProjectConfig(
            hostID: host.id,
            remotePath: "/srv/app path",
            displayName: "App",
            icon: nil,
            iconColor: nil
        )

        let configuration = NativeSSHConnectionConfiguration.make(host: host, remoteConfig: remoteConfig)

        #expect(configuration.remoteExecCommand == nil)
        #expect(configuration.initialShellInput == "cd '/srv/app path'\n")
    }

    @Test("Exec mode runs provided command in remote directory")
    func execModeCommand() {
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

        #expect(configuration.remoteExecCommand == "cd /srv/app; swift test")
        #expect(configuration.initialShellInput == "cd /srv/app\n")
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

    @Test("FD bridge connects Ghostty and SSH directions")
    func fdBridgeConnectsDirections() throws {
        let bridge = try NativeSSHFileDescriptorBridge.make()
        defer { bridge.closeAllBeforeSurfaceCreation() }

        try expectWrite(fd: bridge.ghosttyWriteFD, bytes: [0x61, 0x62])
        try expectRead(fd: bridge.sshReadFD, bytes: [0x61, 0x62])

        try expectWrite(fd: bridge.sshWriteFD, bytes: [0x63, 0x64])
        try expectRead(fd: bridge.ghosttyReadFD, bytes: [0x63, 0x64])
    }

    @Test("Error mapper handles auth and network failures")
    func errorMapper() {
        if case .hostKeyChanged = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.hostKeyChanged, host: "example.com") {
        } else {
            Issue.record("Host key changes should map to host-key changed")
        }

        if case .authFailed = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.unsupportedKeyType, host: "example.com") {
        } else {
            Issue.record("Unsupported key type should map to auth failure")
        }

        if case .unknownHostKey("example.com") = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.unknownHostKey, host: "example.com") {
        } else {
            Issue.record("Unknown host key should map to unknown-host-key error")
        }

        if case .authFailed = SSHConnectionErrorMapper.map(NativeSSHConnectionFailure.encryptedPrivateKey, host: "example.com") {
        } else {
            Issue.record("Encrypted key should map to auth failure")
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
        let firstKeyLine = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f"
        let secondKey = try NIOSSHPublicKey(
            openSSHPublicKey: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4/"
        )
        let secondKeyLine = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4/"
        let knownHosts = """
        example.com \(firstKeyLine)
        [example.net]:2222 \(secondKeyLine)
        """

        #expect(NativeSSHKnownHosts.validate(host: "example.com", port: 22, hostKey: firstKey, knownHosts: knownHosts) == .trusted)
        #expect(NativeSSHKnownHosts.validate(host: "example.com", port: 22, hostKey: secondKey, knownHosts: knownHosts) == .changed)
        #expect(NativeSSHKnownHosts.validate(host: "missing.example", port: 22, hostKey: firstKey, knownHosts: knownHosts) == .unknown)
        #expect(NativeSSHKnownHosts.validate(host: "example.net", port: 2222, hostKey: secondKey, knownHosts: knownHosts) == .trusted)
        #expect(NativeSSHKnownHosts.validate(host: "api.example.com", port: 22, hostKey: firstKey, knownHosts: "*.example.com \(firstKeyLine)") == .trusted)
        #expect(
            NativeSSHKnownHosts.validate(
                host: "hashed.example.com",
                port: 22,
                hostKey: firstKey,
                knownHosts: "\(hashedHostPattern(host: "hashed.example.com")) \(firstKeyLine)",
            ) == .trusted
        )
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

    private func expectWrite(fd: Int32, bytes: [UInt8]) throws {
        let result = bytes.withUnsafeBytes { buffer in
            Darwin.write(fd, buffer.baseAddress, buffer.count)
        }

        #expect(result == bytes.count)
        if result != bytes.count {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
    }

    private func expectRead(fd: Int32, bytes: [UInt8]) throws {
        var buffer = [UInt8](repeating: 0, count: bytes.count)
        let result = Darwin.read(fd, &buffer, buffer.count)

        #expect(result == bytes.count)
        #expect(buffer == bytes)
        if result != bytes.count || buffer != bytes {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
    }
}


private func hashedHostPattern(host: String) -> String {
    let salt = "known-hosts-salt".data(using: .utf8) ?? Data()
    let hostData = host.data(using: .utf8) ?? Data()
    let digest = HMAC<SHA1>.authenticationCode(for: hostData, using: SymmetricKey(data: salt))
    let saltText = salt.base64EncodedString()
    let digestText = Data(digest).base64EncodedString()
    return "|1|\(saltText)|\(digestText)"
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
