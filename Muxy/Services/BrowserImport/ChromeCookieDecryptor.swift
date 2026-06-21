import CommonCrypto
import Foundation

enum ChromeCookieDecryptor {
    private static let salt = "saltysalt"
    private static let iterations = 1003
    private static let keyLength = kCCKeySizeAES128
    private static let iv = Data(repeating: 0x20, count: kCCBlockSizeAES128)
    private static let domainHashLength = 32

    static func deriveKey(fromSafeStoragePassword password: String) -> Data? {
        guard let passwordData = password.data(using: .utf8),
              let saltData = salt.data(using: .utf8)
        else { return nil }

        var derived = Data(count: keyLength)
        let status = derived.withUnsafeMutableBytes { derivedBytes in
            saltData.withUnsafeBytes { saltBytes in
                passwordData.withUnsafeBytes { passwordBytes in
                    CCKeyDerivationPBKDF(
                        CCPBKDFAlgorithm(kCCPBKDF2),
                        passwordBytes.bindMemory(to: Int8.self).baseAddress,
                        passwordData.count,
                        saltBytes.bindMemory(to: UInt8.self).baseAddress,
                        saltData.count,
                        CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA1),
                        UInt32(iterations),
                        derivedBytes.bindMemory(to: UInt8.self).baseAddress,
                        keyLength
                    )
                }
            }
        }
        return status == kCCSuccess ? derived : nil
    }

    static func decrypt(encryptedValue: Data, key: Data) -> String? {
        guard encryptedValue.count > 3 else { return nil }
        let prefix = encryptedValue.prefix(3)
        guard prefix == Data("v10".utf8) || prefix == Data("v11".utf8) else {
            return String(data: encryptedValue, encoding: .utf8)
        }

        let ciphertext = encryptedValue.dropFirst(3)
        guard !ciphertext.isEmpty, ciphertext.count.isMultiple(of: kCCBlockSizeAES128) else { return nil }

        guard let decrypted = aesCBCDecrypt(ciphertext: Data(ciphertext), key: key, iv: iv) else {
            return nil
        }

        let payload = decrypted.count > domainHashLength ? decrypted.dropFirst(domainHashLength) : decrypted
        if let value = String(data: payload, encoding: .utf8) {
            return value
        }
        return String(data: decrypted, encoding: .utf8)
    }

    private static func aesCBCDecrypt(ciphertext: Data, key: Data, iv: Data) -> Data? {
        let outputCapacity = ciphertext.count + kCCBlockSizeAES128
        var output = Data(count: outputCapacity)
        var decryptedCount = 0

        let status = output.withUnsafeMutableBytes { outputBytes in
            ciphertext.withUnsafeBytes { ciphertextBytes in
                key.withUnsafeBytes { keyBytes in
                    iv.withUnsafeBytes { ivBytes in
                        CCCrypt(
                            CCOperation(kCCDecrypt),
                            CCAlgorithm(kCCAlgorithmAES),
                            CCOptions(kCCOptionPKCS7Padding),
                            keyBytes.baseAddress,
                            key.count,
                            ivBytes.baseAddress,
                            ciphertextBytes.baseAddress,
                            ciphertext.count,
                            outputBytes.baseAddress,
                            outputCapacity,
                            &decryptedCount
                        )
                    }
                }
            }
        }

        guard status == kCCSuccess else { return nil }
        output.removeSubrange(decryptedCount ..< output.count)
        return output
    }
}
