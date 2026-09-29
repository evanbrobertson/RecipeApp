//! Ways to read a recipe from a site that refuses our servers (see [`super::scrape_with`]).
//!
//! - **WordPress**: most recipe blogs run WordPress, whose REST API is often left open when the
//!   pages are behind a bot check. The post's content holds the recipe card, and WP Recipe Maker
//!   (WPRM) and Mediavine Create also serve it as structured data; a site's recipe post type may
//!   serve the whole schema.org recipe (`recipe_schema`). Where the API is, and which post, comes
//!   from the page when it was fetched (see [`super::platforms`]), else it is asked by slug.
//!   Also asked first, or beside the page, for a site known to be WordPress (see
//!   [`super::scrape_plan`]).
//! - **Archive**: the Internet Archive's newest copy of the page.
//!
//! Both are kept to the page they were asked for: the recipe's source stays the original link.

use percent_encoding::percent_decode_str;
use regex::Regex;
use scraper::Html;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

use super::platforms::{
    WpLead, WpRoot, mediavine_id, recipe_from_tasty_markup, recipe_from_wprm_fallback,
    wprm_recipe_id,
};
use super::{
    ApiNote, Fetched, MAX_PAGE_BYTES, Method, ReadError, Scraped, block_lines,
    compute_additional_time, decode_text, fetch_wreq, finish, format_duration, is_block_status,
    notes_from_html, parse_recipe_html, read_capped, recipe_from_json_ld, sel, text_of,
    video_from_html, wreq_client,
};
use crate::model::{RecipeFields, Section};

/// The title `finish` gives a recipe that has none.
const UNTITLED: &str = "Untitled recipe";

/// The Internet Archive is slow to find and replay a copy.
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(25);

// ---------- WordPress ----------

/// The fields asked of a post: its address (to tell it from another post with the same slug),
/// its title and content, and `recipe_schema`, which some sites' recipe post types serve as a
/// whole schema.org Recipe. A field a site doesn't have is left out of the answer.
const POST_FIELDS: &str = "_fields=link,title,content,recipe_schema";

/// The recipe in the site's own WordPress REST API, or why there isn't one (for the log).
/// `lead` is what is already known of the site's API: from site memory, or from the page's
/// `Link` header and head (see [`super::platforms::wp_lead`]).
pub async fn fetch_wordpress(url: &str, lead: Option<WpLead>) -> Fetched {
    match read_wordpress(url, lead.unwrap_or_default()).await {
        Ok(read) => Fetched::Recipe(Box::new(Scraped {
            recipe: read.recipe,
            crumb: None,
            api: Some(ApiNote {
                root: read.root.as_address(),
                kind: read.kind,
            }),
        })),
        Err(why) => {
            tracing::debug!(
                "[scraper] {}: WordPress API: {why}",
                crate::telemetry::host_of(url)
            );
            Fetched::Unreachable(why)
        }
    }
}

/// What the API gave: the recipe, where the API is and which of the site's plugins had it.
struct Read {
    recipe: RecipeFields,
    root: WpRoot,
    kind: &'static str,
}

async fn read_wordpress(url: &str, lead: WpLead) -> Result<Read, String> {
    let page = url::Url::parse(url).map_err(|e| e.to_string())?;
    let root = lead
        .root
        .clone()
        .unwrap_or_else(|| WpRoot::at_origin(&page.origin().ascii_serialization()));
    let mut api = Api::new(root);
    let post = api.find_post(&page, url, lead.post.as_ref()).await?;

    // The recipe plugins' own data are a bonus: the post's content still gives a recipe
    // without them. A site's recipe post type that carries the whole recipe needs neither.
    let mut cards = Cards::default();
    if post.schema.is_none() {
        if let Some(id) = wprm_recipe_id(&post.content) {
            cards.wprm = api.get(&format!("/wp/v2/wprm_recipe/{id}"), "").await.ok();
        }
        if let Some(id) = mediavine_id(&post.content) {
            cards.mediavine = api
                .get(&format!("/mv-create/v1/creations/{id}/json_ld"), "")
                .await
                .ok();
        }
    }
    let (recipe, kind) = recipe_from_post(&post, url, &cards).ok_or("no recipe in the post")?;
    Ok(Read {
        recipe,
        root: api.root,
        kind,
    })
}

/// Why a REST call gave no JSON.
enum Miss {
    /// Not served here: a 404, or a page instead of JSON. The other form of the API's
    /// address may be.
    NotFound(String),
    Other(String),
}

impl Miss {
    fn why(self) -> String {
        match self {
            Miss::NotFound(why) | Miss::Other(why) => why,
        }
    }
}

/// A JSON answer from a REST endpoint. A refusal is retried with the other browser profile.
async fn get_json(url: &str) -> Result<Value, Miss> {
    for method in [Method::Firefox, Method::Safari] {
        match fetch_wreq(method, url).await {
            Fetched::Page { status, html, .. } if (200..300).contains(&status) => {
                return serde_json::from_str(&html)
                    .map_err(|_| Miss::NotFound("the answer wasn't JSON".into()));
            }
            Fetched::Page { status, .. } if is_block_status(status) => continue,
            Fetched::Page { status: 404, .. } => {
                return Err(Miss::NotFound("responded with 404".into()));
            }
            Fetched::Page { status, .. } => {
                return Err(Miss::Other(format!("responded with {status}")));
            }
            Fetched::Unreachable(why) => return Err(Miss::Other(why)),
            Fetched::Recipe(_) => unreachable!("a wreq fetch gives pages"),
        }
    }
    Err(Miss::Other("refused".into()))
}

