//! Mastodon actions and lists using the same API as the full-screen client.
use super::*;
use crate::api::types::Conversation;

impl Paged for Account {
    fn id(&self) -> &str {
        &self.id
    }
}
impl Paged for Conversation {
    fn id(&self) -> &str {
        &self.id
    }
}

fn own_status(ctx: &Ctx, typed: &str) -> Result<String> {
    let (origin, id) = parse_status(typed)?;
    if origin.as_deref().is_some_and(|o| o != ctx.origin) {
        return Err(
            Error::usage("the post is given by its address on another server")
                .with_hint("give its ID on the server you write on"),
        );
    }
    Ok(id)
}

fn local_account(client: &Client, typed: &str) -> Result<Account> {
    let mut who = parse_user(typed)?;
    if let Some(origin) = who.origin.take() {
        // Resolve a foreign account on our server; never send our token elsewhere.
        if who.host.is_none() {
            who.host = Some(api::authority(&origin).into());
        }
    }
    find_account(client, &who)
}

fn wrote(ctx: &Ctx, out: &mut dyn Write, action: &str, target: &str, extra: Value) -> Result<()> {
    let mut value =
        json!({"action": action, "target": target, "id": null, "url": null, "already": false});
    if let (Some(v), Some(e)) = (value.as_object_mut(), extra.as_object()) {
        v.extend(e.clone());
    }
    if ctx.json {
        json_line(out, &value)
    } else {
        text(
            out,
            &format!("{action}: {}\n", crate::text::one_line(target)),
        )
    }
}

pub(super) fn status_action(
    ctx: &Ctx,
    out: &mut dyn Write,
    typed: &str,
    action: &str,
) -> Result<()> {
    let id = own_status(ctx, typed)?;
    let (scope, endpoint) = match action {
        "like" => ("write:favourites", "favourite"),
        "unlike" => ("write:favourites", "unfavourite"),
        "repost" => ("write:statuses", "reblog"),
        "unrepost" => ("write:statuses", "unreblog"),
        "bookmark" => ("write:bookmarks", "bookmark"),
        "unbookmark" => ("write:bookmarks", "unbookmark"),
        _ => ("write:statuses", "delete"),
    };
    let client = ctx.writer(&[scope], action)?;
    if endpoint == "delete" {
        client.delete_status(&id)
    } else {
        client.act_on(&id, endpoint)
    }
    .map_err(|e| ctx.explained(e))?;
    wrote(ctx, out, action, &id, json!({}))
}

pub(super) fn relation(ctx: &Ctx, out: &mut dyn Write, actor: &str, action: &str) -> Result<()> {
    let scope = match action {
        "mute" | "unmute" => "write:mutes",
        "block" | "unblock" => "write:blocks",
        _ => "write:follows",
    };
    let client = ctx.writer(&[scope], action)?;
    let account = local_account(&client, actor)?;
    let relation = client
        .relate(&account.id, action)
        .map_err(|e| ctx.explained(e))?;
    let extra = if action == "follow" {
        json!({"requested": relation.requested})
    } else {
        json!({})
    };
    wrote(ctx, out, action, &account.id, extra)
}

fn print_accounts(
    ctx: &Ctx,
    out: &mut dyn Write,
    client: &Client,
    accounts: &[Account],
) -> Result<()> {
    for a in accounts {
        if ctx.json {
            json_line(out, &format::account_json(a, client.origin()))?;
        } else {
            text(out, &format::account(a, client.origin()))?;
        }
    }
    Ok(())
}

fn account_pages(
    ctx: &Ctx,
    out: &mut dyn Write,
    client: &Client,
    path: &str,
    limit: usize,
) -> Result<()> {
    let all = pages(
        limit,
        false,
        |_| true,
        |n, cursor| client.account_list(path, n, cursor),
    )?;
    print_accounts(ctx, out, client, &all)
}

pub(super) fn status_accounts(
    ctx: &Ctx,
    out: &mut dyn Write,
    typed: &str,
    kind: &str,
    limit: usize,
) -> Result<()> {
    let (origin, id) = parse_status(typed)?;
    let client = ctx.client(origin.as_deref())?;
    account_pages(
        ctx,
        out,
        &client,
        &format!("v1/statuses/{}/{kind}", api::percent_encode(&id)),
        limit,
    )
}

