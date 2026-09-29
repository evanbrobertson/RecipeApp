//! Errors a Crumb client call can return, with messages fit to show a person.

use std::fmt;

/// What went wrong talking to Crumb.
#[derive(Debug)]
pub enum Error {
    /// The address given isn't an http(s) URL.
    InvalidUrl,
    /// The server says we're not signed in (401): send the user back to login.
    Unauthorized,
    /// The server answered with an error; `message` is its human-readable text.
    Api {
        status: u16,
        message: String,
        /// The server's machine-readable reason, when it gives one (`"site_blocked"`).
        code: Option<String>,
        /// With `"site_terms"`: the listed site's name ("Allrecipes").
        site: Option<String>,
    },
    /// The request never reached the server, or the connection failed.
    Network(String),
    /// The server answered, but not with the JSON we expected.
    Decode(String),
}

impl Error {
    /// The server's machine-readable reason for an [`Error::Api`], e.g. `"site_blocked"`
    /// when a recipe site's bot check turned the server away (the browser extension can
    /// still read it).
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    /// With `"site_terms"`, the name of the site whose terms forbid automated fetching.
    pub fn site(&self) -> Option<&str> {
        match self {
            Self::Api { site, .. } => site.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl => f.write_str("That isn't a valid Crumb address."),
            Self::Unauthorized => f.write_str("You're not signed in to Crumb."),
            Self::Api { message, .. } if !message.is_empty() => f.write_str(message),
            Self::Api { status, .. } => write!(f, "Crumb returned an error ({status})."),
            Self::Network(detail) => write!(f, "Couldn't reach Crumb: {detail}"),
            Self::Decode(detail) => write!(f, "Crumb sent something unexpected: {detail}"),
        }
    }
}

impl std::error::Error for Error {}
