//! Sender pictures from the internet, fetched off the UI thread and cached on
//! disk. Three sources, in order of trust:
//!
//! 1. **BIMI**: the brand logo a domain publishes at `default._bimi.<domain>`.
//! 2. **Brand icon**: the domain's `apple-touch-icon` / `favicon`.
//! 3. **Gravatar** (opt-in): the picture a person registered for their address.
//!
//! Domain logos are only fetched after a message from that domain passed
//! DMARC at the user's provider, and the cache file only exists after such a
//! verification; a spoofed sender therefore never *introduces* a brand logo.
//! Shared mailbox domains (gmail.com, outlook.com, …) never get a logo, so
//! people on them are not shown as "Google". No per-message identifiers are
//! ever sent: requests name only a domain or an address hash.

use std::io::Read as _;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use megamail_core::config;

const FETCH_MAX: u64 = 768 * 1024;
const SIDE: u32 = 96;
const RETRY_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);

/// Mailbox providers whose domain says nothing about the person.
const SHARED_DOMAINS: &[&str] = &[
    "gmail.com", "googlemail.com", "outlook.com", "hotmail.com", "live.com", "msn.com",
    "yahoo.com", "ymail.com", "icloud.com", "me.com", "mac.com", "aol.com", "proton.me",
    "protonmail.com", "pm.me", "gmx.com", "gmx.de", "gmx.net", "web.de", "mail.com",
    "zoho.com", "yandex.com", "yandex.ru", "fastmail.com", "tutanota.com", "tuta.io",
    "hey.com", "walla.co.il", "walla.com", "bezeqint.net", "012.net.il", "netvision.net.il",
    "qq.com", "163.com", "naver.com", "seznam.cz", "libero.it", "orange.fr", "free.fr",
    "t-online.de", "comcast.net", "verizon.net", "att.net", "btinternet.com",
];

/// The organisation's domain for a sender subdomain (`em.github.com` → `github.com`).
pub(crate) fn organization_domain(email_or_domain: &str) -> Option<String> {
    let domain = email_or_domain
        .rsplit_once('@')
        .map_or(email_or_domain, |(_, d)| d)
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if domain.is_empty()
        || domain.len() > 253
        || !domain.contains('.')
        || !domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return None;
    }
    let labels: Vec<&str> = domain.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() < 2 {
        return None;
    }
    let second = labels[labels.len() - 2];
    let tld = labels[labels.len() - 1];
    let keep = if labels.len() >= 3
        && tld.len() == 2
        && matches!(second, "co" | "com" | "org" | "net" | "ac" | "gov" | "edu" | "ltd" | "plc" | "or" | "ne")
    {
        3
    } else {
        2
    };
    Some(labels[labels.len() - keep..].join("."))
}

pub(crate) fn is_shared_domain(domain: &str) -> bool {
    SHARED_DOMAINS.contains(&domain)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn cache_dir(kind: &str) -> Option<PathBuf> {
    let dir = config::data_base()?.join("avatars.v2").join(kind);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn logo_path(domain: &str) -> Option<PathBuf> {
    Some(cache_dir("logos")?.join(format!("{}.png", hex(domain.as_bytes()))))
}

fn gravatar_path(email: &str) -> Option<PathBuf> {
    Some(cache_dir("gravatar")?.join(format!("{}.png", gravatar_hash(email))))
}

fn gravatar_hash(email: &str) -> String {
    use sha2::{Digest as _, Sha256};
    hex(&Sha256::digest(email.trim().to_lowercase().as_bytes()))
}

/// A miss is remembered (an empty file) and retried after a week.
fn recently_missed(path: &PathBuf) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| {
        meta.len() == 0
            && meta
                .modified()
                .ok()
                .and_then(|at| SystemTime::now().duration_since(at).ok())
                .is_some_and(|age| age < RETRY_AFTER)
    })
}

fn load_png(path: &PathBuf) -> Option<image::RgbaImage> {
    let bytes = std::fs::read(path).ok().filter(|b| !b.is_empty())?;
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .ok()
        .map(|image| image.to_rgba8())
}

fn store(path: &PathBuf, image: Option<&image::RgbaImage>) {
    let mut bytes = Vec::new();
    if let Some(image) = image {
        let _ = image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png);
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&temporary, &bytes).is_ok() {
        let _ = std::fs::rename(&temporary, path);
    }
}

/// Cached domain logo; present only once that domain was verified.
pub(crate) fn cached_logo(email: &str) -> Option<image::RgbaImage> {
    let domain = organization_domain(email)?;
    if is_shared_domain(&domain) {
        return None;
    }
    load_png(&logo_path(&domain)?)
}

