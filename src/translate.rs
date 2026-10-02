//! Message translation (#327): the reader sends a message's text to a
//! translation service the user has an account with, under their own key,
//! and shows what comes back in place of the message. Four services:
//!
//! * **DeepL**: the v2 API, free or paid (a free key ends in `:fx` and
//!   goes to `api-free.deepl.com`).
//! * **Google Cloud Translation**: the v2 ("Basic") API with an API key.
//! * **Microsoft Translator**: the v3 API with a resource key, and the
//!   resource's region when it is not a global one.
//! * **LibreTranslate**: any server, self-hosted ones included, with a key
//!   only when the server asks for one.
//!
//! What is sent is the Reader View of the message ([`crate::reader`]):
//! its text and structure, without the sender's styling, tracking images
//! or scripts. Every service is asked to keep the markup, so paragraphs,
//! lists, links and quotes come back where they were.
//!
//! The settings live in `translation.toml`; the key is in the keyring.
//! Which language a message is in, for the optional "Translate" offer, is
//! worked out here, offline: nothing leaves the computer until the user
//! asks for a translation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::i18n::{i18n, i18n_f};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    #[default]
    Off,
    #[serde(rename = "deepl")]
    DeepL,
    Google,
    Microsoft,
    #[serde(rename = "libretranslate")]
    Libre,
}

impl Service {
    /// In the order the Settings picker lists them.
    pub const ALL: [Service; 5] = [Service::Off, Service::DeepL, Service::Google, Service::Microsoft, Service::Libre];

    /// The service's own name, which is not translated.
    pub fn name(self) -> String {
        match self {
            Service::Off => i18n("Off"),
            Service::DeepL => "DeepL".into(),
            Service::Google => "Google Cloud Translation".into(),
            Service::Microsoft => "Microsoft Translator".into(),
            Service::Libre => "LibreTranslate".into(),
        }
    }

    /// The keyring entry its key is kept under.
    fn key_name(self) -> String {
        let id = match self {
            Service::Off => "off",
            Service::DeepL => "deepl",
            Service::Google => "google",
            Service::Microsoft => "microsoft",
            Service::Libre => "libretranslate",
        };
        format!("translate:{id}")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub service: Service,
    /// The language to translate into, as in [`LANGUAGES`]; empty means the
    /// language Hylki is shown in.
    #[serde(default)]
    pub target: String,
    /// Microsoft: the region of the Translator resource ("westeurope");
    /// empty for a global resource.
    #[serde(default)]
    pub region: String,
    /// LibreTranslate: the server, e.g. `https://translate.example.org`.
    #[serde(default)]
    pub url: String,
    /// Offer a translation on messages in another language.
    #[serde(default)]
    pub offer: bool,
}

impl Settings {
    pub fn enabled(&self) -> bool {
        self.service != Service::Off
    }

