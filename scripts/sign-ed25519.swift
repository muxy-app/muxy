#!/usr/bin/env swift
import CryptoKit
import Foundation

guard CommandLine.arguments.count == 3 else {
    fputs("Usage: sign-ed25519.swift <base64-private-key> <file-path>\n", stderr)
    exit(1)
}

var b64 = CommandLine.arguments[1]
while b64.count % 4 != 0 {
    b64 += "="
}
guard let keyData = Data(base64Encoded: b64) else {
    fputs("Error: invalid base64 key\n", stderr)
    exit(1)
}

let privateKey: Curve25519.Signing.PrivateKey
if keyData.count == 32 {
    privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: keyData)
} else if keyData.count == 96 {
    privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: keyData[..<32])
} else {
    fputs("Error: unexpected key length \(keyData.count) (expected 32 or 96)\n", stderr)
    exit(1)
}

let filePath = CommandLine.arguments[2]
guard let fileData = FileManager.default.contents(atPath: filePath) else {
    fputs("Error: cannot read file at \(filePath)\n", stderr)
    exit(1)
}

let signature = try privateKey.signature(for: fileData)
print(signature.rawRepresentation.base64EncodedString())
