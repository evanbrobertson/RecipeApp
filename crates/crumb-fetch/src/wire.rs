//! What the server and `crumb-relay` say to each other, so neither can drift from the other.
//!
//! `POST /fetch` with a bearer token takes a [`FetchRequest`] and answers 200 with a
//! [`FetchReply`] (the fetch happened, whatever the site said), or a non-200 status with an
//! [`ErrorReply`]:
//!
//! | Status | Meaning |
//! | --- | --- |
//! | 400 | the request or the link is refused (not a public web address, bad JSON) |
//! | 401 | missing or wrong token |
//! | 429 | rate limited or busy: try again later, or another relay |
//! | 502 | the site couldn't be reached |
//!
//! `GET /health` needs no token and answers a [`Health`].

use serde::{Deserialize, Serialize};

use crate::Profile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    pub profile: Profile,
}

/// The site's answer. `body` is the page for a 2xx and empty otherwise, so `status` 403 with
/// no body is "blocked here too".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchReply {
    pub status: u16,
    pub body: String,
    /// The relay's `RELAY_NAME`, for the server's log.
    pub relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReply {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub name: String,
    pub version: String,
}
