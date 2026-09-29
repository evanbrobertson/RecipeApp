//! A recipe as the cook's own browser read it from the page (the Crumb extension), sent with
//! the link as `page` to `POST /api/recipes/import` and `POST /api/preview`. It is for sites
//! that turn the server away but not the cook: the browser is already past the check.
//!
//! It is whatever the caller sends, so it is untrusted: capped in size, and only parsed
//! (never fetched, run or shown as HTML). It is put into a minimal document and read by the
//! same parser as a scraped page. The address it is saved under is always the link, never
//! anything the payload names; a relative image is resolved against that link.

use serde::Deserialize;

use super::Scraped;
use crate::error::{AppError, AppResult};

/// The most the browser may send, in bytes of text (the extension keeps to the same cap).
pub const MAX_BYTES: usize = 512 * 1024;
/// The most JSON-LD scripts read; a page has a handful.
const MAX_SCRIPTS: usize = 32;
const MAX_TITLE_CHARS: usize = 500;
const MAX_IMAGE_CHARS: usize = 2_000;

/// What the extension read: only the recipe's data, never the rest of the page.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FromPage {
    /// The text of each `<script type="application/ld+json">` that holds a Recipe.
    pub json_ld: Vec<String>,
    /// The recipe card's HTML, when no JSON-LD had the recipe.
    pub card: Option<String>,
    /// The page's `og:image`.
    pub image: Option<String>,
    /// The page's `og:title`.
    pub title: Option<String>,
}

fn attr(value: &str) -> String {
    crate::share::escape(value)
}

/// Text cut to `max` characters (not bytes).
fn clip(value: &str, max: usize) -> &str {
    match value.char_indices().nth(max) {
        Some((cut, _)) => &value[..cut],
        None => value,
    }
}

impl FromPage {
    /// Bytes of text held.
    fn size(&self) -> usize {
        self.json_ld.iter().map(String::len).sum::<usize>()
            + self.card.as_deref().map_or(0, str::len)
            + self.image.as_deref().map_or(0, str::len)
            + self.title.as_deref().map_or(0, str::len)
    }

    /// Reads `value` (the request's `page`), or says why it can't be: not the right shape (400)
    /// or over [`MAX_BYTES`] (413).
    pub fn from_json(value: &serde_json::Value) -> AppResult<Self> {
        let page: Self = serde_json::from_value(value.clone())
            .map_err(|_| AppError::bad_request("That page's recipe didn't make sense"))?;
        if page.size() > MAX_BYTES || page.json_ld.len() > MAX_SCRIPTS {
            return Err(AppError::new(
                413,
                "That page's recipe is too big to read (512 KB max)",
            ));
        }
        Ok(page)
    }

