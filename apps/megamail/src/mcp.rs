//! `megamail mcp`: a Model Context Protocol server over stdio.
//!
//! Gives AI agents the same mailboxes the app shows — every saved account,
//! through the same per-account mail worker, cache and keyring credentials.
//! No password is copied anywhere: the server reads MegaMail's own profile.
//!
//! Protocol: newline-delimited JSON-RPC 2.0 on stdin/stdout (MCP stdio
//! transport). Logs go to stderr only; stdout carries nothing but replies.
//!
//! `--read-only` hides every tool that sends, saves, moves or flags mail.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use megamail_core::config::{self, AccountConfig};
use megamail_core::models::{DraftOrigin, Folder, FolderKind, Message};
use megamail_core::worker::{self, MailRequest, OutgoingMessage, SendOutcome, WorkerEvent};
use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2025-06-18";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(45);
const OP_TIMEOUT: Duration = Duration::from_secs(60);
const SYNC_GRACE: Duration = Duration::from_secs(12);
const BODY_LIMIT: usize = 60_000;

type WorkerTx = tokio::sync::mpsc::UnboundedSender<MailRequest>;

struct Mailbox {
    id: u32,
    config: AccountConfig,
    tx: Option<WorkerTx>,
    rx: Option<Receiver<WorkerEvent>>,
    folders: Vec<Folder>,
    /// Every message seen in a listing, by (folder path, uid).
    messages: HashMap<(String, u32), Message>,
    last_error: Option<String>,
}

impl Mailbox {
    fn label(&self) -> String {
        format!("{} <{}>", self.config.name, self.config.email)
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.tx.is_some() && !self.folders.is_empty() {
            return Ok(());
        }
        if self.tx.is_none() {
            let (event_tx, event_rx) = channel();
            let tx = worker::spawn(self.id, Some(self.config.clone()), move |event| {
                let _ = event_tx.send(event);
            });
            self.tx = Some(tx);
            self.rx = Some(event_rx);
        }
        self.wait(CONNECT_TIMEOUT, |_, mb| (!mb.folders.is_empty()).then_some(()))
            .map_err(|e| format!("{}: could not load folders ({e})", self.config.email))
    }

    fn send(&self, request: MailRequest) -> Result<(), String> {
        self.tx
            .as_ref()
            .ok_or("account is not connected")?
            .send(request)
            .map_err(|_| "the mail worker stopped".to_owned())
    }

