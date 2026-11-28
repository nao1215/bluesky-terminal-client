//! NIP-17 one-to-one messages: unsigned rumors, signed seals, ephemeral gift wraps.
use super::{
    Client,
    actions::{self, draft, now, tag},
    event::{Event, bytes},
    nip44,
    relay::{Filter, Relays},
};
use crate::{
    error::{Error, Result},
    output,
};
use k256::{
    elliptic_curve::{
        Generate,
        common::{getrandom::SysRng, rand_core::TryRng},
    },
    schnorr::SigningKey,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;

fn random() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    SysRng.try_fill_bytes(&mut bytes).map_err(Error::file)?;
    Ok(bytes)
}
fn obscured_time() -> Result<u64> {
    let r = random()?;
    Ok(now()?.saturating_sub(u64::from(u32::from_be_bytes([r[0], r[1], r[2], r[3]])) % 172800))
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rumor {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}
impl Rumor {
    fn event(&self) -> Event {
        Event {
            id: self.id.clone(),
            pubkey: self.pubkey.clone(),
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags.clone(),
            content: self.content.clone(),
            sig: String::new(),
        }
    }
    fn validate(&self, sender: &str, recipient: &str) -> Result<()> {
        let event = self.event();
        if self.kind != 14
            || self.pubkey != sender
            || self.id != hex::encode(event.digest()?)
            || !(self.pubkey == recipient || event.tag_values("p").any(|p| p == recipient))
        {
            return Err(Error::network("invalid or misaddressed NIP-17 rumor"));
        }
        Ok(())
    }
}
impl Client<'_> {
    pub(crate) fn inbox_list(&self) -> Result<super::event::Unsigned> {
        if self.config.dm_relays.is_empty() {
            return Err(Error::unavailable(
                "configure dmRelays before publishing your inbox list",
            ));
        }
        let old = self.latest(10050)?.value;
        actions::replacement(
            10050,
            self.config
                .dm_relays
                .iter()
                .map(|r| tag("relay", r))
                .collect(),
            String::new(),
            old.as_ref(),
        )
    }
    fn crypt(&self, peer: &str, text: &str, encrypt: bool) -> Result<String> {
        if let Some(bunker) = &self.config.bunker {
            return bunker.crypt(
                &self.config.public,
                peer,
                text,
                encrypt,
                self.config.timeout,
                &|url| self.authorization(url),
            );
        }
        let key = nip44::conversation_key(&self.config.signing_key()?, &bytes(peer)?)?;
        if encrypt {
            nip44::encrypt(&key, &random()?, text)
        } else {
            nip44::decrypt(&key, text).map(|s| s.to_string())
        }
    }
    fn inbox(&self, public: &str) -> Result<Vec<String>> {
        let list = self
            .latest_for(10050, public)?
            .value
            .ok_or_else(|| Error::unavailable("recipient has no kind-10050 DM relay list"))?;
        crate::config::relay_urls(list.tag_values("relay").map(str::to_owned).collect())
    }
    pub(crate) fn wrap(&self, rumor: &Rumor, recipient: &str) -> Result<Event> {
        let content = self.crypt(
            recipient,
            &serde_json::to_string(rumor).map_err(Error::usage)?,
            true,
        )?;
        let mut seal = draft(13, vec![], content);
        seal.created_at = obscured_time()?;
        let seal = self.sign(seal)?;
        let ephemeral = SigningKey::generate();
        let key = nip44::conversation_key(&ephemeral, &bytes(recipient)?)?;
        let content = nip44::encrypt(
            &key,
            &random()?,
            &serde_json::to_string(&seal).map_err(Error::usage)?,
        )?;
        Event::sign(
            &ephemeral,
            obscured_time()?,
            1059,
            vec![tag("p", recipient)],
            content,
        )
    }
    pub(crate) fn unwrap(&self, wrap: &Event) -> Result<Rumor> {
        wrap.verify()?;
        if wrap.kind != 1059 || !wrap.tag_values("p").any(|p| p == self.config.public) {
            return Err(Error::network("misaddressed gift wrap"));
        }
        let seal: Event = serde_json::from_str(&self.crypt(&wrap.pubkey, &wrap.content, false)?)
            .map_err(|_invalid| Error::network("invalid seal JSON"))?;
        seal.verify()?;
        if seal.kind != 13 || !seal.tags.is_empty() {
            return Err(Error::network("invalid NIP-59 seal"));
        }
        let rumor: Rumor = serde_json::from_str(&self.crypt(&seal.pubkey, &seal.content, false)?)
            .map_err(|_invalid| Error::network("invalid rumor JSON"))?;
        rumor.validate(&seal.pubkey, &self.config.public)?;
        Ok(rumor)
    }
    pub(crate) fn chat(
        &self,
        actor: Option<&str>,
        text: Option<&str>,
        count: usize,
    ) -> Result<(Vec<Value>, Vec<String>)> {
        let peer = actor.map(actions::public).transpose()?;
        if let Some(text) = text {
            let peer = peer.ok_or_else(|| Error::usage("chat text requires a recipient"))?;
            if text.trim().is_empty() {
                return Err(Error::usage("message must not be empty"));
            }
            let mut recipients = vec![peer.clone()];
            if peer != self.config.public {
                recipients.push(self.config.public.clone());
            }
            // Discover every inbox and prepare every envelope before the first publish.
            let destinations = recipients
                .iter()
                .map(|p| self.inbox(p))
                .collect::<Result<Vec<_>>>()?;
            let mut rumor = Rumor {
                id: String::new(),
                pubkey: self.config.public.clone(),
                created_at: now()?,
                kind: 14,
                tags: vec![tag("p", &peer)],
                content: text.into(),
            };
            rumor.id = hex::encode(rumor.event().digest()?);
            let wraps = recipients
                .iter()
                .map(|p| self.wrap(&rumor, p))
                .collect::<Result<Vec<_>>>()?;
            let mut accepted = 0;
            let mut warnings = vec![];
            for (urls, wrap) in destinations.iter().zip(&wraps) {
                let out = Relays {
                    urls,
                    timeout: self.config.timeout,
                }
                .publish(wrap, Some(self))
                .map_err(|e| Error::network(format!("DM delivery may be partial: {}", e.error)))?;
                accepted += out.value;
                warnings.extend(out.warnings);
            }
            return Ok((
                vec![
                    json!({"action":"chat","id":rumor.id,"target":peer,"url":null,"already":false,"acceptedRelays":accepted}),
                ],
                warnings,
            ));
        }
        let urls = self.inbox(&self.config.public)?;
        let out = Relays {
            urls: &urls,
            timeout: self.config.timeout,
        }
        .query(
            Filter {
                kinds: vec![1059],
                mentions: vec![self.config.public.clone()],
                ..Filter::default()
            },
            count.saturating_mul(4).min(1000),
        )?;
        let mut warnings = out.warnings;
        let mut rumors = vec![];
        let mut seen = HashSet::new();
        for wrap in out.value {
            match self.unwrap(&wrap) {
                Ok(r) if seen.insert(r.id.clone()) => {
                    if peer
                        .as_ref()
                        .is_none_or(|p| r.pubkey == *p || r.event().tag_values("p").any(|a| a == p))
                    {
                        rumors.push(r);
                    }
                }
                Ok(_) => (),
                Err(e) => warnings.push(e.error),
            }
        }
        rumors.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        rumors.truncate(count);
        let mut state = crate::read_state::ReadState::load(&self.config.public)?;
        let messages=rumors.iter().map(|r|Ok(json!({"id":r.id,"createdAt":output::timestamp(&r.event())?,"user":output::user(&r.pubkey)?,"text":r.content,"recipients":r.event().tag_values("p").collect::<Vec<_>>(),"isRead":state.seen.contains(&r.id)}))).collect::<Result<Vec<_>>>()?;
        if peer.is_some() {
            state.seen.extend(rumors.iter().map(|r| r.id.clone()));
            state.save(&self.config.public)?;
            return Ok((messages, warnings));
        }
        let mut conversations: Vec<Value> = Vec::new();
        for (rumor, message) in rumors.iter().zip(messages) {
            let mut participants = std::collections::BTreeSet::from([rumor.pubkey.clone()]);
            participants.extend(rumor.event().tag_values("p").map(str::to_owned));
            let id = participants.iter().cloned().collect::<Vec<_>>().join(":");
            let unread =
                usize::from(rumor.pubkey != self.config.public && !state.seen.contains(&rumor.id));
            if let Some(c) = conversations.iter_mut().find(|c| c["id"] == id) {
                c["unreadCount"] = json!(c["unreadCount"].as_u64().unwrap_or(0) + unread as u64);
            } else {
                conversations.push(json!({"id":id,"members":participants.iter().map(|p|output::user(p)).collect::<Result<Vec<_>>>()?,"lastMessage":message,"unreadCount":unread}));
            }
        }
        Ok((conversations, warnings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn official_nip59_example_decrypts_and_authenticates_both_layers() {
        let v: Value =
            serde_json::from_str(include_str!("../../tests/testdata/nip59.json")).unwrap();
        let key =
            SigningKey::from_slice(&bytes::<32>(v["recipientSecret"].as_str().unwrap()).unwrap())
                .unwrap();
        let wrap: Event = serde_json::from_value(v["wrap"].clone()).unwrap();
        wrap.verify().unwrap();
        let conversation = nip44::conversation_key(&key, &bytes(&wrap.pubkey).unwrap()).unwrap();
        let plaintext = nip44::decrypt(&conversation, &wrap.content).unwrap();
        let seal: Event = serde_json::from_str(&plaintext).unwrap();
        assert_eq!(serde_json::to_value(&seal).unwrap(), v["seal"]);
        seal.verify().unwrap();
        let conversation = nip44::conversation_key(&key, &bytes(&seal.pubkey).unwrap()).unwrap();
        let plaintext = nip44::decrypt(&conversation, &seal.content).unwrap();
        let rumor: Rumor = serde_json::from_str(&plaintext).unwrap();
        assert_eq!(serde_json::to_value(&rumor).unwrap(), v["rumor"]);
        assert_eq!(rumor.id, hex::encode(rumor.event().digest().unwrap()));
        // Re-encrypt the official seal's plaintext using its embedded nonce.
        use base64::{Engine, engine::general_purpose::STANDARD};
        let raw = STANDARD.decode(&seal.content).unwrap();
        assert_eq!(
            nip44::encrypt(&conversation, &raw[1..33].try_into().unwrap(), &plaintext).unwrap(),
            seal.content
        );
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn gift_wrap_roundtrip_and_sender_binding(text in ".{1,256}") {
            let dir=tempfile::tempdir().unwrap();
            let key=SigningKey::generate();
            let cfg=crate::config::Config::test_key(&key,dir.path());
            let client=Client::new(&cfg);
            let mut r=Rumor{id:String::new(),pubkey:cfg.public.clone(),created_at:123,kind:14,tags:vec![tag("p",&cfg.public)],content:text.clone()};
            r.id=hex::encode(r.event().digest().unwrap());
            let wrap=client.wrap(&r,&cfg.public).unwrap();
            prop_assert_eq!(wrap.kind,1059);prop_assert_ne!(&wrap.pubkey,&cfg.public);prop_assert_eq!(&wrap.tags,&vec![tag("p",&cfg.public)]);
            let unwrapped=client.unwrap(&wrap).unwrap();prop_assert_eq!(unwrapped.content,text);
            r.pubkey=hex::encode(SigningKey::generate().verifying_key().to_bytes());r.id=hex::encode(r.event().digest().unwrap());
            let forged=client.wrap(&r,&cfg.public).unwrap();prop_assert!(client.unwrap(&forged).is_err());
        }
    }
}