    /// The language translations go into: the chosen one, else Hylki's own.
    pub fn target_language(&self) -> String {
        if self.target.is_empty() {
            ui_language()
        } else {
            self.target.clone()
        }
    }
}

/// The languages offered as a target, by the code this module uses for
/// them. Every service takes all of these; how each one spells them is
/// [`service_code`]'s business.
pub const LANGUAGES: &[&str] = &[
    "ar", "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "he", "hu", "id", "it", "ja", "ko", "lt",
    "lv", "nb", "nl", "pl", "pt-BR", "pt-PT", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi", "zh-Hans", "zh-Hant",
];

/// A language as it names itself ("Deutsch"), the way the spell checker's
/// language list shows them.
pub fn language_name(code: &str) -> String {
    match code {
        "ja" => "日本語".into(),
        "zh-Hans" | "zh" => "中文（简体）".into(),
        "zh-Hant" => "中文（繁體）".into(),
        _ => crate::spell::language_display_name(&code.replace('-', "_")),
    }
}

/// The language Hylki's own text is in, as one of [`LANGUAGES`]' codes
/// where it is one: the language chosen in Settings, else the session's.
pub fn ui_language() -> String {
    let chosen = crate::config::load_language();
    let raw = if chosen.is_empty() {
        ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .map(|v| v.split(':').next().unwrap_or_default().to_string())
            .find(|v| !v.is_empty() && v != "C" && v != "POSIX" && !v.starts_with("C."))
            .unwrap_or_else(|| "en".into())
    } else {
        chosen
    };
    normalize(&raw)
}

/// `pt_BR.UTF-8` → `pt-BR`, `zh_TW` → `zh-Hant`, `de_AT` → `de`.
fn normalize(raw: &str) -> String {
    let base = raw.split(['.', '@']).next().unwrap_or(raw).replace('_', "-");
    let lang = base.split('-').next().unwrap_or(&base).to_lowercase();
    let region = base.split('-').nth(1).unwrap_or_default();
    match lang.as_str() {
        "pt" if region.eq_ignore_ascii_case("PT") => "pt-PT".into(),
        "pt" => "pt-BR".into(),
        "zh" if matches!(region, "TW" | "HK" | "MO" | "Hant") => "zh-Hant".into(),
        "zh" => "zh-Hans".into(),
        "no" | "nn" => "nb".into(),
        "iw" => "he".into(),
        "in" => "id".into(),
        _ => lang,
    }
}

/// The language part alone: `pt-BR` → `pt`, `zh-Hant` → `zh`.
pub fn base(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

/// How `service` spells the target language `code`.
fn service_code(service: Service, code: &str) -> String {
    match (service, code) {
        (Service::DeepL, "en") => {
            // DeepL wants the variety; the session's locale says which.
            let british = ["LC_ALL", "LC_MESSAGES", "LANG"]
                .iter()
                .filter_map(|k| std::env::var(k).ok())
                .any(|v| ["en_GB", "en_IE", "en_AU", "en_NZ", "en_ZA", "en_IN"].iter().any(|p| v.starts_with(p)));
            if british { "EN-GB" } else { "EN-US" }.into()
        }
        (Service::DeepL, _) => code.to_uppercase(),
        (Service::Google, "pt-BR") => "pt".into(),
        (Service::Google, "zh-Hans") => "zh-CN".into(),
        (Service::Google, "zh-Hant") => "zh-TW".into(),
        (Service::Google, "nb") => "no".into(),
        (Service::Google, "he") => "iw".into(),
        (Service::Microsoft, "pt-BR") => "pt".into(),
        (Service::Microsoft, "pt-PT") => "pt-pt".into(),
        (Service::Libre, "pt-BR" | "pt-PT") => "pt".into(),
        (Service::Libre, "zh-Hans") => "zh".into(),
        _ => code.into(),
    }
}

/// A language a service reported back, as one of this module's codes.
fn from_service_code(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "zh-tw" | "zh-hant" | "zh-hk" => "zh-Hant".into(),
        l if l.starts_with("zh") => "zh-Hans".into(),
        "pt-pt" => "pt-PT".into(),
        "pt" | "pt-br" => "pt".into(),
        "no" => "nb".into(),
        "iw" => "he".into(),
        _ => base(&lower).to_string(),
    }
}

// ─── Settings file and key ────────────────────────────────────────────────

fn path() -> Option<PathBuf> {
    Some(crate::config::config_base()?.join("hylki").join("translation.toml"))
}

/// Bumped on every save, so an open reader can tell its offer is stale.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

pub fn load() -> Settings {
    let Some(path) = path() else { return Settings::default() };
    let Ok(text) = std::fs::read_to_string(path) else { return Settings::default() };
    toml::from_str(&text).unwrap_or_default()
}

pub fn save(settings: &Settings) {
    let Some(path) = path() else { return };
    match toml::to_string_pretty(settings) {
        Ok(text) => {
            if let Err(e) = crate::config::write_private_file(&path, &text) {
                tracing::warn!("could not save the translation settings: {e}");
            }
        }
        Err(e) => tracing::warn!("could not save the translation settings: {e}"),
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

pub fn load_key(service: Service) -> Option<String> {
    crate::config::load_cloud_password(&service.key_name()).filter(|k| !k.trim().is_empty())
}

pub fn store_key(service: Service, key: &str) {
    if key.trim().is_empty() {
        crate::config::delete_cloud_password(&service.key_name());
    } else if let Err(e) = crate::config::store_cloud_password(&service.key_name(), key.trim()) {
        tracing::warn!("could not store the {} key: {e}", service.name());
    }
}

// ─── Language detection ───────────────────────────────────────────────────

/// Which language a message's text is in, when that can be told with
/// confidence: one of this module's codes, or `None`. Runs offline.
pub fn detect(html: &str) -> Option<String> {
    let text = visible_text(html);
    // A line or two is too little to go on, and a long newsletter is
    // decided well before its end.
    let text: String = text.chars().take(4000).collect();
    if text.chars().filter(|c| c.is_alphabetic()).count() < 40 {
        return None;
    }
    let info = whatlang::detect(&text)?;
    if !info.is_reliable() {
        return None;
    }
    let code = match info.lang().code() {
        "ara" => "ar",
        "bul" => "bg",
        "cat" => "ca",
        "ces" => "cs",
        "cmn" => "zh-Hans",
        "dan" => "da",
        "deu" => "de",
        "ell" => "el",
        "eng" => "en",
        "est" => "et",
        "fin" => "fi",
        "fra" => "fr",
        "heb" => "he",
        "hin" => "hi",
        "hrv" => "hr",
        "hun" => "hu",
        "ind" => "id",
        "ita" => "it",
        "jpn" => "ja",
        "kor" => "ko",
        "lav" => "lv",
        "lit" => "lt",
        "nld" => "nl",
        "nob" => "nb",
        "pol" => "pl",
        "por" => "pt",
        "ron" => "ro",
        "rus" => "ru",
        "slk" => "sk",
        "slv" => "sl",
        "spa" => "es",
        "srp" => "sr",
        "swe" => "sv",
        "tha" => "th",
        "tur" => "tr",
        "ukr" => "uk",
        "vie" => "vi",
        other => other,
    };
    Some(code.to_string())
}

/// Whether a message in `detected` is worth offering to translate into
/// `target`: a different language, not a variety of the same one.
pub fn differs(detected: &str, target: &str) -> bool {
    base(detected) != base(target)
}

/// The words of a fragment of HTML, roughly: tags dropped, the common
/// entities decoded, quoted text left out. Only good enough for telling
/// the language, which is all it is used for.
fn visible_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut tag = String::new();
    let mut skip_depth = 0usize;
    for c in html.chars() {
        if in_tag {
            if c == '>' {
                in_tag = false;
                let t = tag.trim().to_ascii_lowercase();
                let name: String = t.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                if matches!(name.as_str(), "blockquote" | "style" | "script") {
                    if t.starts_with('/') {
                        skip_depth = skip_depth.saturating_sub(1);
                    } else {
                        skip_depth += 1;
                    }
                }
                out.push(' ');
                tag.clear();
            } else {
                tag.push(c);
            }
        } else if c == '<' {
            in_tag = true;
        } else if skip_depth == 0 {
            out.push(c);
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    // Plain-text replies quote with "> " lines.
    out.lines().filter(|l| !l.trim_start().starts_with('>')).collect::<Vec<_>>().join("\n")
}

// ─── Translating ──────────────────────────────────────────────────────────

/// A translated message.
#[derive(Clone, Debug, PartialEq)]
pub struct Translated {
    /// The translation, as a Reader View fragment.
    pub html: String,
    /// The language the service found the message to be in.
    pub from: Option<String>,
    pub service: Service,
    pub to: String,
}

/// The most of a message's Reader View, in characters of HTML, that is
/// sent in one piece: under every service's per-text limit, and a size
/// whose loss to an error costs little of the user's allowance.
const CHUNK: usize = 4_500;
/// How much goes in one request, under the smallest request limit
/// (Microsoft's 50,000 characters).
const BATCH: usize = 40_000;
/// Past this a message is not sent at all: a mailing list digest or a
/// long log would use up a free plan's month in a few clicks.
const LIMIT: usize = 120_000;

/// Translations made this session, so a message opened again is shown
/// translated without asking the service, and paying for it, twice.
static DONE: Mutex<Option<HashMap<String, Translated>>> = Mutex::new(None);

/// What the session cache keys a translation by: the message, the service
/// and the language it went into.
pub fn cache_key(account_id: u32, message: &str, settings: &Settings) -> String {
    format!("{account_id}\u{1f}{message}\u{1f}{:?}\u{1f}{}", settings.service, settings.target_language())
}

pub fn cached(key: &str) -> Option<Translated> {
    DONE.lock().ok()?.as_ref()?.get(key).cloned()
}

/// Translate a message body (as cached: HTML, or plain text) with the
/// configured service (blocking; call off the UI thread).
pub fn translate(settings: &Settings, key: &str, body: &str, cache: &str) -> Result<Translated, String> {
    if let Some(t) = cached(cache) {
        return Ok(t);
    }
    let service = settings.service;
    if service == Service::Off {
        return Err(i18n("Choose a translation service in Settings → Translation first."));
    }
    if key.is_empty() && service != Service::Libre {
        return Err(i18n_f("No key for {service}: add one in Settings → Translation.", &[("service", &service.name())]));
    }
    let blocks = prepare_blocks(body);
    let total: usize = blocks.iter().map(String::len).sum();
    if total == 0 {
        return Err(i18n("There is no text to translate."));
    }
    if total > LIMIT {
        return Err(i18n("This message is too long to send for translation."));
    }
    let chunks = chunk(&blocks);
    let target = settings.target_language();
    let to = service_code(service, &target);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(45)).build();
    let mut out = String::new();
    let mut from = None;
    for batch in batches(&chunks, service) {
        let (texts, detected) = match service {
            Service::DeepL => deepl(&agent, key, &batch, &to)?,
            Service::Google => google(&agent, key, &batch, &to)?,
            Service::Microsoft => microsoft(&agent, key, &settings.region, &batch, &to)?,
            Service::Libre => libre(&agent, key, &settings.url, &batch, &to)?,
            Service::Off => unreachable!(),
        };
        if texts.len() != batch.len() {
            return Err(i18n("The service's answer could not be read."));
        }
        if from.is_none() {
            from = detected.map(|d| from_service_code(&d));
        }
        for t in texts {
            out.push_str(&t);
        }
    }
    let done = Translated { html: out, from, service, to: target };
    if let Ok(mut g) = DONE.lock() {
        let map = g.get_or_insert_with(HashMap::new);
        if map.len() > 200 {
            map.clear();
        }
        map.insert(cache.to_string(), done.clone());
    }
    Ok(done)
}

/// The Reader View of a body as top-level blocks. Plain text, which the
/// reader shows line for line, is turned into paragraphs and line breaks
/// first: sent as markup, its newlines would otherwise be read as spaces.
fn prepare_blocks(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for block in crate::reader::extract_blocks(body) {
        match block
            .strip_prefix("<div class=\"vireo-plain\">")
            .and_then(|b| b.strip_suffix("</div>"))
        {
            Some(text) => {
                for para in text.split("\n\n").map(|p| p.trim_matches('\n')).filter(|p| !p.trim().is_empty()) {
                    out.push(format!("<p>{}</p>", para.replace('\n', "<br>")));
                }
            }
            None if !block.trim().is_empty() => out.push(block),
            None => {}
        }
    }
    out
}

/// Blocks joined into pieces of at most [`CHUNK`] characters; a block
/// bigger than that on its own goes as one piece.
fn chunk(blocks: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in blocks {
        match out.last_mut() {
            Some(last) if last.len() + b.len() <= CHUNK => last.push_str(b),
            _ => out.push(b.clone()),
        }
    }
    out
}

/// Pieces grouped into requests. LibreTranslate servers differ in whether
/// they take several texts at once, so it gets one at a time.
fn batches(chunks: &[String], service: Service) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut size = 0;
    for c in chunks {
        let full = match out.last() {
            None => true,
            Some(last) => service == Service::Libre || size + c.len() > BATCH || last.len() >= 50,
        };
        if full {
            out.push(Vec::new());
            size = 0;
        }
        size += c.len();
        out.last_mut().unwrap().push(c.clone());
    }
    out
}

type Answer = (Vec<String>, Option<String>);

/// A request's result as JSON, with the service's own explanation when it
/// refused.
fn send(req: ureq::Request, body: Value) -> Result<Value, String> {
    match req.send_json(body) {
        Ok(resp) => resp.into_json::<Value>().map_err(|_| i18n("The service's answer could not be read.")),
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            Err(refusal(code, &text))
        }
        Err(e) => Err(i18n_f("The service could not be reached: {e}", &[("e", &e.to_string())])),
    }
}

/// What a refusal means, in words.
fn refusal(code: u16, body: &str) -> String {
    let said = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            [
                v["message"].as_str(),
                v["error"]["message"].as_str(),
                v["error"].as_str(),
                v["detail"].as_str(),
            ]
            .into_iter()
            .flatten()
            .next()
            .map(str::to_string)
        })
        .unwrap_or_default();
    let what = match code {
        401 | 403 => i18n("The service did not accept the key."),
        429 => i18n("Too many requests: try again in a moment."),
        456 => i18n("This month's allowance is used up."),
        _ => i18n_f("The service refused the request (error {code}).", &[("code", &code.to_string())]),
    };
    if said.is_empty() {
        what
    } else {
        format!("{what} ({said})")
    }
}

