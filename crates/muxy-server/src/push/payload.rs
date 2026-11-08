//! The end-to-end encrypted notification payload, version 1, and the
//! identifiers derived from its key that travel in the clear.

use muxy_core::base64;
use muxy_protocol::{ActivityKind, PUSH_KEY_BYTES};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::digest::{SHA256, digest};
use ring::hmac;
use serde::Serialize;

pub(crate) type PushKey = [u8; PUSH_KEY_BYTES];

const VERSION: u8 = 1;
const KEY_ID_LABEL: &[u8] = b"muxy-push-key-id";
const KEY_ID_BYTES: usize = 8;
const HEADER_BYTES: usize = 1 + KEY_ID_BYTES;
const TAG_BYTES: usize = 16;
/// 3000 base64url characters carry 2250 bytes.
pub(crate) const MAX_PLAINTEXT: usize = 2250 - HEADER_BYTES - NONCE_LEN - TAG_BYTES;
const MAX_TITLE: usize = 120;
const MAX_BODY: usize = 240;
const DERIVED_ID: usize = 16;

/// What the phone shows and opens, before encryption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Notification {
    pub(crate) server_id: String,
    pub(crate) project_id: String,
    pub(crate) session_id: u64,
    pub(crate) event_id: u64,
    pub(crate) kind: ActivityKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) ts: u64,
}

#[derive(Serialize)]
struct Plaintext<'a> {
    v: u8,
    server_id: &'a str,
    project_id: &'a str,
    session_id: u64,
    event_id: u64,
    kind: &'static str,
    title: &'a str,
    body: &'a str,
    ts: u64,
}

impl Notification {
    pub(crate) fn plaintext(&self) -> Option<Vec<u8>> {
        let bytes = serde_json::to_vec(&Plaintext {
            v: VERSION,
            server_id: &self.server_id,
            project_id: &self.project_id,
            session_id: self.session_id,
            event_id: self.event_id,
            kind: kind_name(self.kind)?,
            title: truncated(&self.title, MAX_TITLE),
            body: truncated(&self.body, MAX_BODY),
            ts: self.ts,
        })
        .ok()?;
        (bytes.len() <= MAX_PLAINTEXT).then_some(bytes)
    }
}

pub(crate) fn kind_name(kind: ActivityKind) -> Option<&'static str> {
    match kind {
        ActivityKind::Attention => Some("attention"),
        ActivityKind::Completed => Some("completed"),
        ActivityKind::Unrecognized(_) => None,
    }
}

/// The first `limit` characters.
fn truncated(text: &str, limit: usize) -> &str {
    text.char_indices()
        .nth(limit)
        .map_or(text, |(end, _)| &text[..end])
}

/// Names the key among the ones a phone holds, one per paired server.
pub(crate) fn key_id(key: &PushKey) -> [u8; KEY_ID_BYTES] {
    let hash = digest(&SHA256, &[KEY_ID_LABEL, key].concat());
    let mut id = [0; KEY_ID_BYTES];
    id.copy_from_slice(&hash.as_ref()[..KEY_ID_BYTES]);
    id
}

/// `version ‖ key_id`.
pub(crate) fn header(key: &PushKey) -> [u8; HEADER_BYTES] {
    let mut header = [0; HEADER_BYTES];
    header[0] = VERSION;
    header[1..].copy_from_slice(&key_id(key));
    header
}

/// `header ‖ nonce ‖ ciphertext ‖ tag`, authenticated with `header ‖ server_id`.
pub(crate) fn seal(
    key: &PushKey,
    server_id: &str,
    nonce: [u8; NONCE_LEN],
    plaintext: &[u8],
) -> Option<Vec<u8>> {
    let header = header(key);
    let cipher = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).ok()?);
    let mut sealed = plaintext.to_vec();
    cipher
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from([header.as_slice(), server_id.as_bytes()].concat()),
            &mut sealed,
        )
        .ok()?;
    Some([header.as_slice(), &nonce, &sealed].concat())
}

/// The relay's `payload` field, under a fresh random nonce.
pub(crate) fn encrypt(key: &PushKey, notification: &Notification) -> Option<String> {
    let mut nonce = [0; NONCE_LEN];
    getrandom::fill(&mut nonce).ok()?;
    encrypt_with_nonce(key, notification, nonce)
}