    /// Pump worker events until `pick` returns a value. Folder lists and
    /// message listings are recorded on the way, whatever they answer.
    fn wait<T>(
        &mut self,
        timeout: Duration,
        mut pick: impl FnMut(&WorkerEvent, &Mailbox) -> Option<T>,
    ) -> Result<T, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(self
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "timed out waiting for the mail server".into()));
            }
            let event = match self.rx.as_ref().ok_or("not connected")?.recv_timeout(left) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Err("the mail worker stopped".into()),
            };
            self.observe(&event);
            if let Some(value) = pick(&event, self) {
                return Ok(value);
            }
        }
    }

    fn observe(&mut self, event: &WorkerEvent) {
        match event {
            WorkerEvent::Folders(folders) if !folders.is_empty() => self.folders = folders.clone(),
            WorkerEvent::Messages { messages, .. } | WorkerEvent::MessagesAppend { messages, .. } => {
                for m in messages {
                    if let Some(path) = self.folder_path(m.folder_id) {
                        self.messages.insert((path, m.uid), m.clone());
                    }
                }
            }
            WorkerEvent::Error { text, .. } => {
                tracing::warn!(account = %self.config.email, "{text}");
                self.last_error = Some(text.clone());
            }
            _ => {}
        }
    }

    fn folder_path(&self, id: u32) -> Option<String> {
        self.folders.iter().find(|f| f.id == id).map(|f| f.path.clone())
    }

    fn folder(&self, name: &str) -> Result<Folder, String> {
        let lower = name.to_lowercase();
        let by_kind = match lower.as_str() {
            "inbox" => Some(FolderKind::Inbox),
            "sent" => Some(FolderKind::Sent),
            "drafts" | "draft" => Some(FolderKind::Drafts),
            "archive" | "all mail" => Some(FolderKind::Archive),
            "trash" | "deleted" | "bin" => Some(FolderKind::Trash),
            "spam" | "junk" => Some(FolderKind::Junk),
            "starred" => Some(FolderKind::Starred),
            _ => None,
        };
        self.folders
            .iter()
            .find(|f| f.path == name)
            .or_else(|| by_kind.and_then(|k| self.folders.iter().find(|f| f.kind == k)))
            .or_else(|| self.folders.iter().find(|f| f.name.to_lowercase() == lower || f.path.to_lowercase() == lower))
            .cloned()
            .ok_or_else(|| format!("no folder named {name:?} in {}", self.config.email))
    }

    fn kind(&self, kind: FolderKind) -> Option<Folder> {
        self.folders.iter().find(|f| f.kind == kind).cloned()
    }

    /// Fresh listing of one folder, newest first.
    fn list(&mut self, folder: &Folder) -> Result<Vec<Message>, String> {
        self.connect()?;
        self.last_error = None;
        self.send(MailRequest::LoadMessages { folder_id: folder.id, path: folder.path.clone() })?;
        let id = folder.id;
        let mut got: Option<Vec<Message>> = None;
        // The cache answers first; give the server a moment to bring news.
        let first = self.wait(OP_TIMEOUT, |e, _| match e {
            WorkerEvent::Messages { folder_id, messages } if *folder_id == id => Some(messages.clone()),
            _ => None,
        })?;
        got.replace(first);
        let _ = self.wait(SYNC_GRACE, |e, _| match e {
            WorkerEvent::FolderSynced { folder_id } if *folder_id == id => Some(()),
            WorkerEvent::Messages { folder_id, messages } if *folder_id == id => {
                got.replace(messages.clone());
                None
            }
            _ => None,
        });
        let mut list = got.unwrap_or_default();
        list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(list)
    }

    fn message(&mut self, folder: &Folder, uid: u32) -> Result<Message, String> {
        if let Some(m) = self.messages.get(&(folder.path.clone(), uid)) {
            return Ok(m.clone());
        }
        self.list(folder)?;
        self.messages
            .get(&(folder.path.clone(), uid))
            .cloned()
            .ok_or_else(|| format!("no message with uid {uid} in {}", folder.path))
    }

    fn body(&mut self, folder: &Folder, uid: u32) -> Result<(Message, String), String> {
        let message = self.message(folder, uid)?;
        let mid = message.id;
        self.send(MailRequest::LoadBody { message_id: mid, path: folder.path.clone(), uid })?;
        let body = self.wait(OP_TIMEOUT, |e, _| match e {
            WorkerEvent::Body { message_id, body, .. } if *message_id == mid => Some(Ok(body.clone())),
            WorkerEvent::Gone { message_id, .. } if *message_id == mid => Some(Err("the message is gone from the server".to_owned())),
            _ => None,
        })??;
        Ok((message, html_to_text(&body)))
    }

    /// Queue `request` and wait until the worker has carried it out.
    fn mutate(&mut self, request: MailRequest, path: &str, uids: Vec<u32>) -> Result<(), String> {
        self.connect()?;
        self.last_error = None;
        self.send(request)?;
        self.send(MailRequest::Settle { path: path.to_owned(), uids })?;
        let p = path.to_owned();
        self.wait(OP_TIMEOUT, |e, _| matches!(e, WorkerEvent::MovesSettled { path, .. } if *path == p).then_some(()))?;
        match self.last_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn send_mail(&mut self, mut message: OutgoingMessage) -> Result<&'static str, String> {
        self.connect()?;
        self.last_error = None;
        message.from_account_id = self.id;
        let request_id = next_request_id();
        let sent_path = self.kind(FolderKind::Sent).map(|f| f.path);
        self.send(MailRequest::Send { message: Box::new(message), sent_path, request_id: Some(request_id) })?;
        self.wait(OP_TIMEOUT * 2, |e, _| match e {
            WorkerEvent::SendFinished { request_id: r, outcome } if *r == request_id => Some(match outcome {
                SendOutcome::Sent => Ok("sent"),
                SendOutcome::Queued => Ok("queued in the Outbox (scheduled or will retry)"),
                SendOutcome::Failed => Err(()),
            }),
            WorkerEvent::SendUnknown { request_id: r, .. } if *r == request_id => Some(Ok("handed to the server; delivery unconfirmed")),
            _ => None,
        })?
        .map_err(|_| self.last_error.clone().unwrap_or_else(|| "send failed".into()))
    }

    fn save_draft(&mut self, mut message: OutgoingMessage) -> Result<(), String> {
        self.connect()?;
        self.last_error = None;
        message.from_account_id = self.id;
        let drafts = self.kind(FolderKind::Drafts).ok_or("this account has no Drafts folder")?;
        self.send(MailRequest::SaveDraft {
            message: Box::new(message),
            folder_id: drafts.id,
            path: drafts.path,
            autosave: None,
            template: false,
        })?;
        self.wait(OP_TIMEOUT, |e, mb| match e {
            WorkerEvent::DraftSaved { .. } => Some(Ok(())),
            WorkerEvent::Error { .. } => Some(Err(mb.last_error.clone().unwrap_or_default())),
            _ => None,
        })?
    }
}

