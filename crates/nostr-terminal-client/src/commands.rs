//! Command surface; protocol operations are shared with the full-screen client.
use crate::{
    accounts,
    api::{Client, actions, event::Unsigned, media, relay::Filter},
    config::Config,
    error::{Error, Result},
    output,
};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
    path::PathBuf,
};

#[derive(Args)]
pub(crate) struct Limit {
    /// Maximum results (at least one).
    #[arg(short = 'n', long, default_value = "20")]
    limit: NonZeroUsize,
}
#[derive(Args)]
pub(crate) struct Post {
    /// Public text; - reads standard input.
    text: String,
    /// Reply to this event ID, note or nevent.
    #[arg(long)]
    reply: Option<String>,
    /// Quote this event using NIP-18 q tags.
    #[arg(long)]
    quote: Option<String>,
    /// Content warning; readers initially hide the content.
    #[arg(long)]
    cw: Option<String>,
    /// Image to upload through configured Blossom servers (repeatable).
    #[arg(long)]
    image: Vec<PathBuf>,
    /// Video to upload through Blossom.
    #[arg(long)]
    video: Option<PathBuf>,
    /// Alt text for attachments, in order.
    #[arg(long)]
    alt: Vec<String>,
    /// Show the plan without contacting relays, signing or uploading.
    #[arg(long)]
    dry_run: bool,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Read your profile.
    Whoami,
    /// Read your own and followed authors' notes.
    #[command(alias = "tl")]
    Timeline(Limit),
    /// Read mentions, replies, quotes, reactions and reposts.
    #[command(alias = "notif")]
    Notifications {
        #[command(flatten)]
        count: Limit,
        /// Mark the returned notifications as read locally for this account.
        #[arg(long)]
        seen: bool,
    },
    /// Read profile metadata by public key, npub or nprofile.
    Profile { actor: Option<String> },
    /// Read an author's notes and reposts.
    Author {
        actor: String,
        #[command(flatten)]
        count: Limit,
    },
    /// Read one event by ID, note or nevent.
    Note { id: String },
    /// Read a note's ancestors and replies (at most 32 ancestors).
    Thread {
        id: String,
        #[command(flatten)]
        count: Limit,
    },
    /// Search #hashtags, or text on configured relays advertising NIP-50.
    Search {
        text: String,
        #[command(flatten)]
        count: Limit,
    },
    /// Publish a public note.
    Post(Post),
    /// Publish a reaction (NIP-25).
    Like {
        id: String,
        #[arg(long, default_value = "+")]
        reaction: String,
    },
    /// Request deletion of your reactions to a note.
    Unlike { id: String },
    /// Repost a note.
    Repost { id: String },
    /// Request deletion of your reposts of a note.
    Unrepost { id: String },
    /// Request deletion of your own event; relays decide whether to honor it.
    Delete {
        id: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// List observed reactions to a note.
    Likes {
        id: String,
        #[command(flatten)]
        count: Limit,
    },
    /// List observed reposts of a note.
    Reposts {
        id: String,
        #[command(flatten)]
        count: Limit,
    },
    /// Add an author to your kind-3 list.
    Follow { actor: String },
    /// Remove an author from your kind-3 list.
    Unfollow { actor: String },
    /// Read an author's public follow list.
    Follows { actor: Option<String> },
    /// Read observed followers; this is not a network-wide count.
    Followers {
        actor: Option<String>,
        #[command(flatten)]
        count: Limit,
    },
    /// Add an author to the public NIP-51 mute list; block has the same meaning.
    #[command(alias = "block")]
    Mute { actor: String },
    /// Remove an author from the public mute list.
    #[command(alias = "unblock")]
    Unmute { actor: String },
    /// Read your public mute list.
    #[command(alias = "blocks")]
    Mutes,
    /// Publish a public NIP-56 report; relays decide what to honor.
    Report {
        id: String,
        #[arg(long, default_value = "other")]
        reason: String,
        #[arg(long, default_value = "")]
        comment: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Merge a JSON object into your kind-0 profile, retaining unknown fields.
    ProfileUpdate { patch: String },
    /// Upload an attachment with metadata removed, returning NIP-92 fields.
    Upload {
        path: PathBuf,
        #[arg(long, default_value = "")]
        alt: String,
    },
    /// Save a note's attachments (never overwrite an existing file).
    Download {
        id: String,
        #[arg(short = 'o', long, default_value = ".")]
        output: PathBuf,
    },
    /// List saved configurations or choose the default account.
    Accounts {
        #[arg(long = "use")]
        select: Option<String>,
    },
    /// Save an account configuration; key material remains in your key file.
    Login {
        name: String,
        #[arg(long)]
        public_key: String,
        #[arg(long, conflicts_with = "bunker")]
        key_file: Option<PathBuf>,
        #[arg(long)]
        bunker: Option<String>,
        #[arg(long, required = true)]
        relay: Vec<String>,
    },
    /// Remove a saved configuration (does not delete the key file).
    Logout {
        name: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Read or send NIP-17 private messages using kind-10050 DM relays.
    Chat {
        actor: Option<String>,
        text: Option<String>,
        #[command(flatten)]
        count: Limit,
    },
    /// Publish configured dmRelays as your kind-10050 inbox list.
    DmRelays,
}
fn input(text: String) -> Result<String> {
    if text != "-" {
        return Ok(text);
    }
    let mut out = String::new();
    io::stdin()
        .take(65536)
        .read_to_string(&mut out)
        .map_err(Error::file)?;
    if out.len() >= 65536 {
        return Err(Error::usage("stdin exceeds 65535 bytes"));
    }
    Ok(out)
}
fn confirm(yes: bool, action: &str) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    eprint!("{action}? [y/N] ");
    io::stderr().flush().map_err(Error::file)?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).map_err(Error::file)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}
fn wrote(
    client: &Client<'_>,
    draft: Unsigned,
    action: &str,
    target: Option<&str>,
) -> Result<(Vec<Value>, Vec<String>)> {
    let out = client.publish(draft)?;
    let mut value = output::publication(&out.value, out.answered)?;
    value["action"] = json!(action);
    value["target"] = json!(target);
    Ok((vec![value], out.warnings))
}
pub(crate) fn run(command: Command, account: Option<&str>) -> Result<(Vec<Value>, Vec<String>)> {
    match &command {
        Command::Accounts { select } => {
            if let Some(s) = select {
                accounts::select(s)?;
            }
            return Ok((
                accounts::list()?
                    .into_iter()
                    .map(|s| {
                        let config = Config::load_account(Some(&s))?;
                        let mut value = output::user(&config.public)?;
                        value["name"] = json!(s);
                        value["service"] = json!("nostr");
                        Ok(value)
                    })
                    .collect::<Result<_>>()?,
                vec![],
            ));
        }
        Command::Login {
            name,
            public_key,
            key_file,
            bunker,
            relay,
        } => {
            accounts::name(name)?;
            let key = key_file
                .as_ref()
                .map(std::fs::canonicalize)
                .transpose()
                .map_err(Error::file)?;
            let data = json!({"publicKey":actions::public(public_key)?,"keyFile":key,"bunker":bunker,"relays":crate::config::relay_urls(relay.clone())?});
            if let Some(uri) = bunker {
                crate::api::nip46::Bunker::parse(uri)?;
            }
            accounts::add(Some(name), data)?;
            return Ok((
                vec![
                    json!({"id":actions::public(public_key)?,"username":output::user(&actions::public(public_key)?)?["username"],"host":null,"service":"nostr","name":name}),
                ],
                vec![],
            ));
        }
        Command::Logout { name, yes } => {
            if confirm(*yes, "Remove saved account")? {
                accounts::remove(name)?;
            }
            return Ok((vec![], vec![]));
        }
        _ => (),
    }
    // A dry run needs neither an account nor a signer and performs no networking.
    if let Command::Post(p) = &command
        && p.dry_run
    {
        let text = input(p.text.clone())?;
        validate_post(p, &text)?;
        return Ok((
            vec![
                json!({"action":"post","dryRun":true,"text":text,"replyId":p.reply,"quoteId":p.quote,"cw":p.cw,"files":p.image.iter().chain(p.video.iter()).map(|p|p.display().to_string()).collect::<Vec<_>>()}),
            ],
            vec![],
        ));
    }
    if let Command::Post(p) = &command
        && p.text != "-"
    {
        validate_post(p, &p.text)?;
    }
    let config = Config::load_account(account)?;
    let client = Client::new(&config);
    match command {
        Command::Whoami | Command::Profile { actor: None } => {
            let out = client.profile(&config.public)?;
            Ok((
                vec![output::account(&config.public, out.value.as_ref())?],
                out.warnings,
            ))
        }
        Command::Profile { actor: Some(actor) } => {
            let actor = actions::public(&actor)?;
            let out = client.profile(&actor)?;
            Ok((
                vec![output::account(&actor, out.value.as_ref())?],
                out.warnings,
            ))
        }
        Command::Timeline(c) => client.posts(client.timeline(c.limit.get())?),
        Command::Notifications { count, seen } => {
            let out = client.notifications(count.limit.get())?;
            let mut state = crate::read_state::ReadState::load(&config.public)?;
            if seen {
                state.seen.extend(out.value.iter().map(|e| e.id.clone()));
                state.save(&config.public)?;
            }
            let mut values = out
                .value
                .iter()
                .map(|e| {
                    let mut v = output::notification(e)?;
                    v["isRead"] = json!(state.seen.contains(&e.id));
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            let mut warnings = out.warnings;
            client.enrich(&mut values, &mut warnings);
            Ok((values, warnings))
        }
        Command::Author { actor, count } => client.posts(client.author(&actor, count.limit.get())?),
        Command::Note { id } => {
            let out = client.note(&id)?;
            Ok((vec![output::post(&out.value)?], out.warnings))
        }
        Command::Thread { id, count } => client.posts(client.thread(&id, count.limit.get())?),
        Command::Search { text, count } => client.posts(client.search(&text, count.limit.get())?),
        Command::Post(p) => {
            let text = input(p.text.clone())?;
            validate_post(&p, &text)?;
            let reply = p
                .reply
                .as_ref()
                .map(|id| client.note(id).map(|o| o.value))
                .transpose()?;
            let quote = p
                .quote
                .as_ref()
                .map(|id| client.note(id).map(|o| o.value))
                .transpose()?;
            let mut draft = actions::post(
                text,
                reply.as_ref(),
                quote.as_ref(),
                p.cw.as_deref(),
                &config.relays[0],
            );
            for (i, path) in p.image.iter().chain(p.video.iter()).enumerate() {
                let tag = client.upload(path, p.alt.get(i).map_or("", String::as_str))?;
                draft.content.push('\n');
                draft.content.push_str(tag[1].trim_start_matches("url "));
                draft.tags.push(tag);
            }
            wrote(&client, draft, "post", None)
        }
        Command::Like { id, reaction } => {
            let note = client.note(&id)?.value;
            wrote(
                &client,
                actions::reaction(&note, &reaction),
                "like",
                Some(&note.id),
            )
        }
        Command::Repost { id } => {
            let note = client.note(&id)?.value;
            wrote(
                &client,
                actions::repost(&note, &config.relays[0])?,
                "repost",
                Some(&note.id),
            )
        }
        Command::Unlike { ref id } | Command::Unrepost { ref id } => {
            let kind = if matches!(command, Command::Unlike { .. }) {
                7
            } else {
                6
            };
            let action = if kind == 7 { "unlike" } else { "unrepost" };
            let note = client.note(id)?.value;
            match client.retract(&note, kind)? {
                Some(d) => wrote(&client, d, action, Some(&note.id)),
                None => Ok((
                    vec![
                        json!({"action":action,"target":note.id,"id":null,"url":null,"already":true}),
                    ],
                    vec![],
                )),
            }
        }
        Command::Delete { id, yes } => {
            if !confirm(yes, "Publish deletion request")? {
                return Ok((vec![], vec![]));
            }
            let note = client.note(&id)?.value;
            wrote(
                &client,
                actions::deletion(&[note], &config.public, "")?,
                "delete",
                Some(&id),
            )
        }
        Command::Likes { ref id, ref count } | Command::Reposts { ref id, ref count } => {
            let kind = if matches!(command, Command::Likes { .. }) {
                7
            } else {
                6
            };
            client.posts(client.relays().query(
                Filter {
                    kinds: vec![kind],
                    references: vec![actions::id(id)?],
                    ..Filter::default()
                },
                count.limit.get(),
            )?)
        }
        Command::Follow { ref actor }
        | Command::Unfollow { ref actor }
        | Command::Mute { ref actor }
        | Command::Unmute { ref actor } => {
            let (kind, add, action) = match command {
                Command::Follow { .. } => (3, true, "follow"),
                Command::Unfollow { .. } => (3, false, "unfollow"),
                Command::Mute { .. } => (10000, true, "mute"),
                _ => (10000, false, "unmute"),
            };
            wrote(
                &client,
                client.relation(kind, actor, add)?,
                action,
                Some(actor),
            )
        }
        Command::Follows { .. } | Command::Mutes => {
            let (kind, who) = match &command {
                Command::Follows { actor } => (3, actor.as_deref().unwrap_or(&config.public)),
                _ => (10000, config.public.as_str()),
            };
            let out = client.latest_for(kind, who)?;
            let mut values = Vec::new();
            if let Some(e) = out.value {
                for p in e.tag_values("p") {
                    if crate::api::event::bytes::<32>(p).is_ok() {
                        values.push(output::user(p)?);
                    }
                }
            }
            Ok((values, out.warnings))
        }
        Command::Followers { actor, count } => {
            let out = client.relays().query(
                Filter {
                    kinds: vec![3],
                    mentions: vec![actions::public(actor.as_deref().unwrap_or(&config.public))?],
                    ..Filter::default()
                },
                count.limit.get(),
            )?;
            Ok((
                out.value
                    .iter()
                    .map(|e| output::user(&e.pubkey))
                    .collect::<Result<_>>()?,
                out.warnings,
            ))
        }
        Command::Report {
            id,
            reason,
            comment,
            yes,
        } => {
            if !confirm(yes, "Publish public report")? {
                return Ok((vec![], vec![]));
            }
            let note = client.note(&id)?.value;
            wrote(
                &client,
                actions::report(&note, &reason, &comment)?,
                "report",
                Some(&id),
            )
        }
        Command::ProfileUpdate { patch } => wrote(
            &client,
            client.update_profile(serde_json::from_str(&input(patch)?).map_err(Error::usage)?)?,
            "profile-update",
            None,
        ),
        Command::Upload { path, alt } => {
            Ok((vec![json!({"imeta":client.upload(&path,&alt)?})], vec![]))
        }
        Command::Download { id, output: dir } => {
            let note = client.note(&id)?.value;
            Ok((media::download(&note, &dir)?, vec![]))
        }

        Command::Chat { actor, text, count } => client.chat(
            actor.as_deref(),
            text.map(input).transpose()?.as_deref(),
            count.limit.get(),
        ),
        Command::DmRelays => wrote(&client, client.inbox_list()?, "dm-relays", None),
        Command::Accounts { .. } | Command::Login { .. } | Command::Logout { .. } => {
            Err(Error::usage("account command was not handled"))
        }
    }
}
fn validate_post(p: &Post, text: &str) -> Result<()> {
    if text.trim().is_empty() && p.image.is_empty() && p.video.is_none() {
        return Err(Error::usage("post text must not be empty"));
    }
    if p.alt.len() > p.image.len() + usize::from(p.video.is_some()) {
        return Err(Error::usage("more alt texts than attachments"));
    }
    Ok(())
}
