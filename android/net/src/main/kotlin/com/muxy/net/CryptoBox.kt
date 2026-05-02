package com.muxy.net

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal data class EncryptedPayload(val iv: ByteArray, val ciphertext: ByteArray) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is EncryptedPayload) return false
        return iv.contentEquals(other.iv) && ciphertext.contentEquals(other.ciphertext)
    }

    override fun hashCode(): Int = 31 * iv.contentHashCode() + ciphertext.contentHashCode()
}

internal interface CryptoBox {
    fun encrypt(plaintext: ByteArray): EncryptedPayload

    fun decrypt(payload: EncryptedPayload): ByteArray

    fun deleteKey()
}

internal class KeystoreCryptoBox(private val keyAlias: String) : CryptoBox {
    override fun encrypt(plaintext: ByteArray): EncryptedPayload {
        val key = getOrCreateKey()
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val ciphertext = cipher.doFinal(plaintext)
        return EncryptedPayload(iv = cipher.iv, ciphertext = ciphertext)
    }

    override fun decrypt(payload: EncryptedPayload): ByteArray {
        val key = loadKey() ?: error("Keystore key '$keyAlias' missing")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, payload.iv))
        return cipher.doFinal(payload.ciphertext)
    }

    override fun deleteKey() {
        val keystore = androidKeystore()
        if (keystore.containsAlias(keyAlias)) {
            keystore.deleteEntry(keyAlias)
        }
    }

    private fun getOrCreateKey(): SecretKey {
        loadKey()?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val spec =
            KeyGenParameterSpec.Builder(
                keyAlias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(AES_KEY_SIZE_BITS)
                .build()
        generator.init(spec)
        return generator.generateKey()
    }

    private fun loadKey(): SecretKey? {
        val keystore = androidKeystore()
        return keystore.getKey(keyAlias, null) as? SecretKey
    }

    private fun androidKeystore(): KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val GCM_TAG_BITS = 128
        const val AES_KEY_SIZE_BITS = 256
    }
}