/// Fetch and cache the logo of a domain that just passed DMARC. Blocking;
/// call from a background task. Returns whether a logo is now cached.
pub(crate) fn ensure_verified_logo(email: &str) -> bool {
    let Some(domain) = organization_domain(email) else {
        return false;
    };
    if is_shared_domain(&domain) {
        return false;
    }
    let Some(path) = logo_path(&domain) else {
        return false;
    };
    if load_png(&path).is_some() {
        return true;
    }
    if recently_missed(&path) {
        return false;
    }
    let sender_domain = email.rsplit_once('@').map_or(email, |(_, d)| d).to_ascii_lowercase();
    let logo = bimi_logo(&sender_domain)
        .or_else(|| (sender_domain != domain).then(|| bimi_logo(&domain)).flatten())
        .or_else(|| brand_icon(&domain));
    store(&path, logo.as_ref());
    logo.is_some()
}

/// Gravatar for an address (opt-in). Blocking; background only.
pub(crate) fn gravatar(email: &str) -> Option<image::RgbaImage> {
    let path = gravatar_path(email)?;
    if let Some(image) = load_png(&path) {
        return Some(image);
    }
    if recently_missed(&path) {
        return None;
    }
    let url = format!("https://gravatar.com/avatar/{}?s=192&d=404", gravatar_hash(email));
    let image = fetch(&url).and_then(|bytes| decode_raster(&bytes, 64));
    store(&path, image.as_ref());
    image
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .redirects(3)
        .user_agent("MegaMail/0.1 (sender pictures)")
        .build()
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://") {
        return None;
    }
    let response = agent().get(url).call().ok()?;
    if response.status() != 200 {
        return None;
    }
    let mut bytes = Vec::new();
    response.into_reader().take(FETCH_MAX + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= FETCH_MAX && !bytes.is_empty()).then_some(bytes)
}

/// Decode a raster picture, square-cropped to [`SIDE`]. Tiny icons (a 16px
/// favicon) are refused: initials read better than a blurred blob.
fn decode_raster(bytes: &[u8], min_side: u32) -> Option<image::RgbaImage> {
    let format = image::guess_format(bytes).ok()?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP | image::ImageFormat::Ico
    ) {
        return None;
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    if image.width().min(image.height()) < min_side {
        return None;
    }
    Some(
        image
            .resize_to_fill(SIDE, SIDE, image::imageops::FilterType::Lanczos3)
            .to_rgba8(),
    )
}

fn brand_icon(domain: &str) -> Option<image::RgbaImage> {
    for url in [
        format!("https://{domain}/apple-touch-icon.png"),
        format!("https://www.{domain}/apple-touch-icon.png"),
        format!("https://{domain}/favicon.ico"),
        format!("https://www.{domain}/favicon.ico"),
    ] {
        if let Some(image) = fetch(&url).and_then(|bytes| decode_raster(&bytes, 32)) {
            return Some(image);
        }
    }
    None
}

fn bimi_logo(domain: &str) -> Option<image::RgbaImage> {
    let record = dns_txt(&format!("default._bimi.{domain}"))
        .into_iter()
        .find(|record| record.to_ascii_lowercase().starts_with("v=bimi1"))?;
    let location = record
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("l=").or_else(|| part.strip_prefix("L=")))?
        .trim();
    let svg = fetch(location)?;
    rasterize_svg(&svg)
}

pub(crate) fn rasterize_svg(svg: &[u8]) -> Option<image::RgbaImage> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg, &options).ok()?;
    let size = tree.size();
    let scale = SIDE as f32 / size.width().max(size.height()).max(1.);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIDE, SIDE)?;
    let dx = (SIDE as f32 - size.width() * scale) / 2.;
    let dy = (SIDE as f32 - size.height() * scale) / 2.;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_row(scale, 0., 0., scale, dx, dy),
        &mut pixmap.as_mut(),
    );
    // tiny-skia is premultiplied; unpremultiply for a straight-alpha image.
    let mut data = pixmap.take();
    for pixel in data.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    image::RgbaImage::from_raw(SIDE, SIDE, data)
}

// ---------------------------------------------------------------------------
// A minimal DNS TXT lookup against the system resolver (UDP, recursion
// desired). Enough for BIMI records; no extra resolver dependency.

fn nameservers() -> Vec<String> {
    let mut servers: Vec<String> = std::fs::read_to_string("/etc/resolv.conf")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("nameserver"))
        .map(|rest| rest.trim().to_owned())
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
        .collect();
    if servers.is_empty() {
        servers.push("127.0.0.53".into());
    }
    servers.truncate(3);
    servers
}

