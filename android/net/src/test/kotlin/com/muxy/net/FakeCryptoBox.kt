package com.muxy.net

import java.security.SecureRandom

internal class FakeCryptoBox(initialKey: ByteArray? = null) : CryptoBox {
    private val random = SecureRandom()
    private var key: ByteArray? = initialKey

    var tamperNextDecrypt: Boolean = false

    override fun encrypt(plaintext: ByteArray): EncryptedPayload {
        val keyBytes = key ?: ByteArray(KEY_LENGTH).also {
            random.nextBytes(it)
            key = it
        }
        val iv = ByteArray(IV_LENGTH).also { random.nextBytes(it) }
        val ciphertext = xor(plaintext, keyBytes, iv)
        return EncryptedPayload(iv = iv, ciphertext = ciphertext)
    }

    override fun decrypt(payload: EncryptedPayload): ByteArray {
        if (tamperNextDecrypt) {
            tamperNextDecrypt = false
            error("simulated decryption failure")
        }
        val keyBytes = key ?: error("Key was deleted")
        return xor(payload.ciphertext, keyBytes, payload.iv)
    }

    override fun deleteKey() {
        key = null
    }

    fun exportKey(): ByteArray? = key?.copyOf()

    private fun xor(input: ByteArray, key: ByteArray, iv: ByteArray): ByteArray {
        val output = ByteArray(input.size)
        for (i in input.indices) {
            output[i] = (input[i].toInt() xor key[i % key.size].toInt() xor iv[i % iv.size].toInt()).toByte()
        }
        return output
    }

    private companion object {
        const val KEY_LENGTH = 32
        const val IV_LENGTH = 12
    }
}
