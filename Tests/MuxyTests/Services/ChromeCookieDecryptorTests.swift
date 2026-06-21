import Foundation
import Testing

@testable import Muxy

@Suite("ChromeCookieDecryptor")
struct ChromeCookieDecryptorTests {
    private let safeStoragePassword = "peanuts"
    private let encryptedV10Base64 = "djEwqHqEox1Z+dFydWNsALd51aiMflZL7UMmHHSinuykTHl81u//o1a/VlICPdMRWPl2"
    private let expectedValue = "secret-value"

    @Test("derives a 16-byte AES-128 key")
    func derivesKey() {
        let key = ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword)
        #expect(key?.count == 16)
    }

    @Test("decrypts a v10 cookie and strips the domain hash prefix")
    func decryptsV10() throws {
        let key = try #require(ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword))
        let blob = try #require(Data(base64Encoded: encryptedV10Base64))
        let value = ChromeCookieDecryptor.decrypt(encryptedValue: blob, key: key)
        #expect(value == expectedValue)
    }

    @Test("returns nil for an empty payload")
    func emptyPayload() {
        let key = ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: safeStoragePassword)!
        #expect(ChromeCookieDecryptor.decrypt(encryptedValue: Data(), key: key) == nil)
    }
}