pub(super) fn collection(ctx: &Ctx, out: &mut dyn Write, kind: &str, limit: usize) -> Result<()> {
    let client = ctx.account_client()?;
    let path = format!("v1/{kind}");
    if matches!(kind, "mutes" | "blocks") {
        return account_pages(ctx, out, &client, &path, limit);
    }
    let all = pages(
        limit,
        false,
        |_| true,
        |n, cursor| client.timeline(&path, &[], n, cursor),
    )?;
    print_statuses(ctx, out, client.origin(), &all)
}

pub(super) fn follows(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    limit: usize,
    followers: bool,
    pending: bool,
) -> Result<()> {
    let client = if pending || actor.is_none() {
        ctx.account_client()?
    } else {
        ctx.client(None)?
    };
    if pending {
        if actor.is_some() {
            return Err(Error::usage("--pending lists requests to your own account"));
        }
        return account_pages(ctx, out, &client, "v1/follow_requests", limit);
    }
    let a = match actor {
        Some(a) => local_account(&client, a)?,
        None => client.me()?,
    };
    let side = if followers { "followers" } else { "following" };
    account_pages(
        ctx,
        out,
        &client,
        &format!("v1/accounts/{}/{side}", api::percent_encode(&a.id)),
        limit,
    )
}

pub(super) fn search_accounts(
    ctx: &Ctx,
    out: &mut dyn Write,
    query: &str,
    limit: usize,
) -> Result<()> {
    if query.trim().is_empty() {
        return Err(Error::usage("the search query is empty"));
    }
    let client = ctx.account_client()?;
    let mut all = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0;
    while all.len() < limit {
        let found = client.search(
            query,
            "accounts",
            false,
            (limit - all.len()).min(api::MAX_PAGE),
            offset,
        )?;
        offset += found.accounts.len();
        let before = all.len();
        for account in found.accounts {
            if seen.insert(account.id.clone()) {
                all.push(account);
            }
        }
        if all.len() == before {
            break;
        }
    }
    all.truncate(limit);
    print_accounts(ctx, out, &client, &all)
}

pub(super) fn lists(ctx: &Ctx, out: &mut dyn Write, limit: usize) -> Result<()> {
    for list in ctx.account_client()?.lists()?.into_iter().take(limit) {
        if ctx.json {
            json_line(out, &json!({"id": list.id, "name": list.title}))?;
        } else {
            text(
                out,
                &format!(
                    "{}  {}\n",
                    crate::text::one_line(&list.id),
                    crate::text::one_line(&list.title)
                ),
            )?;
        }
    }
    Ok(())
}

pub(super) fn list(ctx: &Ctx, out: &mut dyn Write, id: &str, limit: usize) -> Result<()> {
    let client = ctx.account_client()?;
    let path = format!("v1/timelines/list/{}", api::percent_encode(id));
    let all = pages(
        limit,
        false,
        |_| true,
        |n, cursor| client.timeline(&path, &[], n, cursor),
    )?;
    print_statuses(ctx, out, client.origin(), &all)
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum ReportReason {
    Spam,
    Violation,
    Misleading,
    Sexual,
    Rude,
    Other,
}
impl ReportReason {
    fn category(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Violation => "violation",
            _ => "other",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Spam => "Spam",
            Self::Violation => "Breaks the server's rules or the law",
            Self::Misleading => "Misleading",
            Self::Sexual => "Sexual content not marked as sensitive",
            Self::Rude => "Rude or harassing",
            Self::Other => "Other",
        }
    }
}
pub(super) struct ReportArgs {
    pub target: String,
    pub reason: ReportReason,
    pub comment: Option<String>,
    pub yes: bool,
}