    /// The minimal document the parser reads: the meta tags, the JSON-LD (only what parses
    /// as JSON, with `<` escaped so nothing in it can end the script), and the card.
    fn document(&self) -> String {
        let mut html = String::from("<!doctype html><html><head><meta charset=\"utf-8\">");
        let meta = |property: &str, value: &Option<String>, max: usize| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| {
                    format!(
                        "<meta property=\"{property}\" content=\"{}\">",
                        attr(clip(v, max))
                    )
                })
                .unwrap_or_default()
        };
        html.push_str(&meta("og:title", &self.title, MAX_TITLE_CHARS));
        html.push_str(&meta("og:image", &self.image, MAX_IMAGE_CHARS));
        for block in &self.json_ld {
            if serde_json::from_str::<serde_json::Value>(block).is_err() {
                continue;
            }
            html.push_str("<script type=\"application/ld+json\">");
            // Outside a string `<` can't occur in JSON, so this changes no meaning
            html.push_str(&block.replace('<', "\\u003c"));
            html.push_str("</script>");
        }
        html.push_str("</head><body>");
        if let Some(card) = &self.card {
            html.push_str(card);
        }
        html.push_str("</body></html>");
        html
    }

    /// The recipe on the page, saved under `url` (the link the cook was on); None when the
    /// reading holds no recipe, and the caller scrapes as usual.
    pub fn scrape(&self, url: &str) -> Option<Scraped> {
        if self.json_ld.is_empty() && self.card.as_deref().is_none_or(|c| c.trim().is_empty()) {
            return None;
        }
        let mut scraped = Scraped::from_page(&self.document(), url)?;
        // Another Crumb's share is fetched from its export, which a reading can't name
        if scraped.crumb.is_some() || scraped.recipe.title.trim().is_empty() {
            return None;
        }
        // JSON-LD without a photo: the page's own preview image (a link kept as it is; /img
        // sends the browser to it when its host turns the server away)
        if scraped.recipe.image.is_none() {
            let image = self
                .image
                .as_deref()
                .map(|i| clip(i.trim(), MAX_IMAGE_CHARS));
            scraped.recipe.image =
                super::absolute_url(image, url).filter(|i| crate::model::is_valid_url(i));
        }
        Some(scraped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const URL: &str = "https://food.test/pie";

    fn ld(recipe: &str) -> String {
        format!(
            r#"{{"@context":"https://schema.org","@graph":[{{"@type":"WebSite"}},{{"@type":"Recipe","name":"{recipe}","recipeIngredient":["1 cup flour","2 eggs"],"recipeInstructions":[{{"@type":"HowToStep","text":"Mix."}}],"url":"https://evil.test/x"}}]}}"#
        )
    }

    #[test]
    fn json_ld_in_a_graph_is_read_and_saved_under_the_link() {
        let page = FromPage {
            json_ld: vec![ld("Pie")],
            ..Default::default()
        };
        let scraped = page.scrape(URL).unwrap();
        assert_eq!(scraped.recipe.title, "Pie");
        assert_eq!(scraped.recipe.url.as_deref(), Some(URL));
    }

    #[test]
    fn the_card_is_read_when_there_is_no_json_ld() {
        let page = FromPage {
            card: Some(
                r#"<div itemscope itemtype="https://schema.org/Recipe"><h2 itemprop="name">Card Pie</h2>
                <ul><li itemprop="recipeIngredient">1 cup flour</li><li itemprop="recipeIngredient">2 eggs</li></ul>
                <div itemprop="recipeInstructions"><p>Mix it.</p></div></div>"#
                    .into(),
            ),
            image: Some("/pie.jpg".into()),
            ..Default::default()
        };
        let scraped = page.scrape(URL).unwrap();
        assert_eq!(scraped.recipe.title, "Card Pie");
        assert_eq!(
            scraped.recipe.image.as_deref(),
            Some("https://food.test/pie.jpg")
        );
    }

    #[test]
    fn the_page_image_fills_in_for_json_ld_without_one_and_is_http_only() {
        let page = |image: &str| FromPage {
            json_ld: vec![ld("Pie")],
            image: Some(image.into()),
            ..Default::default()
        };
        let got = page("https://cdn.test/pie.jpg").scrape(URL).unwrap();
        assert_eq!(
            got.recipe.image.as_deref(),
            Some("https://cdn.test/pie.jpg")
        );
        assert_eq!(
            page("javascript:alert(1)")
                .scrape(URL)
                .unwrap()
                .recipe
                .image,
            None
        );
        assert_eq!(
            page("data:image/png;base64,AAAA")
                .scrape(URL)
                .unwrap()
                .recipe
                .image,
            None
        );
    }

    #[test]
    fn nothing_readable_gives_no_recipe() {
        assert!(FromPage::default().scrape(URL).is_none());
        let bad = FromPage {
            json_ld: vec!["{not json".into(), r#"{"@type":"Article"}"#.into()],
            ..Default::default()
        };
        assert!(bad.scrape(URL).is_none());
    }

    #[test]
    fn a_script_end_inside_the_json_cannot_break_out() {
        let tricky = r#"{"@type":"Recipe","name":"</script><b>x","recipeIngredient":["a","b"],"recipeInstructions":"Do."}"#;
        let page = FromPage {
            json_ld: vec![tricky.into()],
            ..Default::default()
        };
        // The recipe is read whole (the parser strips the markup from the title)
        let recipe = page.scrape(URL).unwrap().recipe;
        assert_eq!(recipe.title, "x");
        assert_eq!(recipe.ingredients[0].items.len(), 2);
    }

    #[test]
    fn size_and_shape_are_checked() {
        let big = json!({"jsonLd": ["x".repeat(MAX_BYTES + 1)]});
        assert_eq!(FromPage::from_json(&big).unwrap_err().status, 413);
        let many = json!({"jsonLd": vec!["{}"; MAX_SCRIPTS + 1]});
        assert_eq!(FromPage::from_json(&many).unwrap_err().status, 413);
        let wrong = json!({"jsonLd": "not a list"});
        assert_eq!(FromPage::from_json(&wrong).unwrap_err().status, 400);
        assert!(FromPage::from_json(&json!({"jsonLd": [], "canonical": "x"})).is_ok());
    }
}
