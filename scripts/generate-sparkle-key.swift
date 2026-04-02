#!/usr/bin/env swift
import CryptoKit
import Foundation

let privateKey = Curve25519.Signing.PrivateKey()
let privateKeyBase64 = privateKey.rawRepresentation.base64EncodedString()
let publicKeyBase64 = privateKey.publicKey.rawRepresentation.base64EncodedString()

print("SPARKLE_PRIVATE_KEY=\(privateKeyBase64)")
print("")
print("Public key (for verification): \(publicKeyBase64)")
print("")
print("Add the private key as a GitHub Actions secret named SPARKLE_PRIVATE_KEY.")
print("The public key is derived automatically at build time — you don't need to store it separately.")