fn next_request_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    0x4d43_5000_0000_0000 | NEXT.fetch_add(1, Ordering::Relaxed)
}

fn blank_message() -> OutgoingMessage {
    OutgoingMessage {
        client_message_id: None,
        from_account_id: 0,
        from_alias: None,
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        reply_to: String::new(),
        subject: String::new(),
        body: String::new(),
        html: String::new(),
        attachments: Vec::new(),
        in_reply_to: String::new(),
        references: String::new(),
        draft_origin: None,
        outbox_origin: None,
        sign: false,
        encrypt: false,
        send_at: None,
        calendar: None,
    }
}

/// Plain text an agent can read: tags dropped, blocks on their own lines.
fn html_to_text(body: &str) -> String {
    if !body.contains('<') {
        return truncate(body.trim().to_owned());
    }
    let mut out = String::with_capacity(body.len() / 2);
    let mut rest = body;
    let mut skip_until: Option<&str> = None;
    while let Some(start) = rest.find('<') {
        if skip_until.is_none() {
            out.push_str(&rest[..start]);
        }
        let Some(end) = rest[start..].find('>') else { break };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        match skip_until {
            Some(close) if tag.starts_with('/') && name == close => skip_until = None,
            Some(_) => {}
            None if !tag.starts_with('/') && (name == "style" || name == "script" || name == "head" || name == "title") => {
                skip_until = Some(match name.as_str() { "style" => "style", "script" => "script", "head" => "head", _ => "title" })
            }
            None => match name.as_str() {
                "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "blockquote" | "table" => {
                    out.push('\n');
                    if name == "li" && !tag.starts_with('/') {
                        out.push_str("• ");
                    }
                }
                "td" | "th" => out.push(' '),
                _ => {}
            },
        }
        rest = &rest[start + end + 1..];
    }
    if skip_until.is_none() {
        out.push_str(rest);
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    let mut text = String::new();
    let mut blank = 0;
    for line in decoded.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        text.push_str(line.trim_start_matches(char::is_whitespace));
        text.push('\n');
    }
    truncate(text.trim().to_owned())
}

fn truncate(mut text: String) -> String {
    if text.len() > BODY_LIMIT {
        let mut cut = BODY_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n…[truncated]");
    }
    text
}

