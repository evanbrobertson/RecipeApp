//! Popular: recipe links that several households have saved, for a cook looking for
//! something new (the Add page). Only with accounts or hosted households, where there are
//! several boxes.
//!
//! What it shows is deliberately little, so nothing of anyone's box leaks: a public recipe
//! page's link, its host, how many boxes hold it, and a title only when enough households
//! saved it under the same one (their own edits stay theirs; otherwise the title comes from
//! the link). Never a photo, notes, ingredients or which household saved what. A link counts
//! only once [`Popular::min_households`] (`POPULAR_MIN_HOUSEHOLDS`, at least 2) have it, and
//! a household that opts out (More, [`set_opted_out`]) isn't counted.
//!
//! Nothing is stored: the counts are worked out from the boxes every few hours and kept in
//! memory. Opening one goes to the preview, which reads the page like any other link.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::AppState;
use crate::error::AppResult;

/// How long a count is kept before the boxes are read again.
const FRESH_FOR: Duration = Duration::from_secs(6 * 3600);
/// The most links kept (and shown).
pub const MAX_LINKS: usize = 30;
const OPT_OUT_KEY: &str = "popular_opt_out";

/// Query parameters that only say where a click came from; dropped so one page counts once.
const TRACKING: [&str; 9] = [
    "fbclid", "gclid", "mc_cid", "mc_eid", "igshid", "ref", "ref_src", "si", "share",
];
/// Query parameters that suggest the link is someone's own (a login, a signed address).
const PRIVATE: [&str; 8] = [
    "token",
    "key",
    "auth",
    "session",
    "sig",
    "signature",
    "password",
    "code",
];

/// A popular link.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// The link, tidied: https where it was, no fragment, no tracking parameters.
    pub url: String,
    /// The link's host, without `www.`.
    pub host: String,
    /// The title the households agree on, else one read from the link.
    pub title: String,
    /// How many households saved it.
    pub households: usize,
}

impl Link {
    pub fn to_json(&self) -> Value {
        json!({
            "url": self.url,
            "host": self.host,
            "title": self.title,
            "households": self.households,
        })
    }
}

/// The latest count, kept for a few hours.
#[derive(Default)]
pub struct Popular {
    cache: tokio::sync::Mutex<Option<(Instant, std::sync::Arc<Vec<Link>>)>>,
}

impl Popular {
    /// Whether Popular runs at all on this server: on (not `POPULAR=off`) with several
    /// households (accounts or hosted mode).
    pub fn enabled(state: &AppState) -> bool {
        state.config.popular && state.accounts.is_some()
    }

    pub fn min_households(state: &AppState) -> usize {
        state.config.popular_min_households.max(2)
    }

    /// The popular links, counted again when the last count is older than [`FRESH_FOR`].
    pub async fn links(&self, state: &AppState) -> std::sync::Arc<Vec<Link>> {
        let mut cache = self.cache.lock().await;
        if let Some((at, links)) = cache.as_ref()
            && at.elapsed() < FRESH_FOR
        {
            return links.clone();
        }
        let scan_state = state.clone();
        let links = tokio::task::spawn_blocking(move || count(&scan_state))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("[popular] couldn't count: {e}");
                Vec::new()
            });
        let links = std::sync::Arc::new(links);
        *cache = Some((Instant::now(), links.clone()));
        links
    }

    /// Forgets the count, so the next ask counts again (a household opted in or out).
    pub async fn forget(&self) {
        *self.cache.lock().await = None;
    }
}

/// Whether this box's household opted out of Popular.
pub fn opted_out(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM box_settings WHERE key = ?1",
        [OPT_OUT_KEY],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .is_some_and(|v| v == "1")
}

pub fn set_opted_out(conn: &Connection, out: bool) -> AppResult<()> {
    if out {
        conn.execute(
            "INSERT OR REPLACE INTO box_settings (key, value) VALUES (?1, '1')",
            [OPT_OUT_KEY],
        )?;
    } else {
        conn.execute("DELETE FROM box_settings WHERE key = ?1", [OPT_OUT_KEY])?;
    }
    Ok(())
}

