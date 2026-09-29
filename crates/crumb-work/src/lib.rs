//! The heavy work behind reading a recipe, shared by the server and a `crumb-relay` that
//! works for it on another machine (docs/RELAY.md, "Relays as workers"): headless Chromium
//! ([`browser`]) and a cooking video's download, transcript and stills ([`video`]). What they
//! say to each other is [`wire`].

pub mod browser;
pub mod video;
pub mod wire;
