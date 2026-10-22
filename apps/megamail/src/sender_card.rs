//! Sender actions shown with a message's details: save to the local contact
//! book, write to, search from, copy. Contacts are matched by address only,
//! never by display name, and nothing is looked up online.
use crate::{contacts, *};

impl MailApp {
    pub(crate) fn save_sender_contact(&mut self, account: u32, name: String, address: String, cx: &mut Context<Self>) {
        if self.demo {
            self.reader_export_status = Some("Contacts cannot be saved in the local demo.".into());
            cx.notify();
            return;
        }
        let backend = if self.live.read(cx).is_direct_account(account) {
            contacts::ContactBackend::Direct
        } else {
            contacts::ContactBackend::Thunderbird
        };
        let scope = contacts::ContactScope { backend, account_id: account };
        self.reader_export_status = Some("Saving contact…".into());
        cx.notify();
        let task = cx.background_spawn(async move {
            let root = config::config_base().ok_or("MegaMail's private storage is unavailable.")?;
            let normalized = contacts::normalize_address(&address).map_err(|_| "This address cannot be saved as a contact.".to_owned())?;
            let book = contacts::LocalContacts::open(&root, scope).map_err(|e| e.to_string())?;
            let snapshot = book.load().map_err(|e| e.to_string())?;
            if let Some(existing) = snapshot.contacts.iter().find(|c| c.fields.emails.iter().any(|e| e.eq_ignore_ascii_case(&normalized))) {
                return Ok(format!("{normalized} is already saved{}.", if existing.fields.display_name.is_empty() { String::new() } else { format!(" as {}", existing.fields.display_name) }));
            }
            let draft = contacts::ContactDraft { display_name: name.trim().to_owned(), emails: vec![normalized.clone()], notes: String::new() };
            book.apply(snapshot.revision, contacts::ContactChange::Create(draft)).map_err(|e| e.to_string())?;
            Ok::<String, String>(format!("Saved {normalized} to your contacts."))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.reader_export_status = Some(result.unwrap_or_else(|e| e));
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn compose_to(&mut self, account: u32, name: &str, address: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_replace_composer(cx) {
            return;
        }
        let mut message = compose::new_message(account);
        message.to = worker::format_recipient(name, address);
        let message = self.signed(message);
        self.composer = Some(Composer::new(message, window, cx));
        self.view = View::Compose;
        cx.notify();
    }

    pub(crate) fn search_from(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
        let query = format!("from:{address}");
        self.search.update(cx, |input, cx| input.set_value(query, window, cx));
        cx.notify();
    }
}