fn deepl(agent: &ureq::Agent, key: &str, texts: &[String], to: &str) -> Result<Answer, String> {
    let host = if key.trim().ends_with(":fx") { "api-free.deepl.com" } else { "api.deepl.com" };
    let req = agent
        .post(&format!("https://{host}/v2/translate"))
        .set("Authorization", &format!("DeepL-Auth-Key {}", key.trim()));
    let v = send(req, json!({ "text": texts, "target_lang": to, "tag_handling": "html" }))?;
    let list = v["translations"].as_array().cloned().unwrap_or_default();
    let detected = list.first().and_then(|t| t["detected_source_language"].as_str()).map(str::to_string);
    Ok((list.iter().map(|t| t["text"].as_str().unwrap_or_default().to_string()).collect(), detected))
}

fn google(agent: &ureq::Agent, key: &str, texts: &[String], to: &str) -> Result<Answer, String> {
    let req = agent
        .post("https://translation.googleapis.com/language/translate/v2")
        .query("key", key.trim());
    let v = send(req, json!({ "q": texts, "target": to, "format": "html" }))?;
    let list = v["data"]["translations"].as_array().cloned().unwrap_or_default();
    let detected = list.first().and_then(|t| t["detectedSourceLanguage"].as_str()).map(str::to_string);
    Ok((list.iter().map(|t| t["translatedText"].as_str().unwrap_or_default().to_string()).collect(), detected))
}

