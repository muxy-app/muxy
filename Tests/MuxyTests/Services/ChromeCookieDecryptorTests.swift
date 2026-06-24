import Foundation
import Testing

@testable import Muxy

@Suite("ChromeCookieDecryptor")
struct ChromeCookieDecryptorTests {
    private let safeStoragePassword = "peanuts"
    private let encryptedV10Base64 = "djEwqHqEox1Z+dFydWNsALd51aiMflZL7UMmHHSinuykTHl81u//o1a/VlICPdMRWPl2"
    private let host = "example.com"
    private let expectedValue = "secret-value"
    private let legacyV10Base64 = "djEwb0a3I7dlcdlKdz3FlzZceUHW24fWNfUZhThUM8H7JteE9NYNFFOz0PhWnZMaufp4"
    private let legacyValue = "long-session-token-value-exceeding-32"

    @Test("derives a 16-byte AES-128 key")
    func derivesKey() {
        let key = ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword)
        #expect(key?.count == 16)
    }

    @Test("decrypts a v10 cookie and strips the domain hash prefix")
    func decryptsV10() throws {
        let key = try #require(ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword))
        let blob = try #require(Data(base64Encoded: encryptedV10Base64))
        let value = ChromeCookieDecryptor.decrypt(encryptedValue: blob, host: host, key: key)
        #expect(value == expectedValue)
    }

    @Test("keeps the full value for a legacy cookie without a domain hash prefix")
    func keepsValueForOldFormat() throws {
        let key = try #require(ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword))
        let blob = try #require(Data(base64Encoded: legacyV10Base64))
        let value = ChromeCookieDecryptor.decrypt(encryptedValue: blob, host: host, key: key)
        #expect(value == legacyValue)
    }

    @Test("returns the raw value for a non-versioned payload")
    func passesThroughPlaintext() throws {
        let key = try #require(ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword))
        let plain = Data("plain-value".utf8)
        #expect(ChromeCookieDecryptor.decrypt(encryptedValue: plain, host: host, key: key) == "plain-value")
    }

    @Test("returns nil for an empty payload")
    func emptyPayload() throws {
        let key = try #require(ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword))
        #expect(ChromeCookieDecryptor.decrypt(encryptedValue: Data(), host: host, key: key) == nil)
    }
}
