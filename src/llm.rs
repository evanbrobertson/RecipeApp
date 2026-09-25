//! Optional AI calls with structured JSON output: parsing messy pasted text into recipe
//! fields, and writing "Try next" blurbs. Works with Anthropic (Claude), OpenAI, or
//! DeepSeek, whichever is configured (see `config::LlmConfig`).

use serde_json::{Value, json};
use std::time::Duration;

use crate::AppState;
use crate::config::{LlmConfig, LlmProvider};
use crate::model::{RecipeFields, normalize_sections, parse_sections};

const SYSTEM: &str = "You extract recipes from messy pasted text (web pages, emails, notes, OCR).
Return the recipe exactly as written: keep every ingredient and every step, with quantities, temperatures and times unchanged.
Drop ads, life stories, comments, navigation and other non-recipe content.
Group ingredients and steps into named sections only when the source does; otherwise use a single section with name null.
Each instruction item is one step. Do not invent ingredients, steps or metadata that are not in the text.";

/// DeepSeek caps output tokens for its chat model.
const DEEPSEEK_MAX_TOKENS: u32 = 8192;

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

/// The effort setting is rejected by older Claude models.
fn supports_effort(model: &str) -> bool {
    const OLDER: [&str; 7] = [
        "claude-3",
        "claude-haiku",
        "claude-sonnet-4-0",
        "claude-sonnet-4-2",
        "claude-sonnet-4-5",
        "claude-opus-4-0",
        "claude-opus-4-1",
    ];
    !OLDER.iter().any(|p| model.starts_with(p))
}

/// OpenAI reasoning models take `reasoning_effort`; older chat models reject it.
fn openai_reasoning(model: &str) -> bool {
    model.starts_with("gpt-5") || model.starts_with('o')
}

pub fn available(state: &AppState) -> bool {
    state.config.llm.is_some()
}

/// "Claude", "OpenAI" or "DeepSeek" for the settings page; None without a key.
pub fn provider_label(state: &AppState) -> Option<&'static str> {
    state.config.llm.as_ref().map(|l| l.provider.label())
}

/// One structured-output request.
pub struct Ask<'a> {
    /// Short tag for log lines, e.g. "extract".
    pub tag: &'a str,
    pub model: &'a str,
    pub system: &'a str,
    pub user: &'a str,
    /// JSON Schema of the reply (strict: every property required, no extra properties).
    pub schema: Value,
    pub max_tokens: u32,
    pub timeout: Duration,
}

/// The HTTP request for `ask` on this provider.
fn build(state: &AppState, llm: &LlmConfig, ask: &Ask) -> reqwest::RequestBuilder {
    match llm.provider {
        LlmProvider::Anthropic => {
            let mut body = json!({
                "model": ask.model,
                "max_tokens": ask.max_tokens,
                "system": ask.system,
                "messages": [{"role": "user", "content": ask.user}],
                "output_config": {"format": {"type": "json_schema", "schema": ask.schema}},
            });
            if supports_effort(ask.model) {
                body["output_config"]["effort"] = json!("low");
            }
            let fallbacks = supports_fallbacks(ask.model);
            if fallbacks {
                body["fallbacks"] = json!("default");
            }
            let mut req = state
                .http
                .post(format!("{}/v1/messages", llm.base_url))
                .header("x-api-key", &llm.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body);
            if fallbacks {
                req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
            }
            req
        }
        LlmProvider::OpenAi => {
            let mut body = json!({
                "model": ask.model,
                "messages": [
                    {"role": "system", "content": ask.system},
                    {"role": "user", "content": ask.user}
                ],
                "max_completion_tokens": ask.max_tokens,
                "response_format": {"type": "json_schema", "json_schema": {
                    "name": "reply", "strict": true, "schema": ask.schema
                }},
            });
            if openai_reasoning(ask.model) {
                body["reasoning_effort"] = json!("low");
            }
            state
                .http
                .post(format!("{}/chat/completions", llm.base_url))
                .bearer_auth(&llm.api_key)
                .json(&body)
        }
        LlmProvider::DeepSeek => {
            // JSON mode without schema enforcement: describe the schema in the prompt
            let system = format!(
                "{}\n\nReply with only a JSON object that matches this JSON Schema:\n{}",
                ask.system, ask.schema
            );
            let body = json!({
                "model": ask.model,
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": ask.user}
                ],
                "max_tokens": ask.max_tokens.min(DEEPSEEK_MAX_TOKENS),
                "response_format": {"type": "json_object"},
            });
            state
                .http
                .post(format!("{}/chat/completions", llm.base_url))
                .bearer_auth(&llm.api_key)
                .json(&body)
        }
    }
}

