import CryptoKit
import Foundation
import NIOSSH
import Testing

import MuxySSH

@Suite("SSH private key loader")
struct SSHPrivateKeyLoaderTests {
    @Test("loads an unencrypted OpenSSH ed25519 private key")
    func loadsUnencryptedOpenSSHKey() throws {
        let key = Curve25519.Signing.PrivateKey()
        let text = makeOpenSSHPrivateKey(privateKey: key, comment: "muxy@test")

        let loaded = try loadKey(text)

        #expect(loaded == loadedExpectedPublicKey(for: key))
    }

    @Test("rejects malformed OpenSSH header")
    func rejectsMalformedHeader() throws {
        let text = """
        -----BEGIN OPENSSH PRIVATE KEY-----
        invalid
        -----END OPENSSH PRIVATE KEY-----
        """

        #expect(throws: SSHConnectionFailure.privateKeyLoadFailed) {
            try loadKey(text)
        }
    }

    @Test("rejects mismatched private key checks")
    func rejectsMismatchedChecks() throws {
        let key = Curve25519.Signing.PrivateKey()
        let text = makeOpenSSHPrivateKey(privateKey: key, check0: 1, check1: 2)

        #expect(throws: SSHConnectionFailure.privateKeyLoadFailed) {
            try loadKey(text)
        }
    }

    @Test("rejects mismatched embedded public key")
    func rejectsMismatchedEmbeddedPublicKey() throws {
        let key = Curve25519.Signing.PrivateKey()
        let otherKey = Curve25519.Signing.PrivateKey()
        let text = makeOpenSSHPrivateKey(privateKey: key, embeddedPublicKey: otherKey.publicKey.rawRepresentation)

        #expect(throws: SSHConnectionFailure.privateKeyLoadFailed) {
            try loadKey(text)
        }
    }

    @Test("rejects invalid padding")
    func rejectsInvalidPadding() throws {
        let key = Curve25519.Signing.PrivateKey()
        let text = makeOpenSSHPrivateKey(privateKey: key, padding: [9, 9, 9, 9])

        #expect(throws: SSHConnectionFailure.privateKeyLoadFailed) {
            try loadKey(text)
        }
    }

    @Test("rejects encrypted OpenSSH ed25519 keys")
    func rejectsEncryptedOpenSSHKey() throws {
        let text = """
        -----BEGIN OPENSSH PRIVATE KEY-----
        b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABBQRAFCo9
        /vv0icX60s6O6UAAAAEAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIBrez0rdYqROdkIA
        qvSrLoYFO1KVEidE4wclxivVKMbmAAAAoA9dkA6h2tAtANBP9RzyKvgrw5JKVJLVHfvZRQ
        8d3ttvy7WOs15y8lL/SdHiCyRukkKOPRd02zqx5g6WSmXZ0dKho/aMMO+58cIxsbCmMePT
        HaJvuQjIx6DIEoQyq83rQeVngk5rgvgou2jgHy/35C1AHtUysH4DIcltmrU3rvMF8i2GL4
        Od3cZL5cIOQVsmAZS6t3oL+GVeVOMFCqGFxjc=
        -----END OPENSSH PRIVATE KEY-----
        """

        #expect(throws: SSHConnectionFailure.encryptedPrivateKey) {
            try loadKey(text)
        }
    }

    private func loadKey(_ text: String) throws -> NIOSSHPublicKey {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-ssh-key-\(UUID().uuidString)")
        try text.write(to: url, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: url) }
        return try SSHPrivateKeyLoader.load(path: url.path).publicKey
    }

    private func loadedExpectedPublicKey(for key: Curve25519.Signing.PrivateKey) -> NIOSSHPublicKey {
        NIOSSHPrivateKey(ed25519Key: key).publicKey
    }
}

private func makeOpenSSHPrivateKey(
    privateKey: Curve25519.Signing.PrivateKey,
    comment: String = "",
    check0: UInt32 = 0x1234_5678,
    check1: UInt32 = 0x1234_5678,
    publicKey: Data? = nil,
    embeddedPublicKey: Data? = nil,
    padding: [UInt8]? = nil
) -> String {
    let publicKey = publicKey ?? privateKey.publicKey.rawRepresentation
    let embeddedPublicKey = embeddedPublicKey ?? publicKey
    let privateAndPublic = privateKey.rawRepresentation + embeddedPublicKey
    let commentData = Data(comment.utf8)

    var privateBlock = Data()
    privateBlock.append(encodeUInt32(check0))
    privateBlock.append(encodeUInt32(check1))
    privateBlock.append(encodeString("ssh-ed25519"))
    privateBlock.append(encodeData(publicKey))
    privateBlock.append(encodeData(privateAndPublic))
    privateBlock.append(encodeData(commentData))

    if let padding {
        privateBlock.append(contentsOf: padding)
    } else {
        let paddingCount = (8 - (privateBlock.count % 8)) % 8
        if paddingCount > 0 {
            privateBlock.append(contentsOf: (1 ... paddingCount).map(UInt8.init))
        }
    }

    var publicBlock = Data()
    publicBlock.append(encodeString("ssh-ed25519"))
    publicBlock.append(encodeData(publicKey))

    var payload = Data()
    payload.append("openssh-key-v1".data(using: .utf8)!)
    payload.append(0)
    payload.append(encodeString("none"))
    payload.append(encodeString("none"))
    payload.append(encodeData(Data()))
    payload.append(encodeUInt32(1))
    payload.append(encodeData(publicBlock))
    payload.append(encodeData(privateBlock))

    let base64 = payload.base64EncodedString()
    let lines = stride(from: 0, to: base64.count, by: 70).map { start in
        let startIndex = base64.index(base64.startIndex, offsetBy: start)
        let endIndex = base64.index(startIndex, offsetBy: min(70, base64.count - start))
        return String(base64[startIndex ..< endIndex])
    }

    return """
    -----BEGIN OPENSSH PRIVATE KEY-----
    \(lines.joined(separator: "\n"))
    -----END OPENSSH PRIVATE KEY-----
    """
}

private func encodeUInt32(_ value: UInt32) -> Data {
    var bigEndian = value.bigEndian
    return withUnsafeBytes(of: &bigEndian) { Data($0) }
}

private func encodeString(_ value: String) -> Data {
    encodeData(Data(value.utf8))
}

private func encodeData(_ value: Data) -> Data {
    var data = Data()
    data.append(encodeUInt32(UInt32(value.count)))
    data.append(value)
    return data
}