pub(crate) fn dns_txt(name: &str) -> Vec<String> {
    let Some(query) = encode_query(name, rand_id()) else {
        return Vec::new();
    };
    for server in nameservers() {
        let address = if server.contains(':') { format!("[{server}]:53") } else { format!("{server}:53") };
        let bind = if server.contains(':') { "[::]:0" } else { "0.0.0.0:0" };
        let Ok(socket) = UdpSocket::bind(bind) else { continue };
        let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
        if socket.send_to(&query, &address).is_err() {
            continue;
        }
        let mut buffer = [0u8; 4096];
        if let Ok(len) = socket.recv(&mut buffer) {
            if let Some(records) = parse_txt_response(&buffer[..len], &query[..2]) {
                return records;
            }
        }
    }
    Vec::new()
}

fn rand_id() -> [u8; 2] {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mixed = nanos ^ std::process::id().rotate_left(16);
    [(mixed >> 8) as u8, mixed as u8]
}

fn encode_query(name: &str, id: [u8; 2]) -> Option<Vec<u8>> {
    let mut out = vec![id[0], id[1], 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.extend_from_slice(&[0, 0, 16, 0, 1]);
    Some(out)
}

fn skip_name(packet: &[u8], mut at: usize) -> Option<usize> {
    loop {
        let len = *packet.get(at)? as usize;
        if len == 0 {
            return Some(at + 1);
        }
        if len & 0xC0 == 0xC0 {
            return Some(at + 2);
        }
        at += 1 + len;
    }
}

fn parse_txt_response(packet: &[u8], id: &[u8]) -> Option<Vec<String>> {
    if packet.len() < 12 || &packet[..2] != id || packet[2] & 0x80 == 0 {
        return None;
    }
    let rcode = packet[3] & 0x0F;
    if rcode == 3 {
        return Some(Vec::new()); // NXDOMAIN: definitive "no record".
    }
    if rcode != 0 {
        return None;
    }
    let questions = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let answers = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    let mut at = 12;
    for _ in 0..questions {
        at = skip_name(packet, at)? + 4;
    }
    let mut records = Vec::new();
    for _ in 0..answers {
        at = skip_name(packet, at)?;
        let kind = u16::from_be_bytes([*packet.get(at)?, *packet.get(at + 1)?]);
        let length = u16::from_be_bytes([*packet.get(at + 8)?, *packet.get(at + 9)?]) as usize;
        let data = packet.get(at + 10..at + 10 + length)?;
        at += 10 + length;
        if kind != 16 {
            continue;
        }
        let mut text = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let n = data[i] as usize;
            text.extend_from_slice(data.get(i + 1..i + 1 + n)?);
            i += 1 + n;
        }
        records.push(String::from_utf8_lossy(&text).into_owned());
    }
    Some(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_domains_and_shared_mailboxes() {
        assert_eq!(organization_domain("news@em.github.com").as_deref(), Some("github.com"));
        assert_eq!(organization_domain("a@mail.bbc.co.uk").as_deref(), Some("bbc.co.uk"));
        assert_eq!(organization_domain("x@walla.co.il").as_deref(), Some("walla.co.il"));
        assert!(organization_domain("no-domain").is_none());
        assert!(organization_domain("a@bad_domain.com").is_none());
        assert!(is_shared_domain("gmail.com"));
        assert!(!is_shared_domain("github.com"));
    }

    #[test]
    fn dns_query_round_trips_a_txt_answer() {
        let query = encode_query("default._bimi.example.com", [0xab, 0xcd]).unwrap();
        // Response: header (1 answer), the question, then a compressed-name TXT answer.
        let mut response = query.clone();
        response[2] = 0x81;
        response[3] = 0x80;
        response[7] = 1;
        let record = b"v=BIMI1; l=https://example.com/logo.svg";
        response.extend_from_slice(&[0xC0, 0x0C, 0, 16, 0, 1, 0, 0, 0, 60]);
        response.extend_from_slice(&((record.len() + 1) as u16).to_be_bytes());
        response.push(record.len() as u8);
        response.extend_from_slice(record);
        let records = parse_txt_response(&response, &[0xab, 0xcd]).unwrap();
        assert_eq!(records, vec![String::from_utf8_lossy(record).into_owned()]);
        assert!(parse_txt_response(&response, &[0, 0]).is_none(), "id must match");
    }

    #[test]
    fn svg_logos_rasterize_to_a_square() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;
        let image = rasterize_svg(svg).unwrap();
        assert_eq!(image.dimensions(), (SIDE, SIDE));
        assert_eq!(image.get_pixel(SIDE / 2, SIDE / 2).0, [255, 0, 0, 255]);
    }
}