/// The JSON reply from a provider's response body, or None if it was refused or cut off.
fn reply_json(provider: LlmProvider, msg: &Value) -> Option<Value> {
    let text = match provider {
        LlmProvider::Anthropic => {
            let stop = msg.get("stop_reason").and_then(Value::as_str);
            if matches!(stop, Some("refusal" | "max_tokens")) {
                return None;
            }
            msg.get("content")?
                .as_array()?
                .iter()
                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))?
                .get("text")?
                .as_str()?
        }
        LlmProvider::OpenAi | LlmProvider::DeepSeek => {
            let choice = msg.get("choices")?.as_array()?.first()?;
            let finish = choice.get("finish_reason").and_then(Value::as_str);
            if matches!(finish, Some("length" | "content_filter")) {
                return None;
            }
            let message = choice.get("message")?;
            if message.get("refusal").is_some_and(|r| !r.is_null()) {
                return None;
            }
            message.get("content")?.as_str()?
        }
    };
    serde_json::from_str(text.trim()).ok()
}

/// Sends one structured-output request, retrying once on rate limits and server errors.
/// None when no provider is configured or the call fails; the failure is logged.
pub async fn ask(state: &AppState, ask: Ask<'_>) -> Option<Value> {
    let llm = state.config.llm.as_ref()?;
    let mut last_error = String::new();
    for attempt in 0..2 {
        match build(state, llm, &ask).timeout(ask.timeout).send().await {
            Ok(res) if res.status().is_success() => {
                let msg: Value = res.json().await.ok()?;
                let out = reply_json(llm.provider, &msg);
                if out.is_none() {
                    tracing::warn!("[llm-{}] no usable reply (refused or cut off)", ask.tag);
                }
                return out;
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
    tracing::warn!(
        "[llm-{}] {} failed: {last_error}",
        ask.tag,
        llm.provider.label()
    );
    None
}

/// Turns pasted text into structured recipe fields. Returns None when no AI is
/// configured, the call fails, or there's no recipe, so callers can fall back to the
/// built-in parser.
pub async fn extract_recipe(state: &AppState, text: &str) -> Option<RecipeFields> {
    let model = state.config.llm.as_ref()?.model.clone();
    let out = ask(
        state,
        Ask {
            tag: "extract",
            model: &model,
            system: SYSTEM,
            user: text,
            schema: schema(),
            max_tokens: 16000,
            timeout: Duration::from_secs(60),
        },
    )
    .await?;
    fields_from_reply(&out)
}

fn fields_from_reply(out: &Value) -> Option<RecipeFields> {
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

    fn toast() -> Value {
        json!({"isRecipe": true, "title": " Toast ", "description": null, "author": null,
            "prepTime": "2m", "cookTime": null, "totalTime": null, "recipeYield": null,
            "recipeCategory": null, "recipeCuisine": null,
            "ingredients": [{"name": null, "items": ["bread", " "]}],
            "instructions": [{"name": null, "items": ["Toast it."]}], "notes": null})
    }

    #[test]
    fn parses_anthropic_structured_output() {
        let msg = json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": toast().to_string()}]});
        let r = fields_from_reply(&reply_json(LlmProvider::Anthropic, &msg).unwrap()).unwrap();
        assert_eq!(r.title, "Toast");
        assert_eq!(r.ingredients[0].items, vec!["bread"]);
        let refused = json!({"stop_reason": "refusal", "content": []});
        assert!(reply_json(LlmProvider::Anthropic, &refused).is_none());
        assert!(schema()["required"].as_array().unwrap().len() == 13);
    }

    #[test]
    fn parses_openai_style_output() {
        let msg = json!({"choices": [{"finish_reason": "stop",
            "message": {"role": "assistant", "content": toast().to_string(), "refusal": null}}]});
        for provider in [LlmProvider::OpenAi, LlmProvider::DeepSeek] {
            let out = reply_json(provider, &msg).unwrap();
            assert_eq!(fields_from_reply(&out).unwrap().title, "Toast");
        }
        let cut =
            json!({"choices": [{"finish_reason": "length", "message": {"content": "{\"isRe"}}]});
        assert!(reply_json(LlmProvider::OpenAi, &cut).is_none());
        let refused = json!({"choices": [{"finish_reason": "stop",
            "message": {"content": null, "refusal": "I can't help with that."}}]});
        assert!(reply_json(LlmProvider::OpenAi, &refused).is_none());
    }
}