/// A site's REST API, asked politely: one request at a time.
struct Api {
    root: WpRoot,
    /// Something answered, so the address is right and needs no second form.
    confirmed: bool,
}

impl Api {
    fn new(root: WpRoot) -> Self {
        Self {
            root,
            confirmed: false,
        }
    }

    /// `route` (`/wp/v2/posts`) with its query. Until something has answered, a 404 is tried
    /// once more the other way round: a site without pretty permalinks serves `/wp-json/...`
    /// only as `/?rest_route=/...`.
    async fn get(&mut self, route: &str, query: &str) -> Result<Value, String> {
        match get_json(&self.root.endpoint(route, query)).await {
            Ok(v) => {
                self.confirmed = true;
                Ok(v)
            }
            Err(Miss::NotFound(why)) if !self.confirmed => {
                let other = self.root.other_form();
                match get_json(&other.endpoint(route, query)).await {
                    Ok(v) => {
                        self.root = other;
                        self.confirmed = true;
                        Ok(v)
                    }
                    Err(_) => Err(why),
                }
            }
            Err(miss) => Err(miss.why()),
        }
    }

    /// The post at `url`: its own endpoint when the page named it, else a search by slug in
    /// the posts and then in a recipe post type of the site's.
    async fn find_post(
        &mut self,
        page: &url::Url,
        url: &str,
        named: Option<&(String, u64)>,
    ) -> Result<Post, String> {
        if let Some((base, id)) = named
            && let Ok(v) = self.get(&format!("/wp/v2/{base}/{id}"), POST_FIELDS).await
            && let Some(post) = post_from_object(&v)
        {
            return Ok(post);
        }
        let slug = slug_of(page).ok_or("the link has no post name")?;
        let query = format!("slug={slug}&{POST_FIELDS}");
        let mut last = "no post at that link".to_string();
        match self.get("/wp/v2/posts", &query).await {
            Ok(posts) => {
                if let Some(post) = find_post(&posts, url) {
                    return Ok(post);
                }
            }
            Err(why) => last = why,
        }
        // Some sites keep recipes in a post type of their own (Taste of Home's `recipe`)
        for base in self.recipe_types().await {
            if let Ok(found) = self.get(&format!("/wp/v2/{base}"), &query).await
                && let Some(post) = find_post(&found, url)
            {
                return Ok(post);
            }
        }
        Err(last)
    }

    /// The `rest_base` of the site's post types that look like recipes (`recipe` at least).
    async fn recipe_types(&mut self) -> Vec<String> {
        let mut bases = vec!["recipe".to_string()];
        if let Ok(types) = self.get("/wp/v2/types", "").await
            && let Some(types) = types.as_object()
        {
            for t in types.values() {
                let Some(base) = t.get("rest_base").and_then(Value::as_str) else {
                    continue;
                };
                if base.contains("recipe")
                    && !base.starts_with("wprm_")
                    && !bases.iter().any(|b| b == base)
                    && base
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                {
                    bases.push(base.to_string());
                }
            }
        }
        bases.truncate(3);
        bases
    }
}

/// The post's name in its address: the last part of the path ("/2020/11/21/gingerbread/"
/// is "gingerbread"). Only what can sit in a query value as it is, so nothing has to be escaped.
fn slug_of(url: &url::Url) -> Option<String> {
    let last = url.path_segments()?.rfind(|s| !s.is_empty())?;
    last.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'%'))
        .then(|| last.to_ascii_lowercase())
}

/// Whether two addresses are the same page: the scheme, `www.`, letter case, a trailing slash,
/// the query and the fragment don't tell pages apart, so a site's own link for a post still
/// matches the one that was pasted.
fn same_page(a: &str, b: &str) -> bool {
    let key = |s: &str| {
        let u = url::Url::parse(s).ok()?;
        let host = u.host_str()?.to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
        let path = percent_decode_str(u.path()).decode_utf8_lossy();
        Some((host, u.port(), path.trim_end_matches('/').to_lowercase()))
    };
    matches!((key(a), key(b)), (Some(a), Some(b)) if a == b)
}

/// A WordPress post as the REST API renders it.
struct Post {
    title: String,
    content: String,
    /// The whole recipe as schema.org data, when the post type serves it (`recipe_schema`).
    schema: Option<Value>,
}

