//! NIP-19 identifiers; relay hints are data and never connection instructions.

use crate::error::{Error, Result};
use bech32::{Bech32, Checksum, Hrp, primitives::decode::CheckedHrpstring};

// NIP-19 permits identifiers beyond Bech32's 1023-character error-correction bound.
enum NostrBech32 {}

impl Checksum for NostrBech32 {
    type MidstateRepr = <Bech32 as Checksum>::MidstateRepr;
    const CODE_LENGTH: usize = 5000;
    const CHECKSUM_LENGTH: usize = Bech32::CHECKSUM_LENGTH;
    const GENERATOR_SH: [Self::MidstateRepr; 5] = Bech32::GENERATOR_SH;
    const TARGET_RESIDUE: Self::MidstateRepr = Bech32::TARGET_RESIDUE;
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Identifier {
    Public([u8; 32]),
    Secret([u8; 32]),
    Note([u8; 32]),
    Profile {
        public: [u8; 32],
        relays: Vec<String>,
    },
    Event {
        id: [u8; 32],
        relays: Vec<String>,
        author: Option<[u8; 32]>,
        kind: Option<u32>,
    },
}

fn invalid() -> Error {
    Error::usage("invalid or unsupported NIP-19 identifier")
}

fn array<const N: usize>(data: &[u8]) -> Result<[u8; N]> {
    data.try_into().map_err(|_invalid| invalid())
}

fn tlv(out: &mut Vec<u8>, kind: u8, data: &[u8]) -> Result<()> {
    if kind == 1 && !data.is_ascii() {
        return Err(invalid());
    }
    out.extend([
        kind,
        u8::try_from(data.len()).map_err(|_invalid| invalid())?,
    ]);
    out.extend(data);
    Ok(())
}

impl Identifier {
    pub(crate) fn encode(&self) -> Result<String> {
        let mut data = Vec::new();
        let prefix = match self {
            Self::Public(b) | Self::Secret(b) | Self::Note(b) => {
                data.extend(b);
                match self {
                    Self::Public(_) => "npub",
                    Self::Secret(_) => "nsec",
                    _ => "note",
                }
            }
            Self::Profile { public, relays } => {
                tlv(&mut data, 0, public)?;
                for relay in relays {
                    tlv(&mut data, 1, relay.as_bytes())?;
                }
                "nprofile"
            }
            Self::Event {
                id,
                relays,
                author,
                kind,
            } => {
                tlv(&mut data, 0, id)?;
                for relay in relays {
                    tlv(&mut data, 1, relay.as_bytes())?;
                }
                if let Some(a) = author {
                    tlv(&mut data, 2, a)?;
                }
                if let Some(k) = kind {
                    tlv(&mut data, 3, &k.to_be_bytes())?;
                }
                "nevent"
            }
        };
        let encoded =
            bech32::encode::<NostrBech32>(Hrp::parse(prefix).map_err(|_invalid| invalid())?, &data)
                .map_err(|_invalid| invalid())?;
        if encoded.len() > 5000 {
            return Err(invalid());
        }
        Ok(encoded)
    }

    pub(crate) fn decode(input: &str) -> Result<Self> {
        if input.len() > 5000 {
            return Err(invalid());
        }
        let checked = CheckedHrpstring::new::<NostrBech32>(input).map_err(|_invalid| invalid())?;
        let data: Vec<u8> = checked.byte_iter().collect();
        let prefix = checked.hrp().to_string().to_ascii_lowercase();
        match prefix.as_str() {
            "npub" => return Ok(Self::Public(array(&data)?)),
            "nsec" => return Ok(Self::Secret(array(&data)?)),
            "note" => return Ok(Self::Note(array(&data)?)),
            "nprofile" | "nevent" => (),
            _ => return Err(invalid()),
        }
        let (mut special, mut author, mut kind) = (None, None, None);
        let mut relays = Vec::new();
        let mut rest = data.as_slice();
        while !rest.is_empty() {
            if rest.len() < 2 {
                return Err(invalid());
            }
            let (t, len) = (rest[0], rest[1] as usize);
            let value = rest.get(2..2 + len).ok_or_else(invalid)?;
            match t {
                0 => {
                    if special.is_some() {
                        return Err(invalid());
                    }
                    special = Some(array(value)?);
                }
                1 => {
                    if !value.is_ascii() {
                        return Err(invalid());
                    }
                    relays.push(String::from_utf8(value.to_vec()).map_err(|_invalid| invalid())?);
                }
                2 if prefix == "nevent" => {
                    if author.is_some() {
                        return Err(invalid());
                    }
                    author = Some(array(value)?);
                }
                3 if prefix == "nevent" => {
                    if kind.is_some() {
                        return Err(invalid());
                    }
                    kind = Some(u32::from_be_bytes(array(value)?));
                }
                _ => (),
            }
            rest = &rest[2 + len..];
        }
        let id = special.ok_or_else(invalid)?;
        Ok(if prefix == "nprofile" {
            Self::Profile { public: id, relays }
        } else {
            Self::Event {
                id,
                relays,
                author,
                kind,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_identifiers_keep_the_bech32_checksum_and_nip19_bound() {
        NostrBech32::sanity_check();
        let id = Identifier::Profile {
            public: [7; 32],
            relays: vec![format!("wss://{}.test", "a".repeat(180)); 10],
        };
        let encoded = id.encode().unwrap();
        assert!(encoded.len() > 1023);
        assert!(Identifier::decode(&encoded).unwrap() == id);
        let too_long = Identifier::Profile {
            public: [7; 32],
            relays: vec!["a".repeat(250); 16],
        };
        assert!(too_long.encode().is_err());
        assert!(Identifier::decode(&"a".repeat(5001)).is_err());
    }
}