fn summary(account: &Mailbox, folder: &str, m: &Message) -> Value {
    json!({
        "account": account.config.email,
        "folder": folder,
        "uid": m.uid,
        "from": if m.from_name.is_empty() { m.from_addr.clone() } else { format!("{} <{}>", m.from_name, m.from_addr) },
        "to": m.to,
        "subject": m.subject,
        "date": m.date,
        "timestamp": m.timestamp,
        "unread": m.unread,
        "starred": m.starred,
        "attachments": m.has_attachment,
        "preview": m.preview,
    })
}

struct Server {
    accounts: Vec<Mailbox>,
    read_only: bool,
}

impl Server {
    fn account(&mut self, key: Option<&str>) -> Result<&mut Mailbox, String> {
        let key = match key {
            Some(k) if !k.trim().is_empty() => k.trim().to_lowercase(),
            _ if self.accounts.len() == 1 => return Ok(&mut self.accounts[0]),
            _ => return Err(format!("several accounts are configured; pass \"account\" (one of: {})", self.emails())),
        };
        let emails = self.emails();
        self.accounts
            .iter_mut()
            .find(|a| a.config.email.to_lowercase() == key || a.config.name.to_lowercase() == key || a.id.to_string() == key)
            .ok_or_else(|| format!("unknown account {key:?}; known: {emails}"))
    }

    fn emails(&self) -> String {
        self.accounts.iter().map(|a| a.config.email.as_str()).collect::<Vec<_>>().join(", ")
    }

