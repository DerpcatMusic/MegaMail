//! Hylki's mail worker and portable modules, with an isolated MegaMail profile.
//! The original GTK app uses the same source modules during the migration.

pub mod config;
pub mod conversation;
pub mod desktop;
pub mod discovery;
pub mod mail_text;
pub mod oauth;
pub mod onboarding;
pub mod query;
pub mod thunderbird;
pub mod thunderbird_bridge;

#[path = "../../../src/backend.rs"]
pub mod backend;
#[path = "../../../src/cache.rs"]
pub mod cache;
#[path = "../../../src/datefmt.rs"]
pub mod datefmt;
#[path = "../../../src/goa.rs"]
pub mod goa;
#[path = "../../../src/i18n.rs"]
pub mod i18n;
#[path = "../../../src/invite.rs"]
pub mod invite;
#[path = "../../../src/markdown.rs"]
pub mod markdown;
#[path = "../../../src/models.rs"]
pub mod models;
#[path = "../../../src/ms_broker.rs"]
pub mod ms_broker;
#[path = "../../../src/mutf7.rs"]
pub mod mutf7;
#[path = "../../../src/percent.rs"]
pub mod percent;
#[path = "../../../src/pgp.rs"]
pub mod pgp;
#[path = "../../../src/ram_cache.rs"]
pub mod ram_cache;
#[path = "../../../src/reader.rs"]
pub mod reader;
#[path = "../../../src/rng.rs"]
pub mod rng;
#[path = "../../../src/unsubscribe.rs"]
pub mod unsubscribe;
#[path = "../../../src/verify.rs"]
pub mod verify;
#[path = "../../../src/worker.rs"]
pub mod worker;

pub mod logo {
    pub const USER_AGENT: &str = "MegaMail/0.1";
}
