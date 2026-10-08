//! What the app has to ask the desktop itself, over GTK's head.
//!
//! Its settings (GNOME's `org.gnome.desktop.interface` and kin), which the
//! locale and GTK know nothing about: the clock format (#173), the
//! monospace font (#181). Read through the settings portal, which works
//! inside the Flatpak sandbox and on the host alike, or — outside the
//! sandbox, when no portal answers — straight from GSettings where the
//! schema is installed. Whether it wants quiet, for a new-mail sound (#292).
//!
//! And its window manager, when the window has to come to the front and
//! `present` is not enough to get it there: see [`present_window`].

/// The string value of `key` in `namespace`, if the desktop has one.
pub fn setting(namespace: &str, key: &str) -> Option<String> {
    fn from_value(v: zbus::zvariant::Value<'_>) -> Option<String> {
        match v {
            zbus::zvariant::Value::Value(inner) => from_value(*inner),
            zbus::zvariant::Value::Str(s) => Some(s.as_str().to_string()),
            _ => None,
        }
    }
    let portal = portal_setting(namespace, key).and_then(|v| from_value(v.into()));
    if portal.is_some() || std::env::var_os("FLATPAK_ID").is_some() {
        return portal;
    }
    use gio::prelude::SettingsExt;
    let settings = host_settings(namespace)?;
    Some(settings.string(key).as_str().to_string())
}

/// The boolean value of `key` in `namespace`, if the desktop has one.
pub fn flag(namespace: &str, key: &str) -> Option<bool> {
    fn from_value(v: zbus::zvariant::Value<'_>) -> Option<bool> {
        match v {
            zbus::zvariant::Value::Value(inner) => from_value(*inner),
            zbus::zvariant::Value::Bool(b) => Some(b),
            _ => None,
        }
    }
    let portal = portal_setting(namespace, key).and_then(|v| from_value(v.into()));
    if portal.is_some() || std::env::var_os("FLATPAK_ID").is_some() {
        return portal;
    }
    use gio::prelude::SettingsExt;
    let settings = host_settings(namespace)?;
    Some(settings.boolean(key))
}

fn portal_setting(namespace: &str, key: &str) -> Option<zbus::zvariant::OwnedValue> {
    let conn = zbus::blocking::Connection::session().ok()?;
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &(namespace, key),
        )
        .ok()?;
    reply.body().deserialize().ok()
}

/// GSettings for `namespace`, where its schema is installed: looking a
/// missing schema up is fatal to GLib, so it is asked for first.
fn host_settings(namespace: &str) -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(namespace, true)?;
    Some(gio::Settings::new(namespace))
}