    fn tools(&self) -> Value {
        let s = |t: &str| json!({ "type": "string", "description": t });
        let account = s("Account email or name. Optional when only one account is configured.");
        let folder = s("Folder path or role: inbox, sent, drafts, archive, trash, spam, or a path from list_folders. Default inbox.");
        let uid = json!({ "type": "integer", "description": "Message UID from a listing." });
        let mut tools = vec![
            json!({ "name": "list_accounts", "description": "List the mail accounts MegaMail has configured.",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "readOnlyHint": true } }),
            json!({ "name": "list_folders", "description": "List an account's folders with unread counts.",
                "inputSchema": { "type": "object", "properties": { "account": account } },
                "annotations": { "readOnlyHint": true } }),
            json!({ "name": "list_messages", "description": "Newest messages in a folder. Omit account to get a unified view across every account.",
                "inputSchema": { "type": "object", "properties": {
                    "account": account, "folder": folder,
                    "limit": { "type": "integer", "description": "Max results, default 25, max 200." },
                    "unread_only": { "type": "boolean" } } },
                "annotations": { "readOnlyHint": true } }),
            json!({ "name": "search_messages", "description": "Find messages whose sender, recipients, subject or preview contain every word of the query (case-insensitive). Searches the synced folder listing; omit account to search all accounts.",
                "inputSchema": { "type": "object", "required": ["query"], "properties": {
                    "query": s("Words to match, e.g. 'invoice march'. Prefix with from: to match only the sender."),
                    "account": account, "folder": folder,
                    "limit": { "type": "integer" } } },
                "annotations": { "readOnlyHint": true } }),
            json!({ "name": "read_message", "description": "Full message: headers and plain-text body.",
                "inputSchema": { "type": "object", "required": ["uid"], "properties": { "account": account, "folder": folder, "uid": uid } },
                "annotations": { "readOnlyHint": true } }),
        ];
        if !self.read_only {
            let compose = json!({
                "account": account,
                "to": s("Comma-separated recipients."),
                "cc": s("Comma-separated."), "bcc": s("Comma-separated."),
                "subject": s("Subject line."), "body": s("Plain-text body."),
                "send_at": { "type": "integer", "description": "Optional unix time to send later." } });
            tools.extend([
                json!({ "name": "send_message", "description": "Send a new email from an account. Delivered immediately unless send_at is set.",
                    "inputSchema": { "type": "object", "required": ["to", "subject", "body"], "properties": compose },
                    "annotations": { "destructiveHint": false, "openWorldHint": true } }),
                json!({ "name": "reply", "description": "Reply (or reply-all) to a message, threaded correctly, quoting the original.",
                    "inputSchema": { "type": "object", "required": ["uid", "body"], "properties": {
                        "account": account, "folder": folder, "uid": uid, "body": s("Your reply text."),
                        "reply_all": { "type": "boolean" },
                        "draft": { "type": "boolean", "description": "Save to Drafts instead of sending." } } },
                    "annotations": { "openWorldHint": true } }),
                json!({ "name": "forward", "description": "Forward a message with an optional note.",
                    "inputSchema": { "type": "object", "required": ["uid", "to"], "properties": {
                        "account": account, "folder": folder, "uid": uid, "to": s("Recipients."), "body": s("Note above the forwarded message."),
                        "draft": { "type": "boolean" } } },
                    "annotations": { "openWorldHint": true } }),
                json!({ "name": "save_draft", "description": "Create a draft, or replace an existing draft when draft_uid is given (that is how drafts are edited).",
                    "inputSchema": { "type": "object", "properties": {
                        "account": account, "to": s("Recipients."), "cc": s("Cc."), "bcc": s("Bcc."), "subject": s("Subject."), "body": s("Body."),
                        "draft_uid": { "type": "integer", "description": "UID of a draft in the Drafts folder to replace." } } } }),
                json!({ "name": "update_messages", "description": "Mark read/unread, star/unstar, archive, trash, mark spam, or move messages.",
                    "inputSchema": { "type": "object", "required": ["uids", "action"], "properties": {
                        "account": account, "folder": folder,
                        "uids": { "type": "array", "items": { "type": "integer" } },
                        "action": { "type": "string", "enum": ["read", "unread", "star", "unstar", "archive", "trash", "spam", "move"] },
                        "destination": s("Folder for action=move.") } },
                    "annotations": { "destructiveHint": true } }),
            ]);
        }
        json!({ "tools": tools })
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        let str_arg = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_owned);
        let account_key = str_arg("account");
        let folder_name = str_arg("folder").unwrap_or_else(|| "inbox".into());
        let uid = || args.get("uid").and_then(Value::as_u64).map(|u| u as u32).ok_or("uid is required");
        let writes = ["send_message", "reply", "forward", "save_draft", "update_messages"];
        if self.read_only && writes.contains(&name) {
            return Err("MegaMail MCP is running read-only".into());
        }
        match name {
            "list_accounts" => Ok(json!(self.accounts.iter().map(|a| json!({
                "id": a.id, "name": a.config.name, "email": a.config.email,
                "aliases": a.config.aliases.iter().map(|x| x.address()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>())),
            "list_folders" => {
                let mb = self.account(account_key.as_deref())?;
                mb.connect()?;
                Ok(json!(mb.folders.iter().map(|f| json!({ "path": f.path, "name": f.name, "role": format!("{:?}", f.kind).to_lowercase(), "unread": f.unread })).collect::<Vec<_>>()))
            }
            "list_messages" | "search_messages" => {
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(25).clamp(1, 200) as usize;
                let unread_only = args.get("unread_only").and_then(Value::as_bool).unwrap_or(false);
                let query = str_arg("query").unwrap_or_default().to_lowercase();
                let (from_only, query) = match query.strip_prefix("from:") {
                    Some(q) => (true, q.trim().to_owned()),
                    None => (false, query),
                };
                let words: Vec<&str> = query.split_whitespace().collect();
                let indices: Vec<usize> = match account_key.as_deref() {
                    Some(k) if !k.trim().is_empty() => {
                        let id = self.account(Some(k))?.id;
                        vec![self.accounts.iter().position(|a| a.id == id).unwrap_or(0)]
                    }
                    _ => (0..self.accounts.len()).collect(),
                };
                let mut rows: Vec<(i64, Value)> = Vec::new();
                let mut problems = Vec::new();
                for i in indices {
                    let mb = &mut self.accounts[i];
                    let result = mb.connect().and_then(|_| mb.folder(&folder_name)).and_then(|f| mb.list(&f).map(|l| (f, l)));
                    match result {
                        Ok((folder, list)) => {
                            for m in list {
                                if unread_only && !m.unread {
                                    continue;
                                }
                                let hay = if from_only {
                                    format!("{} {}", m.from_name, m.from_addr).to_lowercase()
                                } else {
                                    format!("{} {} {} {} {} {}", m.from_name, m.from_addr, m.to, m.cc, m.subject, m.preview).to_lowercase()
                                };
                                if words.iter().all(|w| hay.contains(w)) {
                                    rows.push((m.timestamp, summary(mb, &folder.path, &m)));
                                }
                            }
                        }
                        Err(e) => problems.push(format!("{}: {e}", mb.label())),
                    }
                }
                rows.sort_by(|a, b| b.0.cmp(&a.0));
                let total = rows.len();
                let messages: Vec<Value> = rows.into_iter().take(limit).map(|r| r.1).collect();
                Ok(json!({ "total_matches": total, "messages": messages, "errors": problems }))
            }
            "read_message" => {
                let uid = uid()?;
                let mb = self.account(account_key.as_deref())?;
                mb.connect()?;
                let folder = mb.folder(&folder_name)?;
                let (m, body) = mb.body(&folder, uid)?;
                let mut value = summary(mb, &folder.path, &m);
                value["cc"] = json!(m.cc);
                value["reply_to"] = json!(m.reply_to);
                value["message_id"] = json!(m.message_id);
                value["body"] = json!(body);
                if m.unread {
                    let _ = mb.mutate(MailRequest::SetSeen { path: folder.path.clone(), uid, seen: true }, &folder.path, vec![uid]);
                }
                Ok(value)
            }
            "send_message" | "save_draft" => {
                let mb = self.account(account_key.as_deref())?;
                mb.connect()?;
                let mut out = blank_message();
                out.to = str_arg("to").unwrap_or_default();
                out.cc = str_arg("cc").unwrap_or_default();
                out.bcc = str_arg("bcc").unwrap_or_default();
                out.subject = str_arg("subject").unwrap_or_default();
                out.body = str_arg("body").unwrap_or_default();
                out.send_at = args.get("send_at").and_then(Value::as_i64);
                if name == "send_message" {
                    if out.to.trim().is_empty() {
                        return Err("to is required".into());
                    }
                    if let Some(bad) = worker::invalid_recipient(&format!("{},{},{}", out.to, out.cc, out.bcc).trim_matches(',').replace(",,", ",")) {
                        return Err(format!("invalid recipient: {bad}"));
                    }
                    let status = mb.send_mail(out)?;
                    return Ok(json!({ "status": status, "from": mb.config.email }));
                }
                if let Some(draft_uid) = args.get("draft_uid").and_then(Value::as_u64) {
                    let drafts = mb.kind(FolderKind::Drafts).ok_or("this account has no Drafts folder")?;
                    out.draft_origin = Some(DraftOrigin { account_id: mb.id, folder_id: drafts.id, path: drafts.path, uid: draft_uid as u32 });
                }
                mb.save_draft(out)?;
                Ok(json!({ "status": "draft saved", "account": mb.config.email }))
            }
            "reply" | "forward" => {
                let uid = uid()?;
                let as_draft = args.get("draft").and_then(Value::as_bool).unwrap_or(false);
                let note = str_arg("body").unwrap_or_default();
                let mb = self.account(account_key.as_deref())?;
                mb.connect()?;
                let folder = mb.folder(&folder_name)?;
                let (m, original) = mb.body(&folder, uid)?;
                let own: Vec<String> = std::iter::once(mb.config.email.to_lowercase())
                    .chain(mb.config.aliases.iter().map(|a| a.address().to_lowercase()))
                    .collect();
                let sender = if m.from_name.is_empty() { m.from_addr.clone() } else { format!("{} <{}>", m.from_name, m.from_addr) };
                let mut out = blank_message();
                if name == "reply" {
                    out.to = if m.reply_to.trim().is_empty() { m.from_addr.clone() } else { m.reply_to.clone() };
                    if args.get("reply_all").and_then(Value::as_bool).unwrap_or(false) {
                        let others: Vec<String> = format!("{},{}", m.to, m.cc)
                            .split(',')
                            .map(|a| a.trim().to_owned())
                            .filter(|a| !a.is_empty() && !own.iter().any(|o| a.to_lowercase().contains(o.as_str())) && !out.to.to_lowercase().contains(&a.to_lowercase()))
                            .collect();
                        out.cc = others.join(", ");
                    }
                    out.subject = if m.subject.to_lowercase().starts_with("re:") { m.subject.clone() } else { format!("Re: {}", m.subject) };
                    out.in_reply_to = m.message_id.clone();
                    out.references = format!("{} {}", m.references, m.message_id).trim().to_owned();
                    let quoted: String = original.lines().map(|l| format!("> {l}\n")).collect();
                    out.body = format!("{note}\n\nOn {}, {sender} wrote:\n{quoted}", m.date);
                } else {
                    out.to = str_arg("to").ok_or("to is required")?;
                    out.subject = if m.subject.to_lowercase().starts_with("fwd:") { m.subject.clone() } else { format!("Fwd: {}", m.subject) };
                    out.body = format!(
                        "{note}\n\n---------- Forwarded message ----------\nFrom: {sender}\nDate: {}\nSubject: {}\nTo: {}\n\n{original}",
                        m.date, m.subject, m.to
                    );
                }
                if as_draft {
                    mb.save_draft(out)?;
                    return Ok(json!({ "status": "draft saved" }));
                }
                let status = mb.send_mail(out)?;
                if name == "reply" {
                    let _ = mb.mutate(MailRequest::SetSeen { path: folder.path.clone(), uid, seen: true }, &folder.path, vec![uid]);
                }
                Ok(json!({ "status": status }))
            }
            "update_messages" => {
                let action = str_arg("action").ok_or("action is required")?;
                let uids: Vec<u32> = args
                    .get("uids")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_u64).map(|u| u as u32).collect())
                    .unwrap_or_default();
                if uids.is_empty() {
                    return Err("uids is required".into());
                }
                let mb = self.account(account_key.as_deref())?;
                mb.connect()?;
                let folder = mb.folder(&folder_name)?;
                let path = folder.path.clone();
                let dest = match action.as_str() {
                    "archive" => Some(mb.kind(FolderKind::Archive).ok_or("this account has no Archive folder")?.path),
                    "trash" => Some(mb.kind(FolderKind::Trash).ok_or("this account has no Trash folder")?.path),
                    "spam" => Some(mb.kind(FolderKind::Junk).ok_or("this account has no Spam folder")?.path),
                    "move" => Some(mb.folder(&str_arg("destination").ok_or("destination is required for move")?)?.path),
                    _ => None,
                };
                match (action.as_str(), dest) {
                    ("spam", Some(dest)) => {
                        for &uid in &uids {
                            mb.send(MailRequest::MarkSpam { path: path.clone(), uid, dest: dest.clone() })?;
                        }
                        mb.mutate(MailRequest::RefreshUnread, &path, uids.clone())?;
                    }
                    (_, Some(dest)) => mb.mutate(MailRequest::MoveMessages { path: path.clone(), uids: uids.clone(), dest }, &path, uids.clone())?,
                    ("read" | "unread", None) => {
                        for &uid in &uids {
                            mb.send(MailRequest::SetSeen { path: path.clone(), uid, seen: action == "read" })?;
                        }
                        mb.mutate(MailRequest::RefreshUnread, &path, uids.clone())?;
                    }
                    ("star" | "unstar", None) => {
                        for &uid in &uids {
                            mb.send(MailRequest::SetFlagged { path: path.clone(), uid, flagged: action == "star" })?;
                        }
                        mb.mutate(MailRequest::RefreshUnread, &path, uids.clone())?;
                    }
                    _ => return Err(format!("unknown action {action:?}")),
                }
                for uid in &uids {
                    mb.messages.remove(&(path.clone(), *uid));
                }
                Ok(json!({ "status": "done", "action": action, "count": uids.len() }))
            }
            other => Err(format!("unknown tool {other:?}")),
        }
    }
}

