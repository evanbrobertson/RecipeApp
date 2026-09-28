//! Parity tests: every case in `fixtures/ingredients.json` (generated from the
//! real `web/src/lib/ingredients.ts` by `fixtures/gen-ingredients.ts`) must
//! produce the same output from the Rust port, numbers compared with 1e-9.

use crumb_core::ingredients::{
    IngredientLine, find_timers, format_quantity, ingredients_for_step_in, mise_en_place,
    parse_ingredient, parse_number, plural_unit, scale_ingredient,
};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/ingredients.json")).expect("valid fixture JSON")
}

fn entries<'a>(root: &'a Value, name: &str) -> &'a Vec<Value> {
    root.get(name)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing fixture array {name}"))
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("expected a number, got {value}"))
}

/// Deep equality with a 1e-9 tolerance on numbers, so `1` and `1.0` agree.
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-9,
            _ => false,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| equal(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| equal(v, w)))
        }
        _ => a == b,
    }
}

fn check(name: &str, input: &Value, expected: &Value, actual: &Value) {
    assert!(
        equal(expected, actual),
        "\n{name} mismatch\n  input:    {input}\n  expected: {expected}\n  actual:   {actual}"
    );
}

#[test]
fn parse_number_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "parseNumber") {
        let input = entry["input"].as_str().unwrap();
        let actual = serde_json::to_value(parse_number(input)).unwrap();
        check("parseNumber", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn parse_ingredient_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "parseIngredient") {
        let input = entry["input"].as_str().unwrap();
        let actual = serde_json::to_value(parse_ingredient(input)).unwrap();
        check(
            "parseIngredient",
            &entry["input"],
            &entry["output"],
            &actual,
        );
    }
}

#[test]
fn format_quantity_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "formatQuantity") {
        let actual = Value::String(format_quantity(number(&entry["input"])));
        check("formatQuantity", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn scale_ingredient_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "scaleIngredient") {
        let raw = entry["input"]["raw"].as_str().unwrap();
        let factor = number(&entry["input"]["factor"]);
        let actual = Value::String(scale_ingredient(raw, factor));
        check(
            "scaleIngredient",
            &entry["input"],
            &entry["output"],
            &actual,
        );
    }
}

#[test]
fn plural_unit_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "pluralUnit") {
        let unit = entry["input"]["unit"].as_str().unwrap();
        let quantity = number(&entry["input"]["quantity"]);
        let actual = Value::String(plural_unit(unit, quantity));
        check("pluralUnit", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn mise_en_place_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "miseEnPlace") {
        let lines: Vec<&str> = entry["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let actual = serde_json::to_value(mise_en_place(&lines)).unwrap();
        check("miseEnPlace", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn find_timers_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "findTimers") {
        let input = entry["input"].as_str().unwrap();
        let actual = serde_json::to_value(find_timers(input)).unwrap();
        check("findTimers", &entry["input"], &entry["output"], &actual);
    }
}

#[test]
fn ingredients_for_step_matches_typescript() {
    let root = fixture();
    for entry in entries(&root, "ingredientsForStep") {
        let step = &entry["input"]["step"];
        let text = step["text"].as_str().unwrap();
        let section = step["section"].as_str();
        let ingredients: Vec<IngredientLine> = entry["input"]["ingredients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| IngredientLine {
                raw: v["raw"].as_str().unwrap().to_string(),
                section: v["section"].as_str().map(str::to_string),
            })
            .collect();
        let actual =
            serde_json::to_value(ingredients_for_step_in(text, section, &ingredients)).unwrap();
        check(
            "ingredientsForStep",
            &entry["input"],
            &entry["output"],
            &actual,
        );
    }
}