/// A saved link as Popular counts it (the key two households' copies share), or None when it
/// isn't a public recipe page: not http(s), an address or a local name, a login or signed
/// link, or another Crumb's share.
pub fn tidy(raw: &str, own_host: Option<&str>) -> Option<(String, String)> {
    let mut url = url::Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let host = match url.host()? {
        url::Host::Domain(d) => d.trim_end_matches('.').to_ascii_lowercase(),
        _ => return None,
    };
    let local = [
        "localhost",
        ".local",
        ".internal",
        ".lan",
        ".home.arpa",
        ".test",
    ];
    if !host.contains('.')
        || local
            .iter()
            .any(|l| host == l.trim_start_matches('.') || host.ends_with(l))
    {
        return None;
    }
    let bare = host.strip_prefix("www.").unwrap_or(&host).to_string();
    if own_host.is_some_and(|own| own.strip_prefix("www.").unwrap_or(own) == bare) {
        return None;
    }
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if pairs
        .iter()
        .any(|(k, _)| PRIVATE.contains(&k.to_ascii_lowercase().as_str()))
    {
        return None;
    }
    let kept: Vec<_> = pairs
        .into_iter()
        .filter(|(k, _)| {
            let k = k.to_ascii_lowercase();
            !k.starts_with("utm_") && !TRACKING.contains(&k.as_str())
        })
        .collect();
    url.set_fragment(None);
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(kept);
    }
    let _ = url.set_host(Some(&host));
    let path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        // A site's home page isn't a recipe
        return None;
    }
    let key = format!(
        "{bare}{path}{}",
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    Some((key, url.to_string()))
}

/// A title read from the link's last part: `/recipes/best-banana-bread-123/` is "Best banana
/// bread".
fn title_from_link(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let slug = parsed
        .path_segments()?
        .rev()
        .find(|s| s.chars().any(|c| c.is_ascii_alphabetic()))?
        .to_string();
    let slug = slug
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() <= 4)
        .map_or(slug.as_str(), |(stem, _)| stem)
        .to_string();
    let words: Vec<String> = slug
        .split(['-', '_', '+'])
        .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()))
        .filter(|w| !w.eq_ignore_ascii_case("recipe"))
        .map(|w| {
            percent_encoding::percent_decode_str(w)
                .decode_utf8_lossy()
                .to_lowercase()
        })
        .collect();
    let mut title = words.join(" ");
    let first = title.chars().next()?;
    title.replace_range(..first.len_utf8(), &first.to_uppercase().to_string());
    Some(title)
}

#[derive(Default)]
struct Tally {
    url: String,
    households: HashSet<i64>,
    /// Each title (lowercased) and the households that saved the link under it.
    titles: HashMap<String, (String, HashSet<i64>)>,
}