pub(super) fn report(
    ctx: &Ctx,
    out: &mut dyn Write,
    args: &ReportArgs,
    input: &mut dyn BufRead,
    asked: Option<&mut dyn Write>,
) -> Result<()> {
    let comment = match args
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(c) => format!("{}\n-----\n{c}", args.reason.label()),
        None => args.reason.label().to_string(),
    };
    if comment.chars().count() > 1000 {
        return Err(Error::usage("the report comment exceeds 1000 characters"));
    }
    if !args.yes {
        let Some(err) = asked else {
            return Err(Error::usage("a report is sent only once confirmed")
                .with_hint("pass --yes, or answer y on a terminal"));
        };
        if !terminal_kit::confirm::ask(input, err, "Report to the server's moderators? [y/N] ")
            .map_err(|e| Error::io(format!("cannot confirm: {e}")))?
        {
            return Err(Error::usage("not reported"));
        }
    }
    let client = ctx.writer(&["write:reports"], "report")?;
    let (account, status) = if parse_status(&args.target).is_ok() {
        let id = own_status(ctx, &args.target)?;
        let status = client.status(&id)?;
        (status.account.id, Some(id))
    } else {
        (local_account(&client, &args.target)?.id, None)
    };
    client
        .categorized_report(
            &account,
            status.as_deref(),
            &comment,
            args.reason.category(),
        )
        .map_err(|e| ctx.explained(e))?;
    wrote(ctx, out, "report", &account, json!({"comment": comment}))
}

fn message(s: &Status, origin: &str) -> Value {
    json!({"id": s.id, "createdAt": s.created_at, "user": format::account_json(&s.account, origin), "text": crate::html::to_text(&s.content)})
}

pub(super) fn chat(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    body: Option<&str>,
    limit: usize,
    input: &mut dyn BufRead,
) -> Result<()> {
    let client = if body.is_some() {
        ctx.writer(&["write:statuses"], "send direct posts")?
    } else {
        ctx.account_client()?
    };
    let Some(actor) = actor else {
        let all = pages(
            limit,
            false,
            |_| true,
            |_, cursor| client.conversations(cursor),
        )?;
        for c in all {
            if ctx.json {
                json_line(
                    out,
                    &json!({"id": c.id, "members": c.accounts.iter().map(|a| format::account_json(a, client.origin())).collect::<Vec<_>>(), "unreadCount": null, "unread": c.unread, "muted": null, "lastMessage": c.last_status.as_ref().map(|s| message(s, client.origin()))}),
                )?;
            } else if let Some(s) = c.last_status {
                text(out, &format::status(&s, client.origin()))?;
            }
        }
        return Ok(());
    };
    let account = local_account(&client, actor)?;
    if let Some(body) = body {
        let body = if body == "-" {
            let mut s = String::new();
            input
                .read_to_string(&mut s)
                .map_err(|e| Error::io(format!("cannot read message: {e}")))?;
            s
        } else {
            body.into()
        };
        if body.trim().is_empty() {
            return Err(Error::usage("the message is empty"));
        }
        let args = PostArgs {
            text: format!("@{} {}", account.acct, body.trim()),
            images: vec![],
            alts: vec![],
            video: None,
            reply: None,
            quote: None,
            cw: None,
            sensitive: false,
            visibility: Visibility::Direct,
            dry_run: false,
        };
        let made = post::composed(&client, &args, &json!({}))?;
        return if ctx.json {
            json_line(out, &message(&made, client.origin()))
        } else {
            text(out, &format::status(&made, client.origin()))
        };
    }
    // Conversations can be group threads. Do not mix them into one-to-one history.
    let conversations = pages(
        usize::MAX,
        false,
        |_| true,
        |_, cursor| client.conversations(cursor),
    )?;
    let me = client.me()?;
    let mut statuses = Vec::new();
    let mut seen = HashSet::new();
    for c in conversations
        .into_iter()
        .filter(|c| c.accounts.len() == 1 && c.accounts[0].id == account.id)
    {
        if let Some(s) = c.last_status.filter(|s| s.visibility == "direct") {
            let context = client.context(&s.id)?;
            for s in context
                .ancestors
                .into_iter()
                .chain(std::iter::once(s))
                .chain(context.descendants)
            {
                if api::same_conversation(
                    &s,
                    std::slice::from_ref(&account),
                    Some((&me.id, &me.acct)),
                ) && seen.insert(s.id.clone())
                {
                    statuses.push(s);
                }
            }
        }
    }
    statuses.sort_by(|a, b| a.id.len().cmp(&b.id.len()).then(a.id.cmp(&b.id)));
    for s in statuses.iter().skip(statuses.len().saturating_sub(limit)) {
        if ctx.json {
            json_line(out, &message(s, client.origin()))?;
        } else {
            text(out, &format::status(s, client.origin()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