/// A post from the API's object for it. `None` when it has neither content nor a schema.
fn post_from_object(post: &Value) -> Option<Post> {
    let rendered = |key: &str| {
        post.get(key)
            .and_then(|v| v.get("rendered"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    // As data, or as the JSON text of it
    let schema = match post.get("recipe_schema") {
        None => None,
        Some(Value::String(text)) => serde_json::from_str::<Value>(text).ok(),
        Some(other) => Some(other.clone()),
    }
    .filter(|s| s.is_object() || s.is_array());
    let content = rendered("content");
    (!content.trim().is_empty() || schema.is_some()).then(|| Post {
        title: rendered("title"),
        content,
        schema,
    })
}

/// The post at `url` among the API's answers for its slug (another post can share a slug
/// under another date, so the link decides).
fn find_post(posts: &Value, url: &str) -> Option<Post> {
    let post = posts.as_array()?.iter().find(|p| {
        p.get("link")
            .and_then(Value::as_str)
            .is_some_and(|link| same_page(link, url))
    })?;
    post_from_object(post)
}

/// What the recipe plugins' own endpoints gave for a post.
#[derive(Default)]
struct Cards {
    /// WP Recipe Maker's card data (`/wp/v2/wprm_recipe/{id}`).
    wprm: Option<Value>,
    /// Mediavine Create's JSON-LD (`/mv-create/v1/creations/{id}/json_ld`).
    mediavine: Option<Value>,
}

/// The recipe in a post, and which plugin's data it came from (for site memory): a schema the
/// post type serves whole; else Mediavine's JSON-LD; else WPRM's data filled in from the
/// card's markup; else the markup alone (JSON-LD or microdata in the content, which WPZOOM's
/// blocks and others leave, then WPRM's, Tasty's and WPRM's fallback card).
fn recipe_from_post(post: &Post, url: &str, cards: &Cards) -> Option<(RecipeFields, &'static str)> {
    let found = |mut recipe: RecipeFields, kind| {
        if recipe.title == UNTITLED {
            recipe.title = decode_text(&post.title);
            if recipe.title.is_empty() {
                recipe.title = UNTITLED.into();
            }
        }
        recipe.url = Some(url.to_string());
        Some((recipe, kind))
    };
    if let Some(recipe) = post
        .schema
        .as_ref()
        .and_then(|schema| recipe_from_json_ld(schema, url))
    {
        return found(recipe, "recipe_schema");
    }
    if let Some(recipe) = cards
        .mediavine
        .as_ref()
        .and_then(|data| recipe_from_json_ld(data, url))
    {
        return found(recipe, "mediavine");
    }

    let page = format!(
        "<!doctype html><html><head><title>{}</title></head><body>{}</body></html>",
        post.title, post.content
    );
    let doc = Html::parse_document(&page);
    let markup = parse_recipe_html(&page, url)
        .map(|r| {
            (
                r,
                if page.contains("ld+json") {
                    "json-ld"
                } else {
                    "microdata"
                },
            )
        })
        .or_else(|| recipe_from_wprm_markup(&doc, url).map(|r| (r, "wprm-markup")))
        .or_else(|| recipe_from_tasty_markup(&doc, url).map(|r| (r, "tasty")))
        .or_else(|| recipe_from_wprm_fallback(&doc, url).map(|r| (r, "wprm-fallback")));
    match (
        cards
            .wprm
            .as_ref()
            .and_then(|data| recipe_from_wprm(data, url)),
        markup,
    ) {
        (Some(mut recipe), Some((markup, _))) => {
            fill_from(&mut recipe, markup);
            found(recipe, "wprm")
        }
        (Some(only), None) => found(only, "wprm"),
        (None, Some((only, kind))) => found(only, kind),
        (None, None) => None,
    }
}

/// Gives `recipe` what it lacks from `other`.
fn fill_from(recipe: &mut RecipeFields, other: RecipeFields) {
    fn or(a: &mut Option<String>, b: Option<String>) {
        if a.is_none() {
            *a = b;
        }
    }
    if recipe.title == UNTITLED {
        recipe.title = other.title;
    }
    or(&mut recipe.description, other.description);
    or(&mut recipe.image, other.image);
    or(&mut recipe.author, other.author);
    or(&mut recipe.prep_time, other.prep_time);
    or(&mut recipe.cook_time, other.cook_time);
    or(&mut recipe.total_time, other.total_time);
    or(&mut recipe.freeze_time, other.freeze_time);
    or(&mut recipe.recipe_yield, other.recipe_yield);
    or(&mut recipe.recipe_category, other.recipe_category);
    or(&mut recipe.recipe_cuisine, other.recipe_cuisine);
    or(&mut recipe.notes, other.notes);
    or(&mut recipe.video, other.video);
    if recipe.ingredients.is_empty() {
        recipe.ingredients = other.ingredients;
    }
    if recipe.instructions.is_empty() {
        recipe.instructions = other.instructions;
    }
    if recipe.nutrition.is_none() {
        recipe.nutrition = other.nutrition;
    }
}

/// A string or number field, trimmed; blank is none.
fn text_field(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

/// WPRM keeps times as whole minutes, as a number or a string; zero is none.
fn minutes_field(v: &Value, key: &str) -> Option<i64> {
    text_field(v, key)?.parse::<i64>().ok().filter(|m| *m > 0)
}

/// The names of a WPRM taxonomy ("course", "cuisine"), as "Dinner, Entree".
fn tag_names(recipe: &Value, taxonomy: &str) -> Option<String> {
    let names: Vec<String> = recipe
        .get("tags")?
        .get(taxonomy)?
        .as_array()?
        .iter()
        .filter_map(|t| match t {
            Value::String(s) => Some(s.clone()),
            other => text_field(other, "name"),
        })
        .collect();
    Some(names.join(", ")).filter(|n| !n.is_empty())
}

/// "1 cup flour, sifted": the parts WPRM keeps apart, joined as the card reads them.
fn ingredient_line(
    amount: Option<String>,
    unit: Option<String>,
    name: Option<String>,
    notes: Option<String>,
) -> String {
    let mut line = [amount, unit, name]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(notes) = notes {
        let glue = if line.is_empty() || notes.starts_with(['(', ',']) {
            " "
        } else {
            ", "
        };
        line = format!("{line}{glue}{notes}");
    }
    line.trim().to_string()
}

/// A group list from WPRM's data (`[{name, <key>: [..]}]`) as sections; `item` reads one entry.
fn wprm_groups(
    groups: Option<&Value>,
    key: &str,
    item: impl Fn(&Value) -> Option<String>,
) -> Vec<Section> {
    groups
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|group| Section {
            name: text_field(group, "name"),
            items: group
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(&item)
                .collect(),
        })
        .collect()
}

/// A WP Recipe Maker recipe, as `/wp-json/wp/v2/wprm_recipe/{id}` serves it (the recipe
/// itself under `recipe`), in the same shape the JSON-LD path gives. `None` when it has
/// neither ingredients nor steps.
pub fn recipe_from_wprm(data: &Value, url: &str) -> Option<RecipeFields> {
    let r = data.get("recipe").filter(|r| r.is_object()).unwrap_or(data);

    let ingredients = wprm_groups(r.get("ingredients"), "ingredients", |i| {
        let line = ingredient_line(
            text_field(i, "amount"),
            text_field(i, "unit"),
            text_field(i, "name"),
            text_field(i, "notes"),
        );
        Some(line).filter(|l| !l.is_empty())
    });
    static BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    let instructions = wprm_groups(r.get("instructions"), "instructions", |step| {
        let text = match step {
            Value::String(s) => Some(s.clone()),
            other => text_field(other, "text"),
        }?;
        Some(BREAK.replace_all(&text, " ").into_owned())
    });

    // Minutes in, the same ISO durations out that the JSON-LD path reads
    let iso = |key: &str| minutes_field(r, key).map(|m| format!("PT{m}M"));
    let (prep, cook, total) = (iso("prep_time"), iso("cook_time"), iso("total_time"));

    let servings = text_field(r, "servings").filter(|s| s != "0");
    let recipe_yield = servings.map(|n| match text_field(r, "servings_unit") {
        Some(unit) => format!("{n} {unit}"),
        None => n,
    });

    let recipe = RecipeFields {
        url: Some(url.to_string()),
        title: text_field(r, "name").unwrap_or_default(),
        description: text_field(r, "summary"),
        image: text_field(r, "image_url"),
        author: text_field(r, "author_name"),
        prep_time: format_duration(prep.as_deref()),
        cook_time: format_duration(cook.as_deref()),
        total_time: format_duration(total.as_deref()),
        freeze_time: compute_additional_time(prep.as_deref(), cook.as_deref(), total.as_deref()),
        recipe_yield,
        recipe_category: tag_names(r, "course"),
        recipe_cuisine: tag_names(r, "cuisine"),
        ingredients,
        instructions,
        nutrition: None,
        notes: text_field(r, "notes").and_then(|html| {
            let lines = block_lines(Html::parse_fragment(&html).root_element());
            Some(lines.join("\n")).filter(|n| !n.is_empty())
        }),
        video: None,
    };
    finish(recipe, url)
}

/// A WPRM card's markup in a post's content: what a recipe has when the API doesn't serve
/// the card as data. Times are left to the data.
fn recipe_from_wprm_markup(doc: &Html, url: &str) -> Option<RecipeFields> {
    let first = |selector: &str| {
        doc.select(&sel(selector))
            .next()
            .map(text_of)
            .filter(|t| !t.is_empty())
    };
    let ingredient_text = |li: scraper::ElementRef| {
        let part = |name: &str| {
            li.select(&sel(&format!(".wprm-recipe-ingredient-{name}")))
                .next()
                .map(text_of)
                .filter(|t| !t.is_empty())
        };
        let (amount, unit, name) = (part("amount"), part("unit"), part("name"));
        if amount.is_none() && unit.is_none() && name.is_none() {
            return text_of(li);
        }
        ingredient_line(amount, unit, name, part("notes"))
    };
    let group_name = |group: scraper::ElementRef| {
        group
            .select(&sel(".wprm-recipe-group-name"))
            .next()
            .map(text_of)
            .filter(|n| !n.is_empty())
    };

    let groups = |group_sel: &str, item_sel: &str, text: &dyn Fn(scraper::ElementRef) -> String| {
        let items = sel(item_sel);
        let mut sections: Vec<Section> = doc
            .select(&sel(group_sel))
            .map(|group| Section {
                name: group_name(group),
                items: group.select(&items).map(text).collect(),
            })
            .collect();
        if sections.is_empty() {
            sections.push(Section::unnamed(doc.select(&items).map(text).collect()));
        }
        sections
    };
    let ingredients = groups(
        ".wprm-recipe-ingredient-group",
        ".wprm-recipe-ingredient",
        &ingredient_text,
    );
    let instructions = groups(
        ".wprm-recipe-instruction-group",
        ".wprm-recipe-instruction-text",
        &|el| block_lines(el).join(" "),
    );
    let servings = first(".wprm-recipe-servings");
    let recipe_yield = servings.map(|n| match first(".wprm-recipe-servings-unit") {
        Some(unit) => format!("{n} {unit}"),
        None => n,
    });
    let image = doc
        .select(&sel(".wprm-recipe-image img"))
        .next()
        .and_then(|img| {
            ["data-lazy-src", "data-src", "data-lazy-srcset", "src"]
                .iter()
                .filter_map(|a| img.value().attr(a))
                .find(|v| !v.is_empty() && !v.starts_with("data:") && !v.contains(' '))
        })
        .map(String::from);

    finish(
        RecipeFields {
            url: Some(url.to_string()),
            title: first(".wprm-recipe-name").unwrap_or_default(),
            description: first(".wprm-recipe-summary"),
            image,
            author: first(".wprm-recipe-author"),
            recipe_yield,
            ingredients,
            instructions,
            notes: notes_from_html(doc),
            video: video_from_html(doc, url),
            ..Default::default()
        },
        url,
    )
}

// ---------- The Internet Archive ----------

/// The Internet Archive's newest copy of `url`, as it was served (`id_`, without the archive's
/// toolbar), read like any page.
pub async fn fetch_archive(url: &str) -> Fetched {
    let Some(client) = wreq_client(Method::Firefox) else {
        return Fetched::Unreachable("client unavailable".into());
    };
    let Some(page) = archive_link(url) else {
        return Fetched::Unreachable("not a link".into());
    };
    let res = match client.get(page).timeout(ARCHIVE_TIMEOUT).send().await {
        Ok(res) => res,
        Err(e) => return Fetched::Unreachable(e.to_string()),
    };
    let status = res.status().as_u16();
    if !res.status().is_success() {
        return Fetched::Page {
            status,
            html: String::new(),
            link: None,
        };
    }
    // Where the archive redirected to names the copy's date
    tracing::debug!(
        "[scraper] {}: Internet Archive copy at {}",
        crate::telemetry::host_of(url),
        res.uri().path().trim_start_matches("/web/")
    );
    match read_capped(res, MAX_PAGE_BYTES).await {
        Ok(body) => Fetched::Page {
            status,
            html: String::from_utf8_lossy(&body).into_owned(),
            link: None,
        },
        Err(ReadError::TooLarge) => Fetched::Unreachable("page too large".into()),
        Err(ReadError::Failed(e)) => Fetched::Unreachable(e),
    }
}

/// The archive's address for the newest copy of a page (`2` is a date it redirects from to the
/// latest copy, `id_` asks for the page unchanged).
fn archive_link(url: &str) -> Option<String> {
    let mut page = url::Url::parse(url).ok()?;
    page.set_fragment(None);
    Some(format!("https://web.archive.org/web/2id_/{page}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const URL: &str = "https://food.test/2020/11/21/ginger-cookies/";

    fn u(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn slug_is_the_last_path_part() {
        assert_eq!(slug_of(&u(URL)).as_deref(), Some("ginger-cookies"));
        assert_eq!(
            slug_of(&u("https://food.test/Ginger-Cookies?utm=1#top")).as_deref(),
            Some("ginger-cookies")
        );
        assert_eq!(
            slug_of(&u("https://food.test/a//b//")).as_deref(),
            Some("b")
        );
        assert_eq!(slug_of(&u("https://food.test/")), None);
        // Nothing that would need escaping in a query
        assert_eq!(slug_of(&u("https://food.test/a&b=c")), None);
        assert_eq!(slug_of(&u("https://food.test/a+b")), None);
    }

    #[test]
    fn same_page_ignores_what_doesnt_tell_pages_apart() {
        for link in [
            URL,
            "https://food.test/2020/11/21/ginger-cookies",
            "http://food.test/2020/11/21/ginger-cookies/",
            "https://www.FOOD.test/2020/11/21/ginger-cookies/?utm_source=x#recipe",
            "https://food.test/2020/11/21/Ginger-Cookies/",
        ] {
            assert!(same_page(link, URL), "{link}");
        }
        for link in [
            "https://food.test/2020/11/22/ginger-cookies/",
            "https://food.test/ginger-cookies/",
            "https://other.test/2020/11/21/ginger-cookies/",
            "https://food.test:8443/2020/11/21/ginger-cookies/",
            "https://food.test/?p=12",
            "not a link",
        ] {
            assert!(!same_page(link, URL), "{link}");
        }
    }

    #[test]
    fn finds_the_post_by_its_link() {
        let posts = json!([
            {"link": "https://food.test/2019/01/01/ginger-cookies/",
             "title": {"rendered": "Old"}, "content": {"rendered": "<p>old</p>"}},
            {"link": URL, "title": {"rendered": "Ginger &amp; Spice"},
             "content": {"rendered": "<p>new</p>"}},
        ]);
        let post = find_post(&posts, URL).unwrap();
        assert_eq!(post.title, "Ginger &amp; Spice");
        assert_eq!(post.content, "<p>new</p>");
        assert!(find_post(&posts, "https://food.test/2021/ginger-cookies/").is_none());
        // What WordPress says when it's not WordPress, or has nothing
        assert!(find_post(&json!([]), URL).is_none());
        assert!(find_post(&json!({"code": "rest_no_route"}), URL).is_none());
        let empty = json!([{"link": URL, "content": {"rendered": "  "}}]);
        assert!(find_post(&empty, URL).is_none());
    }

    /// Hand-written in the shape of WPRM's REST data.
    fn wprm() -> Value {
        json!({"id": 16841, "recipe": {
            "name": "Ginger Cookies",
            "summary": "<p>Spicy &amp; <b>soft</b>.</p>",
            "image_url": "https://food.test/wp-content/uploads/ginger.jpg",
            "servings": 8, "servings_unit": "Cookies",
            "prep_time": 20, "cook_time": "12", "total_time": 92,
            "author_name": "Jo",
            "tags": {"course": [{"name": "Dessert"}], "cuisine": [{"name": "American"}, {"name": "Cozy"}]},
            "ingredients": [
                {"name": "", "ingredients": [
                    {"amount": "2", "unit": "cups", "name": "flour", "notes": "sifted"},
                    {"amount": "0.5", "unit": "cup", "name": "<a href=\"/x\">butter</a>", "notes": "(soft)"},
                    {"amount": "", "unit": "", "name": "", "notes": ""}
                ]},
                {"name": "Icing", "ingredients": [
                    {"amount": "1", "unit": "", "name": "egg white"}
                ]}
            ],
            "instructions": [
                {"name": "", "instructions": [
                    {"text": "<p>Mix.<br>Then chill.</p>"}, {"text": ""}
                ]},
                {"name": "Finish", "instructions": [{"text": "Bake &amp; cool."}]}
            ],
            "notes": "<span style=\"display: block;\">Freeze the dough.</span><span style=\"display: block;\">Keeps a week.</span>"
        }})
    }

    #[test]
    fn maps_a_wprm_recipe() {
        let r = recipe_from_wprm(&wprm(), URL).unwrap();
        assert_eq!(r.url.as_deref(), Some(URL));
        assert_eq!(r.title, "Ginger Cookies");
        assert_eq!(r.description.as_deref(), Some("Spicy & soft."));
        assert_eq!(
            r.image.as_deref(),
            Some("https://food.test/wp-content/uploads/ginger.jpg")
        );
        assert_eq!(r.author.as_deref(), Some("Jo"));
        assert_eq!(r.recipe_yield.as_deref(), Some("8 Cookies"));
        assert_eq!(r.prep_time.as_deref(), Some("20m"));
        assert_eq!(r.cook_time.as_deref(), Some("12m"));
        assert_eq!(r.total_time.as_deref(), Some("1h 32m"));
        assert_eq!(r.freeze_time.as_deref(), Some("1h"));
        assert_eq!(r.recipe_cuisine.as_deref(), Some("American, Cozy"));
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[0].name, None);
        // Fractions come back as the page would show them, notes as the card reads them
        assert_eq!(
            r.ingredients[0].items,
            ["2 cups flour, sifted", "½ cup butter (soft)"]
        );
        assert_eq!(r.ingredients[1].name.as_deref(), Some("Icing"));
        assert_eq!(r.ingredients[1].items, ["1 egg white"]);
        assert_eq!(r.instructions[0].items, ["Mix. Then chill."]);
        assert_eq!(r.instructions[1].name.as_deref(), Some("Finish"));
        assert_eq!(r.instructions[1].items, ["Bake & cool."]);
        assert_eq!(r.notes.as_deref(), Some("Freeze the dough.\nKeeps a week."));
    }

    #[test]
    fn a_bare_or_sparse_wprm_recipe_still_maps() {
        // Not wrapped, with nothing but a title and one ingredient; empty PHP arrays are []
        let r = recipe_from_wprm(
            &json!({"name": "Toast", "tags": [], "servings": "0",
                    "ingredients": [{"ingredients": [{"name": "bread"}]}], "instructions": []}),
            URL,
        )
        .unwrap();
        assert_eq!(r.title, "Toast");
        assert_eq!(r.ingredients[0].items, ["bread"]);
        assert_eq!(r.recipe_yield, None);
        assert_eq!(r.recipe_category, None);
        assert!(r.instructions.is_empty());
        // Nothing to cook from
        assert!(recipe_from_wprm(&json!({"name": "Empty"}), URL).is_none());
        assert!(recipe_from_wprm(&json!("nope"), URL).is_none());
    }

    /// A post's content as WordPress renders it: the WPRM card, and no JSON-LD.
    const CONTENT: &str = r#"<p>My favourite cookies.</p>
        <div id="wprm-recipe-container-16841" class="wprm-recipe-container" data-recipe-id="16841">
          <div class="wprm-recipe-image"><img src="data:image/gif;base64,R0lG" data-lazy-src="https://food.test/wp-content/uploads/g.jpg"></div>
          <h2 class="wprm-recipe-name">Ginger Cookies</h2>
          <div class="wprm-recipe-summary"><span>Soft and spicy.</span></div>
          <div class="wprm-recipe-servings-container"><span class="wprm-recipe-servings">8</span> <span class="wprm-recipe-servings-unit">Cookies</span></div>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Dough</h4><ul>
            <li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-amount">2</span> <span class="wprm-recipe-ingredient-unit">cups</span> <span class="wprm-recipe-ingredient-name">flour</span> <span class="wprm-recipe-ingredient-notes">sifted</span></li>
            <li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-name">1 pinch salt</span></li>
          </ul></div>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Icing</h4><ul>
            <li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-amount">1</span> <span class="wprm-recipe-ingredient-name">egg white</span></li>
          </ul></div>
          <div class="wprm-recipe-instruction-group"><ul>
            <li class="wprm-recipe-instruction"><div class="wprm-recipe-instruction-text"><span style="display: block;">Mix.</span><span style="display: block;">Chill.</span></div></li>
            <li class="wprm-recipe-instruction"><div class="wprm-recipe-instruction-text">Bake.</div></li>
          </ul></div>
          <div class="wprm-recipe-notes-container"><div class="wprm-recipe-notes"><span style="display: block;">Freeze the dough.</span></div></div>
        </div>"#;

    fn post() -> Post {
        Post {
            title: "Ginger Cookies!".into(),
            content: CONTENT.into(),
            schema: None,
        }
    }

    #[test]
    fn reads_a_recipe_from_the_posts_card_markup() {
        // No card data: the markup alone, like a page with no JSON-LD
        let (r, kind) = recipe_from_post(&post(), URL, &Cards::default()).unwrap();
        assert_eq!(kind, "wprm-markup");
        assert_eq!(r.url.as_deref(), Some(URL));
        assert_eq!(r.title, "Ginger Cookies");
        assert_eq!(r.description.as_deref(), Some("Soft and spicy."));
        assert_eq!(
            r.image.as_deref(),
            Some("https://food.test/wp-content/uploads/g.jpg")
        );
        assert_eq!(r.recipe_yield.as_deref(), Some("8 Cookies"));
        assert_eq!(r.ingredients.len(), 2);
        assert_eq!(r.ingredients[0].name.as_deref(), Some("Dough"));
        assert_eq!(
            r.ingredients[0].items,
            ["2 cups flour, sifted", "1 pinch salt"]
        );
        assert_eq!(r.ingredients[1].items, ["1 egg white"]);
        assert_eq!(r.instructions[0].items, ["Mix. Chill.", "Bake."]);
        assert_eq!(r.notes.as_deref(), Some("Freeze the dough."));
    }

    #[test]
    fn the_cards_data_wins_and_the_markup_fills_the_gaps() {
        let data = json!({"recipe": {"name": "Ginger Cookies (data)", "prep_time": 20,
            "ingredients": [{"ingredients": [{"amount": "3", "name": "eggs"}]}]}});
        let cards = Cards {
            wprm: Some(data),
            ..Default::default()
        };
        let (r, kind) = recipe_from_post(&post(), URL, &cards).unwrap();
        assert_eq!(kind, "wprm");
        assert_eq!(r.title, "Ginger Cookies (data)");
        assert_eq!(r.prep_time.as_deref(), Some("20m"));
        assert_eq!(r.ingredients.len(), 1);
        assert_eq!(r.ingredients[0].items, ["3 eggs"]);
        // Steps, summary and image weren't in the data
        assert_eq!(r.instructions[0].items, ["Mix. Chill.", "Bake."]);
        assert_eq!(r.description.as_deref(), Some("Soft and spicy."));
        assert!(r.image.is_some());
        assert_eq!(r.url.as_deref(), Some(URL));
    }

    #[test]
    fn the_post_title_stands_in_for_a_missing_recipe_name() {
        let post = Post {
            title: "Mac &amp; Cheese".into(),
            content: "<div class=\"wprm-recipe-ingredient-group\"><ul><li class=\"wprm-recipe-ingredient\">1 lb macaroni</li></ul></div>".into(),
            schema: None,
        };
        let (r, _) = recipe_from_post(&post, URL, &Cards::default()).unwrap();
        assert_eq!(r.title, "Mac & Cheese");
        assert_eq!(r.ingredients[0].items, ["1 lb macaroni"]);
    }

    #[test]
    fn a_post_without_a_recipe_gives_none() {
        let post = Post {
            title: "Life update".into(),
            content: "<p>We moved house.</p>".into(),
            schema: None,
        };
        assert!(recipe_from_post(&post, URL, &Cards::default()).is_none());
    }

    fn content_post(content: &str) -> Post {
        Post {
            title: "Some Post".into(),
            content: content.into(),
            schema: None,
        }
    }

    /// A schema.org Recipe as a site's recipe post type serves it (hand-written).
    fn schema() -> Value {
        json!({"@context": "https://schema.org", "@type": "Recipe", "name": "Stuffed Peppers",
            "recipeYield": "4 servings", "prepTime": "PT20M", "cookTime": "PT45M",
            "recipeIngredient": ["4 bell peppers", "1 cup cooked rice"],
            "recipeInstructions": [{"@type": "HowToStep", "text": "Fill the peppers."},
                                   {"@type": "HowToStep", "text": "Bake."}]})
    }

    #[test]
    fn a_recipe_post_type_serving_its_schema_is_read_as_json_ld() {
        // As an object, or as the JSON text of one
        for schema in [schema(), Value::String(schema().to_string())] {
            let posts = json!([{"link": URL, "title": {"rendered": "Peppers"},
                "content": {"rendered": ""}, "recipe_schema": schema}]);
            let post = find_post(&posts, URL).expect("a schema stands in for content");
            let (r, kind) = recipe_from_post(&post, URL, &Cards::default()).unwrap();
            assert_eq!(kind, "recipe_schema");
            assert_eq!(r.title, "Stuffed Peppers");
            assert_eq!(
                r.ingredients[0].items,
                ["4 bell peppers", "1 cup cooked rice"]
            );
            assert_eq!(r.instructions[0].items, ["Fill the peppers.", "Bake."]);
            assert_eq!(r.prep_time.as_deref(), Some("20m"));
            assert_eq!(r.url.as_deref(), Some(URL));
        }
        // Junk in the field is no schema
        let posts = json!([{"link": URL, "content": {"rendered": "  "}, "recipe_schema": "nope"}]);
        assert!(find_post(&posts, URL).is_none());
    }

    #[test]
    fn mediavine_json_ld_becomes_the_recipe() {
        // What /mv-create/v1/creations/{id}/json_ld answers (hand-written)
        let cards = Cards {
            mediavine: Some(schema()),
            ..Default::default()
        };
        let post =
            content_post(r#"<div id="mv-creation-903" class="mv-create-wrapper">card</div>"#);
        assert_eq!(mediavine_id(&post.content), Some(903));
        let (r, kind) = recipe_from_post(&post, URL, &cards).unwrap();
        assert_eq!(kind, "mediavine");
        assert_eq!(r.title, "Stuffed Peppers");
        assert_eq!(r.recipe_yield.as_deref(), Some("4 servings"));
        assert_eq!(r.instructions[0].items.len(), 2);
    }

    #[test]
    fn json_ld_left_in_the_content_is_read() {
        // WPZOOM's Recipe Card blocks write the structured data into the block's content
        let post = content_post(&format!(
            r#"<div class="wp-block-wpzoom-recipe-card-block-recipe-card"><script type="application/ld+json">{}</script></div>"#,
            schema()
        ));
        let (r, kind) = recipe_from_post(&post, URL, &Cards::default()).unwrap();
        assert_eq!(kind, "json-ld");
        assert_eq!(r.title, "Stuffed Peppers");
    }

    #[test]
    fn a_tasty_card_in_the_content_is_read() {
        let post = content_post(
            r#"<div class="tasty-recipes"><h2 class="tasty-recipes-title">Skillet Pie</h2>
               <div class="tasty-recipes-ingredients-body"><ul>
                 <li><span data-amount="2" data-unit="cups">2 cups</span> flour</li></ul></div>
               <div class="tasty-recipes-instructions-body"><ol><li>Bake.</li></ol></div></div>"#,
        );
        let (r, kind) = recipe_from_post(&post, URL, &Cards::default()).unwrap();
        assert_eq!(kind, "tasty");
        assert_eq!(r.title, "Skillet Pie");
        assert_eq!(r.ingredients[0].items, ["2 cups flour"]);
    }

    #[test]
    fn the_wprm_fallback_card_and_its_comment_marker_are_read() {
        // Just One Cookbook's REST content: a comment marker and the fallback card, no
        // container id and no JSON-LD
        let post = content_post(
            r#"<p>Intro.</p><!--WPRM Recipe 4521-->
               <div class="wprm-fallback-recipe"><h2 class="wprm-fallback-recipe-name">Miso Soup</h2>
               <ul class="wprm-fallback-recipe-ingredients"><li class="wprm-fallback-recipe-ingredient">4 cups dashi</li></ul>
               <ol class="wprm-fallback-recipe-instructions"><li class="wprm-fallback-recipe-instruction">Warm it.</li></ol></div>"#,
        );
        assert_eq!(wprm_recipe_id(&post.content), Some(4521));
        let (r, kind) = recipe_from_post(&post, URL, &Cards::default()).unwrap();
        assert_eq!(kind, "wprm-fallback");
        assert_eq!(r.title, "Miso Soup");
        assert_eq!(r.instructions[0].items, ["Warm it."]);
    }

    /// Answers each request with `answer(path_and_query)`: (status, body). Returns the origin.
    async fn serve_paths(answer: fn(&str) -> (u16, String)) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let (status, body) = answer(&path);
                    let response = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        origin
    }

    #[tokio::test]
    async fn a_site_without_wp_json_is_asked_through_rest_route() {
        // /wp-json is a 404 page; the same API answers under ?rest_route=
        let origin = serve_paths(|path| {
            if path.starts_with("/?rest_route=/wp/v2/posts") {
                let link = "http://SITE/2020/11/21/ginger-cookies/";
                (
                    200,
                    json!([{"link": link, "title": {"rendered": "Peppers"},
                        "content": {"rendered": ""}, "recipe_schema": schema()}])
                    .to_string(),
                )
            } else {
                (404, "<html>not found</html>".into())
            }
        })
        .await;
        // The site's own links name the same host as the pasted one
        let url = format!("{origin}/2020/11/21/ginger-cookies/");
        let host = origin.trim_start_matches("http://");
        let mut api = Api::new(WpRoot::at_origin(&origin));
        let posts = api
            .get("/wp/v2/posts", "slug=ginger-cookies")
            .await
            .expect("the query form answers");
        assert!(api.root.is_query_form());
        let text = posts.to_string().replace("SITE", host);
        let posts: Value = serde_json::from_str(&text).unwrap();
        assert!(find_post(&posts, &url).is_some());
    }

    #[tokio::test]
    async fn the_post_the_page_named_is_fetched_by_id_and_a_missing_api_is_an_error() {
        let origin = serve_paths(|path| {
            if path.starts_with("/wp-json/wp/v2/recipe/77") {
                (
                    200,
                    json!({"link": "http://x.test/r", "title": {"rendered": "Peppers"},
                        "content": {"rendered": ""}, "recipe_schema": schema()})
                    .to_string(),
                )
            } else {
                (404, "{}".into())
            }
        })
        .await;
        let url = format!("{origin}/some/other-name/");
        let lead = WpLead {
            root: None,
            post: Some(("recipe".into(), 77)),
        };
        let read = read_wordpress(&url, lead).await.expect("read by its id");
        assert_eq!(read.kind, "recipe_schema");
        assert_eq!(read.recipe.title, "Stuffed Peppers");
        assert!(!read.root.is_query_form());

        // Nothing there at all
        let dead = serve_paths(|_| (404, "nope".into())).await;
        let err = read_wordpress(&format!("{dead}/a/b/"), WpLead::default())
            .await
            .err()
            .expect("no API");
        assert!(!err.is_empty());
    }

    #[test]
    fn builds_the_archive_address() {
        assert_eq!(
            archive_link("https://food.test/r/?a=1#top").as_deref(),
            Some("https://web.archive.org/web/2id_/https://food.test/r/?a=1")
        );
        assert_eq!(archive_link("nope"), None);
    }
}
