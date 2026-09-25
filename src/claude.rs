//! Optional Claude-powered parsing of messy pasted text into recipe fields.

use serde_json::{Value, json};
use std::time::Duration;

use crate::AppState;
use crate::model::{RecipeFields, normalize_sections, parse_sections};

const SYSTEM: &str = "You extract recipes from messy pasted text (web pages, emails, notes, OCR).
Return the recipe exactly as written: keep every ingredient and every step, with quantities, temperatures and times unchanged.
Drop ads, life stories, comments, navigation and other non-recipe content.
Group ingredients and steps into named sections only when the source does; otherwise use a single section with name null.
Each instruction item is one step. Do not invent ingredients, steps or metadata that are not in the text.";

fn nullable(description: Option<&str>) -> Value {
    let mut v = json!({"type": ["string", "null"]});
    if let Some(d) = description {
        v["description"] = json!(d);
    }
    v
}

fn schema() -> Value {
    let section = json!({
        "type": "object",
        "properties": {
            "name": {"type": ["string", "null"], "description": "Sub-heading such as \"For the sauce\", or null"},
            "items": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["name", "items"],
        "additionalProperties": false
    });
    let properties = json!({
        "isRecipe": {"type": "boolean", "description": "False if the text does not contain a recipe"},
        "title": {"type": "string"},
        "description": nullable(None),
        "author": nullable(None),
        "prepTime": nullable(Some("Human readable, e.g. \"15m\" or \"1h 10m\"")),
        "cookTime": nullable(None),
        "totalTime": nullable(None),
        "recipeYield": nullable(Some("e.g. \"4 servings\"")),
        "recipeCategory": nullable(Some("e.g. \"Dessert\", \"Main course\"")),
        "recipeCuisine": nullable(None),
        "ingredients": {"type": "array", "items": section.clone()},
        "instructions": {"type": "array", "items": section},
        "notes": nullable(None)
    });
    let required: Vec<&String> = properties.as_object().unwrap().keys().collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

/// Server-side refusal fallbacks exist for the Opus 5 and Fable families.
fn supports_fallbacks(model: &str) -> bool {
    model.starts_with("claude-opus-5") || model.starts_with("claude-fable")
}

pub fn available(state: &AppState) -> bool {
    state.config.anthropic_api_key.is_some()
}

/// Uses Claude to turn pasted text into structured recipe fields. Returns None when
/// Claude is not configured, fails, or finds no recipe, so callers can fall back to
/// the built-in parser.
pub async fn extract_recipe(state: &AppState, text: &str) -> Option<RecipeFields> {
    let key = state.config.anthropic_api_key.as_deref()?;
    let model = &state.config.anthropic_model;

    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": SYSTEM,
        "messages": [{"role": "user", "content": text}],
        "output_config": {"effort": "low", "format": {"type": "json_schema", "schema": schema()}},
    });
    let fallbacks = supports_fallbacks(model);
    if fallbacks {
        body["fallbacks"] = json!("default");
    }

    let mut last_error = String::new();
    for attempt in 0..2 {
        let mut req = state
            .http
            .post(format!("{}/v1/messages", state.config.anthropic_base_url))
            .timeout(Duration::from_secs(60))
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body);
        if fallbacks {
            req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
        }
        match req.send().await {
            Ok(res) if res.status().is_success() => {
                let msg: Value = res.json().await.ok()?;
                return parse_response(&msg);
            }
            Ok(res) => {
                let status = res.status();
                last_error = format!(
                    "API error {}: {}",
                    status.as_u16(),
                    res.text().await.unwrap_or_default()
                );
                // Retry rate limits, overloads and server errors once
                if !(status.as_u16() == 429 || status.as_u16() == 529 || status.is_server_error()) {
                    break;
                }
            }
            Err(err) => last_error = err.to_string(),
        }
        if attempt == 0 {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
    tracing::warn!("[claude-extract] failed: {last_error}");
    None
}

fn parse_response(msg: &Value) -> Option<RecipeFields> {
    let stop = msg.get("stop_reason").and_then(Value::as_str);
    if matches!(stop, Some("refusal" | "max_tokens")) {
        return None;
    }
    let text = msg
        .get("content")?
        .as_array()?
        .iter()
        .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))?
        .get("text")?
        .as_str()?;
    let out: Value = serde_json::from_str(text).ok()?;
    if out.get("isRecipe").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let s = |k: &str| out.get(k).and_then(Value::as_str).map(String::from);
    let title = s("title")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    Some(RecipeFields {
        title: title.unwrap_or_else(|| "Untitled recipe".into()),
        description: s("description"),
        author: s("author"),
        prep_time: s("prepTime"),
        cook_time: s("cookTime"),
        total_time: s("totalTime"),
        recipe_yield: s("recipeYield"),
        recipe_category: s("recipeCategory"),
        recipe_cuisine: s("recipeCuisine"),
        ingredients: normalize_sections(
            parse_sections("ingredients", out.get("ingredients")?).ok()?,
        ),
        instructions: normalize_sections(
            parse_sections("instructions", out.get("instructions")?).ok()?,
        ),
        notes: s("notes"),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_output() {
        let inner = json!({"isRecipe": true, "title": " Toast ", "description": null, "author": null,
            "prepTime": "2m", "cookTime": null, "totalTime": null, "recipeYield": null,
            "recipeCategory": null, "recipeCuisine": null,
            "ingredients": [{"name": null, "items": ["bread", " "]}],
            "instructions": [{"name": null, "items": ["Toast it."]}], "notes": null});
        let msg = json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": inner.to_string()}]});
        let r = parse_response(&msg).unwrap();
        assert_eq!(r.title, "Toast");
        assert_eq!(r.ingredients[0].items, vec!["bread"]);
        let refused = json!({"stop_reason": "refusal", "content": []});
        assert!(parse_response(&refused).is_none());
        assert!(schema()["required"].as_array().unwrap().len() == 13);
    }
}