/// Entry point for `megamail mcp [--read-only]`.
pub(crate) fn run(read_only: bool) -> i32 {
    let accounts = match config::load_profiles_existing() {
        Ok(list) => list,
        Err(error) => {
            eprintln!("megamail mcp: could not read MegaMail's accounts: {error}");
            return 1;
        }
    };
    let accounts: Vec<Mailbox> = accounts
        .into_iter()
        .filter(|(_, c)| c.enabled)
        .map(|(id, config)| Mailbox { id, config, tx: None, rx: None, folders: Vec::new(), messages: HashMap::new(), last_error: None })
        .collect();
    eprintln!(
        "megamail mcp: {} account(s){}",
        accounts.len(),
        if read_only { ", read-only" } else { "" }
    );
    let mut server = Server { accounts, read_only };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(stdout, "{}", json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }));
                let _ = stdout.flush();
                continue;
            }
        };
        let Some(id) = request.get("id").cloned() else { continue }; // notification
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let reply = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "megamail", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Email for every account saved in MegaMail. Start with list_accounts or list_messages (no account = unified inbox). Messages are addressed by account + folder + uid. Edit a draft by calling save_draft with draft_uid.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(server.tools()),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                Ok(match server.call(&name, &args) {
                    Ok(value) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }], "structuredContent": if value.is_object() { value.clone() } else { json!({ "result": value }) } }),
                    Err(error) => json!({ "content": [{ "type": "text", "text": error }], "isError": true }),
                })
            }
            _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
        };
        let response = match reply {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        if writeln!(stdout, "{response}").and_then(|_| stdout.flush()).is_err() {
            break;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::html_to_text;

    #[test]
    fn html_becomes_readable_text() {
        let text = html_to_text("<html><head><style>p{}</style></head><body><p>Hi &amp; welcome</p><ul><li>One</li></ul><script>x()</script></body></html>");
        assert_eq!(text, "Hi & welcome\n\n• One");
    }
}

#[cfg(test)]
mod worker_flow {
    use super::*;

    /// Drives the offline sample worker through the same calls the tools make.
    #[test]
    fn list_read_and_mutate_against_the_sample_worker() {
        let (event_tx, event_rx) = channel();
        let tx = worker::spawn(1, None, move |event| {
            let _ = event_tx.send(event);
        });
        let mut mb = Mailbox {
            id: 1,
            config: AccountConfig::new("demo@example.com"),
            tx: Some(tx),
            rx: Some(event_rx),
            folders: Vec::new(),
            messages: HashMap::new(),
            last_error: None,
        };
        mb.connect().expect("folders");
        let inbox = mb.folder("inbox").expect("inbox");
        let list = mb.list(&inbox).expect("list");
        assert!(!list.is_empty());
        let (_, body) = mb.body(&inbox, list[0].uid).expect("body");
        assert!(!body.is_empty());
        mb.mutate(MailRequest::SetSeen { path: inbox.path.clone(), uid: list[0].uid, seen: true }, &inbox.path, vec![list[0].uid])
            .expect("settled");
    }
}