pub(crate) fn encrypt_with_nonce(
    key: &PushKey,
    notification: &Notification,
    nonce: [u8; NONCE_LEN],
) -> Option<String> {
    let plaintext = notification.plaintext()?;
    seal(key, &notification.server_id, nonce, &plaintext).map(|bytes| base64::encode_url(&bytes))
}

/// Replaces an older notification for the same session on the phone.
pub(crate) fn collapse_id(key: &PushKey, session_id: u64) -> String {
    derived(key, format!("collapse:{session_id}").as_bytes())
}

/// Groups a project's notifications on the phone.
pub(crate) fn thread_id(key: &PushKey, project_id: &str) -> String {
    derived(
        key,
        [b"thread:".as_slice(), project_id.as_bytes()]
            .concat()
            .as_slice(),
    )
}

fn derived(key: &PushKey, message: &[u8]) -> String {
    let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), message);
    let mut text = base64::encode_url(tag.as_ref());
    text.truncate(DERIVED_ID);
    text
}

/// What a phone does with a payload: opens it with its key and the server's id.
#[cfg(test)]
pub(crate) fn open(key: &PushKey, server_id: &str, payload: &[u8]) -> Option<Vec<u8>> {
    let (header, rest) = payload.split_at(HEADER_BYTES);
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let cipher = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).ok()?);
    let mut sealed = sealed.to_vec();
    cipher
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce).ok()?,
            Aad::from([header, server_id.as_bytes()].concat()),
            &mut sealed,
        )
        .ok()
        .map(|opened| opened.to_vec())
}

