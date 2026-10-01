//! System suspend/resume and network-return detection.
//!
//! When the machine sleeps, open IMAP sockets are silently killed by the OS or
//! dropped by NAT/keepalive timeouts. On wake our long-lived worker sessions
//! (including a parked IMAP IDLE) are dead but nothing notices, so the app stops
//! pulling new mail until the next auto-fetch tick (#322). Three signals cover
//! it, and the app reconnects on any of them:
//!
//! - logind's `PrepareForSleep(false)` on the **system** bus, the moment the
//!   system resumes. Only outside the Flatpak: the sandbox has no system bus.
//! - A gap between the boot clock and the monotonic clock, which works
//!   everywhere and notices a wake within [`CLOCK_TICK`].
//! - The network coming back, since at the moment of waking a laptop is usually
//!   still rejoining its Wi-Fi and a reconnect then would only fail.

use std::time::{Duration, Instant};

const LOGIN1_DEST: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const LOGIN1_MANAGER: &str = "org.freedesktop.login1.Manager";

/// How often the clock check runs, and so the most a wake can go unnoticed
/// where logind is out of reach.
const CLOCK_TICK: u32 = 10;
/// How far the boot clock may run ahead of the monotonic one across a tick
/// before it counts as a sleep. Both advance together while awake, so any
/// real suspend clears this easily.
const SLEEP_GAP: Duration = Duration::from_secs(5);

/// Watch systemd-logind for resume-from-sleep, invoking `on_wake` each time the
/// system wakes. Runs on a dedicated thread; silently no-ops if logind or the
/// system bus is unavailable.
pub fn watch_resume<F: Fn() + Send + 'static>(on_wake: F) {
    let _ = std::thread::Builder::new()
        .name("logind-watch".into())
        .spawn(move || {
            if let Err(e) = watch_loop(&on_wake) {
                tracing::debug!("logind sleep watch stopped: {e}");
            }
        });
}

fn watch_loop<F: Fn()>(on_wake: &F) -> Result<(), Box<dyn std::error::Error>> {
    let conn = zbus::blocking::Connection::system()?;
    let proxy = zbus::blocking::Proxy::new(&conn, LOGIN1_DEST, LOGIN1_PATH, LOGIN1_MANAGER)?;
    // Blocks until logind emits PrepareForSleep; ends if the bus closes.
    let mut signals = proxy.receive_signal("PrepareForSleep")?;
    for msg in signals.by_ref() {
        // start == true just before sleeping, false once resumed. Only the
        // resume edge needs a reconnect.
        if let Ok(false) = msg.body().deserialize::<bool>() {
            on_wake();
        }
    }
    Ok(())
}

/// Notice a wake from the clocks alone, for where logind can't be reached.
/// `Instant` is CLOCK_MONOTONIC, which stands still while the system sleeps;
/// /proc/uptime is the boot clock, which keeps counting. Must be called on the
/// GTK main thread.
pub fn watch_clock_gap<F: Fn() + 'static>(on_wake: F) {
    let Some(mut uptime) = boot_clock() else { return };
    let mut mono = Instant::now();
    gtk::glib::timeout_add_seconds_local(CLOCK_TICK, move || {
        let Some(now_uptime) = boot_clock() else { return gtk::glib::ControlFlow::Break };
        let now_mono = Instant::now();
        let slept = (now_uptime.saturating_sub(uptime)).saturating_sub(now_mono - mono);
        uptime = now_uptime;
        mono = now_mono;
        if slept >= SLEEP_GAP {
            tracing::info!("woke from about {}s of sleep", slept.as_secs());
            on_wake();
        }
        gtk::glib::ControlFlow::Continue
    });
}

fn boot_clock() -> Option<Duration> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = text.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}

/// Invoke `on_back` each time the network becomes reachable again after being
/// down. GIO answers through the network-monitor portal inside the Flatpak,
/// so this needs no permission. Must be called on the GTK main thread.
pub fn watch_network<F: Fn() + 'static>(on_back: F) {
    use gtk::gio::prelude::*;
    let monitor = gtk::gio::NetworkMonitor::default();
    let was_up = std::cell::Cell::new(monitor.is_network_available());
    // Fires on every route change too (VPNs, container bridges); only the
    // down-to-up edge means sessions opened since have nothing to reuse.
    monitor.connect_network_changed(move |_, up| {
        if up && !was_up.get() {
            tracing::info!("network is back");
            on_back();
        }
        was_up.set(up);
    });
}

/// Whether the network looks reachable right now.
pub fn network_available() -> bool {
    use gtk::gio::prelude::*;
    gtk::gio::NetworkMonitor::default().is_network_available()
}