fn microsoft(agent: &ureq::Agent, key: &str, region: &str, texts: &[String], to: &str) -> Result<Answer, String> {
    let mut req = agent
        .post("https://api.cognitive.microsofttranslator.com/translate")
        .query("api-version", "3.0")
        .query("to", to)
        .query("textType", "html")
        .set("Ocp-Apim-Subscription-Key", key.trim());
    if !region.trim().is_empty() {
        req = req.set("Ocp-Apim-Subscription-Region", region.trim());
    }
    let body: Vec<Value> = texts.iter().map(|t| json!({ "Text": t })).collect();
    let v = send(req, Value::Array(body))?;
    let list = v.as_array().cloned().unwrap_or_default();
    let detected = list.first().and_then(|t| t["detectedLanguage"]["language"].as_str()).map(str::to_string);
    Ok((list.iter().map(|t| t["translations"][0]["text"].as_str().unwrap_or_default().to_string()).collect(), detected))
}

fn libre(agent: &ureq::Agent, key: &str, url: &str, texts: &[String], to: &str) -> Result<Answer, String> {
    let base = url.trim().trim_end_matches('/');
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err(i18n("Enter the LibreTranslate server's address in Settings → Translation."));
    }
    let mut out = Vec::new();
    let mut detected = None;
    for text in texts {
        let mut body = json!({ "q": text, "source": "auto", "target": to, "format": "html" });
        if !key.trim().is_empty() {
            body["api_key"] = Value::String(key.trim().to_string());
        }
        let v = send(agent.post(&format!("{base}/translate")), body)?;
        if detected.is_none() {
            detected = v["detectedLanguage"]["language"].as_str().map(str::to_string);
        }
        out.push(v["translatedText"].as_str().unwrap_or_default().to_string());
    }
    Ok((out, detected))
}

