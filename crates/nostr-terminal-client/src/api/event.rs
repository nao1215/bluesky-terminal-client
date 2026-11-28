//! NIP-01 canonical event hashing and BIP-340 signatures.

use k256::schnorr::{Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashVerifier};
use k256::sha2::{Digest, Sha256};
use k256::{
    elliptic_curve::common::getrandom::SysRng, schnorr::signature::hazmat::RandomizedPrehashSigner,
};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Event {
    pub(crate) id: String,
    pub(crate) pubkey: String,
    pub(crate) created_at: u64,
    pub(crate) kind: u16,
    pub(crate) tags: Vec<Vec<String>>,
    pub(crate) content: String,
    pub(crate) sig: String,
}

pub(crate) fn bytes<const N: usize>(value: &str) -> Result<[u8; N]> {
    let mut out = [0; N];
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::usage(
            "expected lowercase hexadecimal with the required length",
        ));
    }
    hex::decode_to_slice(value, &mut out).map_err(Error::usage)?;
    Ok(out)
}

impl Event {
    pub(crate) fn digest(&self) -> Result<[u8; 32]> {
        let canonical = serde_json::to_vec(&(
            0,
            &self.pubkey,
            self.created_at,
            self.kind,
            &self.tags,
            &self.content,
        ))
        .map_err(Error::usage)?;
        Ok(Sha256::digest(canonical).into())
    }

    pub(crate) fn sign(
        key: &SigningKey,
        created_at: u64,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Result<Self> {
        let mut event = Self {
            id: String::new(),
            pubkey: hex::encode(key.verifying_key().to_bytes()),
            created_at,
            kind,
            tags,
            content,
            sig: String::new(),
        };
        let digest = event.digest()?;
        event.id = hex::encode(digest);
        // NIP-01 signs the event ID directly; Signer would hash it a second time.
        event.sig = hex::encode(
            key.sign_prehash_with_rng(&mut SysRng, &digest)
                .map_err(|_invalid| Error::file("cannot sign event"))?
                .to_bytes(),
        );
        Ok(event)
    }

    pub(crate) fn verify(&self) -> Result<()> {
        let invalid = || Error::network("relay sent an invalid event ID, public key or signature");
        let id = bytes::<32>(&self.id).map_err(|_invalid| invalid())?;
        let pubkey = bytes::<32>(&self.pubkey).map_err(|_invalid| invalid())?;
        let sig = bytes::<64>(&self.sig).map_err(|_invalid| invalid())?;
        if self.digest()? != id
            || jiff::Timestamp::from_second(
                i64::try_from(self.created_at).map_err(|_invalid| invalid())?,
            )
            .is_err()
        {
            return Err(invalid());
        }
        let key = VerifyingKey::from_slice(&pubkey).map_err(|_invalid| invalid())?;
        let signature = Signature::try_from(sig.as_slice()).map_err(|_invalid| invalid())?;
        key.verify_prehash(&id, &signature)
            .map_err(|_invalid| invalid())
    }

    pub(crate) fn tag_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.tags
            .iter()
            .filter(move |t| t.first().is_some_and(|s| s == name))
            .filter_map(|t| t.get(1).map(String::as_str))
    }

    pub(crate) fn reply_id(&self) -> Option<&str> {
        let tags: Vec<_> = self
            .tags
            .iter()
            .filter(|t| {
                t.first().is_some_and(|s| s == "e")
                    && t.get(1).is_some_and(|s| bytes::<32>(s).is_ok())
            })
            .collect();
        for marker in ["reply", "root"] {
            if let Some(t) = tags.iter().find(|t| t.get(3).is_some_and(|s| s == marker)) {
                return Some(&t[1]);
            }
        }
        tags.iter()
            .rev()
            .find(|t| t.get(3).is_none_or(String::is_empty))
            .map(|t| t[1].as_str())
    }
}