#[cfg(test)]
pub(crate) fn base64url_decode(text: &str) -> Vec<u8> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'-' => 62,
        _ => 63,
    };
    let mut bytes = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let bits = chunk.iter().enumerate().fold(0_u32, |bits, (index, byte)| {
            bits | (u32::from(value(*byte)) << (18 - 6 * index))
        });
        for index in 0..chunk.len() - 1 {
            bytes.push(bits.to_be_bytes()[1 + index]);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use muxy_core::hex;

    use super::*;

    const KEY: PushKey = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
        0x1e, 0x1f,
    ];
    const NONCE: [u8; 12] = [
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b,
    ];
    const SERVER_ID: &str = "6b0c7e2a9f1d4c3b8a7e6d5c4b3a2910";
    const PLAINTEXT: &str = r#"{"v":1,"server_id":"6b0c7e2a9f1d4c3b8a7e6d5c4b3a2910","project_id":"0f3a5c7e9b1d2f4a6c8e0b2d4f6a8c1e","session_id":42,"event_id":1731067200000123,"kind":"attention","title":"Claude Code · muxy-website","body":"Needs your attention","ts":1731067200}"#;

    fn notification() -> Notification {
        Notification {
            server_id: SERVER_ID.into(),
            project_id: "0f3a5c7e9b1d2f4a6c8e0b2d4f6a8c1e".into(),
            session_id: 42,
            event_id: 1_731_067_200_000_123,
            kind: ActivityKind::Attention,
            title: "Claude Code · muxy-website".into(),
            body: "Needs your attention".into(),
            ts: 1_731_067_200,
        }
    }

    #[test]
    fn the_plaintext_is_the_documented_json() {
        assert_eq!(
            notification().plaintext(),
            Some(PLAINTEXT.as_bytes().to_vec())
        );
    }

    /// The vectors in the mobile SDK's README; the apps check their decryption against them.
    const SEALED: &str = concat!(
        "01627df844ff14f4ed202122232425262728292a2ba918d05256a9362c691930",
        "b8a46aab90b46bd6beb1e2538d5b935e7370ec6f6d41edd666b64402bf22995b",
        "f16c6b2ca9d013ddbeb8100f11bc0d15782fc655b4bc04f729a6f1d6f7be09de",
        "42823f0805e7f107658198be7bff2df04e2e7aa8bd3a371f33ea68bc80acefd7",
        "6ecc98927a14b3b576a3f28d682697697baaeee60a06d54aad0ade358657bf92",
        "5a3e0f2d94e4e6095ebaf6bea80a2adc5bfeb6bfb9b09a520df0d103e595dbac",
        "9e9c0858c7cac6d8e16980a0590ee02c13c657a6d29a8e017316f5ad2b303f99",
        "9c8dbccfb5ef1fd750a3ba77ec21c6662935742a79487100fff40011de7f46af",
        "e2cbcca126f94e82bf04383503c3d940df580e7d7801a3839f53c46fb77d",
    );
    const PAYLOAD: &str = concat!(
        "AWJ9-ET_FPTtICEiIyQlJicoKSorqRjQUlapNixpGTC4pGqrkLRr1r6x4lONW5Ne",
        "c3Dsb21B7dZmtkQCvyKZW_Fsayyp0BPdvrgQDxG8DRV4L8ZVtLwE9ymm8db3vgne",
        "QoI_CAXn8QdlgZi-e_8t8E4ueqi9OjcfM-povICs79duzJiSehSztXaj8o1oJpdp",
        "e6ru5goG1UqtCt41hle_klo-Dy2U5OYJXrr2vqgKKtxb_ra_ubCaUg3w0QPlldus",
        "npwIWMfKxtjhaYCgWQ7gLBPGV6bSmo4Bcxb1rSswP5mcjbzPte8f11CjunfsIcZm",
        "KTV0KnlIcQD_9AAR3n9Gr-LLzKEm-U6CvwQ4NQPD2UDfWA59eAGjg59TxG-3fQ",
    );

    #[test]
    fn a_fixed_key_and_nonce_give_the_documented_bytes() {
        assert_eq!(hex::encode(&key_id(&KEY)), "627df844ff14f4ed");
        assert_eq!(hex::encode(&header(&KEY)), "01627df844ff14f4ed");
        assert_eq!(PLAINTEXT.len(), 249);
        let sealed = seal(&KEY, SERVER_ID, NONCE, PLAINTEXT.as_bytes());
        assert_eq!(sealed.as_deref().map(hex::encode).as_deref(), Some(SEALED));
        let payload = encrypt_with_nonce(&KEY, &notification(), NONCE);
        assert_eq!(payload.as_deref(), Some(PAYLOAD));
        assert_eq!(collapse_id(&KEY, 42), "usZ3J1QStv6L6omf");
        assert_eq!(
            thread_id(&KEY, "0f3a5c7e9b1d2f4a6c8e0b2d4f6a8c1e"),
            "aJnt975IClwhFwRP"
        );
    }

    #[test]
    fn the_phone_opens_it_only_with_its_key_and_the_servers_id() {
        let Some(payload) = encrypt(&KEY, &notification()) else {
            panic!("encrypting failed");
        };
        assert!(payload.len() <= 3000);
        assert!(!payload.contains(['=', '+', '/']));
        assert_eq!(hex::encode(&base64url_decode(PAYLOAD)), SEALED);
        let bytes = base64url_decode(&payload);
        assert_eq!(bytes[..HEADER_BYTES], header(&KEY));
        assert_eq!(
            open(&KEY, SERVER_ID, &bytes),
            Some(PLAINTEXT.as_bytes().to_vec())
        );
        assert_eq!(open(&KEY, "another-server", &bytes), None);
        let mut other = KEY;
        other[0] ^= 1;
        assert_eq!(open(&other, SERVER_ID, &bytes), None);
        let mut tampered = bytes.clone();
        tampered[1] ^= 1;
        assert_eq!(open(&KEY, SERVER_ID, &tampered), None);
        assert_ne!(encrypt(&KEY, &notification()), Some(payload));
    }

    #[test]
    fn derived_ids_are_short_url_safe_and_differ_per_key_and_input() {
        let mut other = KEY;
        other[31] ^= 1;
        for id in [collapse_id(&KEY, 42), thread_id(&KEY, "p")] {
            assert_eq!(id.len(), 16);
            assert!(
                id.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            );
        }
        assert_ne!(collapse_id(&KEY, 42), collapse_id(&KEY, 43));
        assert_ne!(collapse_id(&KEY, 42), collapse_id(&other, 42));
        assert_ne!(thread_id(&KEY, "p"), thread_id(&KEY, "q"));
    }

    #[test]
    fn long_text_is_cut_by_characters_and_the_largest_payload_fits() {
        let mut long = notification();
        long.title = "é".repeat(500);
        long.body = "ü".repeat(500);
        let Some(plaintext) = long.plaintext() else {
            panic!("long text did not fit");
        };
        let text = String::from_utf8(plaintext).unwrap_or_default();
        assert!(text.contains(&format!("\"title\":\"{}\"", "é".repeat(120))));
        assert!(text.contains(&format!("\"body\":\"{}\"", "ü".repeat(240))));
        let control = Notification {
            title: "\u{1}".repeat(500),
            ..notification()
        };
        let Some(payload) = encrypt(&KEY, &control) else {
            panic!("escaped text did not fit");
        };
        assert!(payload.len() <= 3000, "{}", payload.len());
    }
}