/// Reads every opted-in household's saved links and keeps those enough of them share.
fn count(state: &AppState) -> Vec<Link> {
    let Some(accounts) = &state.accounts else {
        return Vec::new();
    };
    let ids = match accounts.household_ids() {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!("[popular] couldn't list the households: {e:?}");
            return Vec::new();
        }
    };
    let own = own_host(state);
    let started = Instant::now();
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let mut counted = 0;
    for id in ids {
        let Some(household) = state.households.existing(id) else {
            continue;
        };
        let conn = household.db.lock();
        if opted_out(&conn) {
            continue;
        }
        counted += 1;
        let Ok(mut stmt) = conn.prepare(
            "SELECT url, title FROM recipes
             WHERE url IS NOT NULL AND original_url IS NULL AND source IN ('url', 'video')",
        ) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        else {
            continue;
        };
        for (url, title) in rows.flatten() {
            let Some((key, tidied)) = tidy(&url, own.as_deref()) else {
                continue;
            };
            let tally = tallies.entry(key).or_default();
            if tally.url.is_empty() {
                tally.url = tidied;
            }
            tally.households.insert(id);
            let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
            if !title.is_empty() {
                tally
                    .titles
                    .entry(title.to_lowercase())
                    .or_insert_with(|| (title.clone(), HashSet::new()))
                    .1
                    .insert(id);
            }
        }
    }
    let min = Popular::min_households(state);
    let mut links: Vec<Link> = tallies
        .into_values()
        .filter(|t| t.households.len() >= min)
        .filter_map(|t| {
            let host = crate::share::host_of(&t.url)?;
            let agreed = t
                .titles
                .values()
                .filter(|(_, who)| who.len() >= min)
                .max_by_key(|(_, who)| who.len())
                .map(|(title, _)| title.clone());
            let title = agreed
                .or_else(|| title_from_link(&t.url))
                .unwrap_or_else(|| host.clone());
            Some(Link {
                host,
                title,
                households: t.households.len(),
                url: t.url,
            })
        })
        .collect();
    links.sort_by(|a, b| {
        b.households
            .cmp(&a.households)
            .then_with(|| a.title.cmp(&b.title))
    });
    links.truncate(MAX_LINKS);
    tracing::info!(
        "[popular] {} links from {counted} households ({} ms)",
        links.len(),
        started.elapsed().as_millis()
    );
    links
}

/// This server's own host, whose links are other boxes' shares.
fn own_host(state: &AppState) -> Option<String> {
    let site = state.config.site_url.as_deref()?;
    url::Url::parse(site)
        .ok()?
        .host_str()
        .map(str::to_ascii_lowercase)
}

/// The popular links for this household: those it hasn't saved itself.
pub async fn for_household(state: &AppState) -> Vec<Link> {
    if !Popular::enabled(state) {
        return Vec::new();
    }
    let links = state.popular.links(state).await;
    let own = own_host(state);
    let saved: HashSet<String> = {
        let conn = state.db.lock();
        let Ok(mut stmt) = conn.prepare("SELECT url FROM recipes WHERE url IS NOT NULL") else {
            return Vec::new();
        };
        stmt.query_map([], |r| r.get::<_, String>(0))
            .map(|rows| {
                rows.flatten()
                    .filter_map(|u| tidy(&u, own.as_deref()).map(|(k, _)| k))
                    .collect()
            })
            .unwrap_or_default()
    };
    links
        .iter()
        .filter(|l| tidy(&l.url, own.as_deref()).is_some_and(|(k, _)| !saved.contains(&k)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_tidied_to_one_key_per_page() {
        let (a, url) = tidy(
            "https://www.Food.example/pie/?utm_source=x&fbclid=1#comments",
            None,
        )
        .unwrap();
        let (b, _) = tidy("http://food.example/pie", None).unwrap();
        assert_eq!(a, b);
        assert_eq!(url, "https://www.food.example/pie/");
        let (with_id, _) = tidy("https://food.example/r?id=12&utm_medium=y", None).unwrap();
        assert_eq!(with_id, "food.example/r?id=12");
    }

    #[test]
    fn only_public_recipe_pages_count() {
        for private in [
            "https://192.168.1.4/pie",
            "http://localhost/pie",
            "https://nas.local/pie",
            "https://food.example:8443/pie",
            "https://me:pw@food.example/pie",
            "https://food.example/pie?token=abc",
            "https://food.example/",
            "ftp://food.example/pie",
            "https://crumb.example/s/abc123",
        ] {
            assert!(tidy(private, Some("crumb.example")).is_none(), "{private}");
        }
    }

    #[test]
    fn a_title_comes_from_the_link_when_households_disagree() {
        assert_eq!(
            title_from_link("https://food.example/recipes/best-banana-bread-123/").as_deref(),
            Some("Best banana bread")
        );
        assert_eq!(
            title_from_link("https://food.example/2024/05/chicken_curry-recipe.html").as_deref(),
            Some("Chicken curry")
        );
    }
}
