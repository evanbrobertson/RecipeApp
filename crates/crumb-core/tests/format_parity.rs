//! Parity tests: every case in `fixtures/format.json` (generated from the real
//! `web/src/lib/format.ts` and `web/src/lib/recipe.ts` by `fixtures/gen-format.ts`)
//! must produce the same output from the Rust port.
//!
//! `isShareLink`, `publicSource` and `countItems` are not re-implemented here: the
//! fixtures check the Rust helpers the server already uses (`source::is_share_link`,
//! `source::source_url` and `model::count_items`). `recipeToMarkdown` is checked
//! against the new `format::recipe_to_markdown`; `markdown::recipe_to_markdown` is
//! the connector's separate rendering (see the module comment).

use crumb_core::format::{
    book_imported_title, book_to_text, host_of, kicker, recipe_to_markdown, recipe_to_text,
    web_link,
};
use crumb_core::model::{Recipe, Section};
use crumb_core::{model, source};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/format.json")).expect("valid fixture JSON")
}

fn entries<'a>(root: &'a Value, name: &str) -> &'a Vec<Value> {
    root.get(name)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing fixture array {name}"))
}

fn sections(value: &Value) -> Vec<Section> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .map(|s| Section {
                    name: s.get("name").and_then(Value::as_str).map(str::to_string),
                    items: s
                        .get("items")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .map(|i| i.as_str().unwrap().to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A `Recipe` with only the fields the formatters read filled in from `v`.
fn recipe_from(v: &Value) -> Recipe {
    let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    Recipe {
        id: 0,
        url: text("url"),
        source: "url".into(),
        title: v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: text("description"),
        image: None,
        author: text("author"),
        prep_time: text("prepTime"),
        cook_time: text("cookTime"),
        total_time: text("totalTime"),
        freeze_time: text("freezeTime"),
        recipe_yield: text("recipeYield"),
        recipe_category: text("recipeCategory"),
        recipe_cuisine: text("recipeCuisine"),
        ingredients: sections(&v["ingredients"]),
        instructions: sections(&v["instructions"]),
        nutrition: None,
        notes: text("notes"),
        original_url: text("originalUrl"),
        created_at: 0,
        updated_at: 0,
    }
}

fn check(name: &str, input: &Value, expected: &Value, actual: &Value) {
    assert_eq!(
        expected, actual,
        "\n{name} mismatch\n  input:    {input}\n  expected: {expected}\n  actual:   {actual}"
    );
}

#[test]
fn is_share_link_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "isShareLink") {
        let actual = Value::Bool(entry["input"].as_str().is_some_and(source::is_share_link));
        check("isShareLink", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn public_source_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "publicSource") {
        let r = recipe_from(&entry["input"]);
        let actual = serde_json::to_value(source::source_url(&r)).unwrap();
        check("publicSource", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn recipe_to_markdown_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "recipeToMarkdown") {
        let r = recipe_from(&entry["input"]["recipe"]);
        let actual = Value::String(recipe_to_markdown(&r));
        check(
            "recipeToMarkdown",
            &entry["input"],
            &entry["output"],
            &actual,
        );
    }
}

#[test]
fn recipe_to_text_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "recipeToText") {
        let r = recipe_from(&entry["input"]["recipe"]);
        let link = entry["input"]["link"].as_str();
        let actual = Value::String(recipe_to_text(&r, link));
        check("recipeToText", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn book_to_text_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "bookToText") {
        let name = entry["input"]["name"].as_str().unwrap();
        let titles: Vec<&str> = entry["input"]["titles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        let link = entry["input"]["link"].as_str();
        let actual = Value::String(book_to_text(name, &titles, link));
        check("bookToText", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn book_imported_title_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "bookImportedTitle") {
        let input = &entry["input"];
        let actual = Value::String(book_imported_title(
            input["name"].as_str().unwrap(),
            input["added"].as_u64().unwrap() as usize,
            input["duplicates"].as_u64().unwrap() as usize,
            input
                .get("skipped")
                .and_then(Value::as_u64)
                .map(|n| n as usize),
        ));
        check("bookImportedTitle", input, &entry["output"], &actual);
    }
}

#[test]
fn count_items_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "countItems") {
        let actual = serde_json::to_value(model::count_items(&sections(&entry["input"]))).unwrap();
        check("countItems", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn kicker_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "kicker") {
        let input = &entry["input"];
        let actual = Value::String(kicker(
            input["recipeCategory"].as_str(),
            input["recipeCuisine"].as_str(),
        ));
        check("kicker", input, &entry["output"], &actual);
    }
}

#[test]
fn web_link_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "webLink") {
        let actual = serde_json::to_value(web_link(entry["input"].as_str())).unwrap();
        check("webLink", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn host_of_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "hostOf") {
        let actual = serde_json::to_value(host_of(entry["input"].as_str())).unwrap();
        check("hostOf", &entry["input"], &entry["output"], &actual);
    }
}