/// Try the settings with a greeting, for the Settings page's check
/// (blocking): what was sent and what came back. The greeting is in a
/// language other than the target, or nothing would change.
pub fn check(settings: &Settings, key: &str) -> Result<(String, String), String> {
    let greeting = if base(&settings.target_language()) == "en" { "Guten Morgen" } else { "Good morning" };
    let probe = format!("{}\u{1f}check", generation());
    let t = translate(settings, key, &format!("<p>{greeting}</p>"), &probe)?;
    if let Ok(mut g) = DONE.lock() {
        if let Some(map) = g.as_mut() {
            map.remove(&probe);
        }
    }
    Ok((greeting.to_string(), visible_text(&t.html).trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_become_target_codes() {
        assert_eq!(normalize("de_AT.UTF-8"), "de");
        assert_eq!(normalize("pt_BR.UTF-8"), "pt-BR");
        assert_eq!(normalize("pt_PT"), "pt-PT");
        assert_eq!(normalize("zh_TW.UTF-8"), "zh-Hant");
        assert_eq!(normalize("zh_CN"), "zh-Hans");
        assert_eq!(normalize("nn_NO"), "nb");
        assert_eq!(normalize("en"), "en");
    }

    #[test]
    fn every_service_gets_its_own_spelling() {
        assert_eq!(service_code(Service::DeepL, "pt-BR"), "PT-BR");
        assert_eq!(service_code(Service::DeepL, "zh-Hans"), "ZH-HANS");
        assert_eq!(service_code(Service::Google, "zh-Hant"), "zh-TW");
        assert_eq!(service_code(Service::Google, "he"), "iw");
        assert_eq!(service_code(Service::Microsoft, "pt-PT"), "pt-pt");
        assert_eq!(service_code(Service::Libre, "pt-BR"), "pt");
        assert_eq!(service_code(Service::Microsoft, "de"), "de");
        for code in LANGUAGES {
            assert!(!language_name(code).is_empty());
        }
    }

    #[test]
    fn detected_languages_come_back_as_ours() {
        assert_eq!(from_service_code("DE"), "de");
        assert_eq!(from_service_code("EN-US"), "en");
        assert_eq!(from_service_code("zh-CN"), "zh-Hans");
        assert_eq!(from_service_code("zh-TW"), "zh-Hant");
        assert_eq!(from_service_code("iw"), "he");
    }

    #[test]
    fn plain_text_is_sent_as_paragraphs() {
        let blocks = prepare_blocks("Hallo Anna,\nwie geht es?\n\nViele Grüße\nBen");
        assert_eq!(blocks, ["<p>Hallo Anna,<br>wie geht es?</p>", "<p>Viele Grüße<br>Ben</p>"]);
    }

    #[test]
    fn long_messages_go_in_pieces() {
        let blocks: Vec<String> = (0..30).map(|i| format!("<p>{}</p>", "x".repeat(400 + i))).collect();
        let chunks = chunk(&blocks);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= CHUNK));
        assert_eq!(chunks.concat(), blocks.concat(), "nothing lost or reordered");
        assert_eq!(batches(&chunks, Service::DeepL).len(), 1);
        assert_eq!(batches(&chunks, Service::Libre).len(), chunks.len());
    }

    #[test]
    fn the_language_is_told_offline_and_quotes_do_not_count() {
        let de = "<p>Vielen Dank für Ihre Nachricht. Wir melden uns so bald wie möglich bei Ihnen, \
                  spätestens aber bis Ende der Woche.</p>";
        assert_eq!(detect(de).as_deref(), Some("de"));
        let reply = format!(
            "<p>Thanks, that works for me. I will bring the printed plans and the budget, and we can go through \
             both before lunch. See you on Friday at the office then.</p><blockquote>{de}{de}{de}</blockquote>"
        );
        assert_eq!(detect(&reply).as_deref(), Some("en"));
        assert_eq!(detect("<p>OK</p>"), None, "too little to go on");
        assert!(differs("de", "en"));
        assert!(!differs("pt", "pt-BR"));
    }

    #[test]
    fn refusals_say_what_went_wrong() {
        assert!(refusal(403, r#"{"message":"Wrong key"}"#).contains("Wrong key"));
        assert!(!refusal(456, "").is_empty());
        assert!(refusal(400, r#"{"error":{"message":"Bad language"}}"#).contains("Bad language"));
    }
}
