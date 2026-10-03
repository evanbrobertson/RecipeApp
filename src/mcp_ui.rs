//! The ChatGPT plugin's in-chat UI: one HTML bundle (`web/widget`, built to
//! `shell/chatgpt/shelf.html`) served as an MCP Apps resource, and the metadata that ties
//! the cookbook, recipe and save tools to it.
//!
//! The model reads a tool's `structuredContent` verbatim, so it holds only what the widget
//! draws; the photo links, which are signed and noisy, go in the result's `_meta`, which only
//! the widget sees.

use serde_json::{Map, Value, json};

use crate::AppState;
use crate::model::Recipe;

pub const SHELF_URI: &str = "ui://crumb/shelf-v1.html";
const MIME: &str = "text/html;profile=mcp-app";
const TEMPLATE: &str = "shell/chatgpt/shelf.html";
const ORIGIN_MARK: &str = "__CRUMB_ORIGIN__";

/// Tools whose result the widget draws.
const UI_TOOLS: [&str; 4] = [
    "list_cookbooks",
    "get_cookbook",
    "get_recipe",
    "save_recipe",
];

/// Photo width for a card: the widget shows them small.
const CARD_PHOTO_WIDTH: u32 = 400;

pub fn is_ui_tool(name: &str) -> bool {
    UI_TOOLS.contains(&name)
}

/// What `tools/list` adds to a tool: the resource it renders in, the status lines ChatGPT shows
/// while it runs, and, for the shelf, a place in ChatGPT's sidebar.
pub fn tool_meta(name: &str) -> Option<Value> {
    let (invoking, invoked) = match name {
        "list_cookbooks" => ("Opening your shelf…", "Shelf ready"),
        "get_cookbook" => ("Opening the cookbook…", "Cookbook ready"),
        "get_recipe" => ("Finding the recipe…", "Recipe ready"),
        "save_recipe" => ("Saving to Crumb…", "Saved to Crumb"),
        _ => return None,
    };
    let mut meta = json!({
        "ui": {"resourceUri": SHELF_URI},
        "openai/outputTemplate": SHELF_URI,
        "openai/toolInvocation/invoking": invoking,
        "openai/toolInvocation/invoked": invoked,
    });
    if name == "list_cookbooks" {
        meta["openai/ui"] = json!({"entrypoints": [{"type": "global"}]});
    }
    Some(meta)
}

/// Every UI tool says what shape its `structuredContent` has: a `view` the widget switches on.
pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"view": {"type": "string", "enum": ["shelf", "book", "recipe", "saved"]}},
        "required": ["view"],
    })
}

pub fn resource_list() -> Value {
    json!({"resources": [{
        "uri": SHELF_URI,
        "name": "Crumb shelf",
        "title": "Crumb shelf",
        "description": "The user's cookbooks and recipes",
        "mimeType": MIME,
    }]})
}

/// The widget's HTML, with the server's own address filled in (its fonts and photos load from
/// there, so the page's CSP must name it).
pub fn read_resource(state: &AppState, origin: &str) -> Option<Value> {
    let html = state.web.html(TEMPLATE)?.replace(ORIGIN_MARK, origin);
    Some(json!({"contents": [{
        "uri": SHELF_URI,
        "mimeType": MIME,
        "text": html,
        "_meta": {
            "ui": {
                "prefersBorder": false,
                "csp": {"resourceDomains": [origin], "connectDomains": []},
            },
            "openai/widgetDescription":
                "The user's Crumb shelf: cookbooks as books, with the recipes inside, or one recipe's card.",
            "openai/ui": {"availableDisplayModes": ["inline", "fullscreen"]},
        },
    }]}))
}

/// A tool result the widget draws: the text the model reads, the `structuredContent` both
/// read, and hidden `_meta` for the widget alone.
pub fn with_ui(mut result: Value, structured: Value, meta: Value) -> Value {
    result["structuredContent"] = structured;
    result["_meta"] = meta;
    result
}

/// A recipe as the widget's card draws it.
pub fn card(origin: &str, r: &Recipe) -> Value {
    json!({
        "id": r.id,
        "title": r.title,
        "link": format!("{origin}/recipes/{}", r.id),
        "totalTime": r.total_time,
        "category": r.recipe_category,
        "description": r.description,
        "ingredients": crate::model::count_items(&r.ingredients),
        "steps": crate::model::count_items(&r.instructions),
    })
}

pub fn summary_card(origin: &str, r: &crate::model::RecipeSummary) -> Value {
    json!({
        "id": r.id,
        "title": r.title,
        "link": format!("{origin}/recipes/{}", r.id),
        "totalTime": r.total_time,
        "category": r.recipe_category,
    })
}

/// Signed photo links for the recipes that have a photo, by recipe id.
pub fn photos<'a>(
    state: &AppState,
    origin: &str,
    recipes: impl IntoIterator<Item = (i64, Option<&'a str>)>,
) -> Value {
    let mut out = Map::new();
    for (id, image) in recipes {
        if let Some(image) = image.filter(|i| !i.is_empty()) {
            out.insert(
                id.to_string(),
                json!(crate::images::signed_url(
                    state,
                    origin,
                    id,
                    CARD_PHOTO_WIDTH,
                    image
                )),
            );
        }
    }
    json!({"photos": out, "origin": origin})
}
