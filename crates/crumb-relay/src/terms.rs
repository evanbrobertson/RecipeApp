//! Sites whose terms forbid automated fetching (`data/site-terms.toml`, the server's list,
//! compiled in here too). The server never asks a relay for one, but a page can redirect or
//! link to one, and Chromium follows: every check the relay makes (`crumb_fetch::guard`,
//! Chromium's proxy included) refuses these hosts, as the server's do.

use std::sync::LazyLock;

use serde::Deserialize;

const LIST: &str = include_str!("../../../data/site-terms.toml");

#[derive(Deserialize)]
struct List {
    #[serde(default)]
    site: Vec<Site>,
}

#[derive(Deserialize)]
struct Site {
    hosts: Vec<String>,
    #[serde(default)]
    image_hosts: Vec<String>,
}

static HOSTS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let list: List = toml::from_str(LIST).expect("data/site-terms.toml");
    list.site
        .into_iter()
        .flat_map(|s| s.hosts.into_iter().chain(s.image_hosts))
        .map(|h| h.trim_start_matches("www.").to_ascii_lowercase())
        .collect()
});

/// Whether `host` (lowercase, no trailing dot) is a listed site or one of its subdomains.
pub fn host_is_listed(host: &str) -> bool {
    HOSTS.iter().any(|listed| {
        host == listed
            || host
                .strip_suffix(listed.as_str())
                .is_some_and(|rest| rest.ends_with('.'))
    })
}

/// Makes every guard check in this process refuse the listed hosts.
pub fn install() {
    crumb_fetch::guard::set_veto(host_is_listed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_sites_and_their_subdomains_are_refused() {
        assert!(host_is_listed("allrecipes.com"));
        assert!(host_is_listed("www.allrecipes.com"));
        assert!(!host_is_listed("notallrecipes.com"));
        assert!(!host_is_listed("budgetbytes.com"));
    }
}
