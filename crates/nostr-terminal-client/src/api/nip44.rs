//! NIP-44 v2 payloads. Outer NIP-01 signatures must be checked before decryption.

use crate::error::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20::{
    ChaCha20,
    cipher::{KeyIvInit, StreamCipher},
};
use hmac::{Hmac, KeyInit, Mac};
use k256::{
    elliptic_curve::point::AffineCoordinates,
    schnorr::{SigningKey, VerifyingKey},
    sha2::Sha256,
};
use zeroize::Zeroizing;

// Bound allocations and preserve interoperability with the original v2 payload format.
const MAX_PLAINTEXT: usize = 65535;
const MAX_PAYLOAD: usize = 87472;

fn invalid() -> Error {
    Error::network("invalid NIP-44 payload")
}

fn mac(key: &[u8]) -> Result<Hmac<Sha256>> {
    Hmac::<Sha256>::new_from_slice(key).map_err(|_invalid| invalid())
}

pub(crate) fn conversation_key(
    secret: &SigningKey,
    public: &[u8; 32],
) -> Result<Zeroizing<[u8; 32]>> {
    let public = VerifyingKey::from_slice(public).map_err(|_invalid| invalid())?;
    // BIP-340 lifts x to the even point; negating either scalar changes only y.
    let shared = (*public.as_affine() * secret.as_nonzero_scalar().as_ref()).to_affine();
    let shared_x = Zeroizing::new(<[u8; 32]>::from(shared.x()));
    let mut extract = mac(b"nip44-v2")?;
    extract.update(shared_x.as_ref());
    Ok(Zeroizing::new(extract.finalize().into_bytes().into()))
}

fn message_keys(key: &[u8; 32], nonce: &[u8; 32]) -> Result<Zeroizing<[u8; 76]>> {
    // RFC 5869 HKDF-expand, fixed L=76: T(i)=HMAC(PRK,T(i-1)||info||i).
    let mut out = Zeroizing::new([0; 76]);
    let mut previous = Zeroizing::new(Vec::new());
    for (i, chunk) in out.chunks_mut(32).enumerate() {
        let mut expand = mac(key)?;
        expand.update(&previous);
        expand.update(nonce);
        expand.update(&[(i + 1) as u8]);
        *previous = expand.finalize().into_bytes().to_vec();
        chunk.copy_from_slice(&previous[..chunk.len()]);
    }
    Ok(out)
}

fn padded_len(len: usize) -> usize {
    if len <= 32 {
        return 32;
    }
    let next = len.next_power_of_two();
    let chunk = if next <= 256 { 32 } else { next / 8 };
    len.div_ceil(chunk) * chunk
}

pub(crate) fn encrypt(key: &[u8; 32], nonce: &[u8; 32], text: &str) -> Result<String> {
    if !(1..=MAX_PLAINTEXT).contains(&text.len()) {
        return Err(Error::usage(
            "NIP-44 plaintext must contain 1 to 65535 UTF-8 bytes",
        ));
    }
    let keys = message_keys(key, nonce)?;
    let mut payload = Vec::with_capacity(67 + padded_len(text.len()));
    payload.push(2);
    payload.extend_from_slice(nonce);
    payload.extend_from_slice(&(text.len() as u16).to_be_bytes());
    payload.extend_from_slice(text.as_bytes());
    payload.resize(35 + padded_len(text.len()), 0);
    ChaCha20::new_from_slices(&keys[..32], &keys[32..44])
        .map_err(|_invalid| invalid())?
        .apply_keystream(&mut payload[33..]);
    let mut auth = mac(&keys[44..])?;
    auth.update(&payload[1..]);
    payload.extend_from_slice(&auth.finalize().into_bytes());
    Ok(STANDARD.encode(payload))
}

pub(crate) fn decrypt(key: &[u8; 32], payload: &str) -> Result<Zeroizing<String>> {
    if payload.starts_with('#') {
        return Err(Error::network("unsupported NIP-44 version"));
    }
    if !(132..=MAX_PAYLOAD).contains(&payload.len()) {
        return Err(invalid());
    }
    let mut data = Zeroizing::new(STANDARD.decode(payload).map_err(|_invalid| invalid())?);
    if data.len() < 99 {
        return Err(invalid());
    }
    if data[0] != 2 {
        return Err(Error::network("unsupported NIP-44 version"));
    }
    let nonce = data[1..33].try_into().map_err(|_invalid| invalid())?;
    let keys = message_keys(key, &nonce)?;
    let end = data.len() - 32;
    let mut auth = mac(&keys[44..])?;
    auth.update(&data[1..end]);
    auth.verify_slice(&data[end..])
        .map_err(|_invalid| invalid())?;
    let padded = &mut data[33..end];
    ChaCha20::new_from_slices(&keys[..32], &keys[32..44])
        .map_err(|_invalid| invalid())?
        .apply_keystream(padded);
    let len = u16::from_be_bytes([padded[0], padded[1]]) as usize;
    if len == 0
        || padded.len() != 2 + padded_len(len)
        || len + 2 > padded.len()
        || padded[2 + len..].iter().any(|b| *b != 0)
    {
        return Err(invalid());
    }
    let text = std::str::from_utf8(&padded[2..2 + len]).map_err(|_invalid| invalid())?;
    Ok(Zeroizing::new(text.to_owned()))
}

#[cfg(test)]
#[path = "nip44_tests.rs"]
mod tests;
