//! Parity tests: every case in `fixtures/app.json` (generated from the real `web/src/lib`
//! by `fixtures/gen-app.ts`) must produce the same output from the Rust port.

use crumb_core::add::job_progress;
use crumb_core::books::{
    ShelfBook, book_color, book_lean, book_look, book_size, seeded, spine_band, stack_books,
};
use crumb_core::prep::ingredient_color;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/app.json")).expect("valid fixture JSON")
}

fn cases<'a>(root: &'a Value, name: &str) -> impl Iterator<Item = (&'a Value, &'a Value)> {
    root[name]
        .as_array()
        .unwrap_or_else(|| panic!("missing fixture array {name}"))
        .iter()
        .map(|e| (&e["input"], &e["output"]))
}

fn book(v: &Value) -> ShelfBook {
    serde_json::from_value(v.clone()).expect("a shelf book")
}

// `Math.sin` and Rust's may differ in the last bit
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn colours() {
    let root = fixture();
    for (input, output) in cases(&root, "bookColor") {
        assert_eq!(book_color(input.as_str()), output, "bookColor({input})");
    }
    for (input, output) in cases(&root, "bookPalette") {
        let look = book_look(input.as_str());
        assert_eq!(look.cloth, output["cloth"], "cloth for {input}");
        assert_eq!(look.shade, output["shade"], "shade for {input}");
        assert_eq!(look.foil, output["foil"], "foil for {input}");
        assert_eq!(look.bands[0], output["bands"][0], "bands for {input}");
        assert_eq!(look.bands[1], output["bands"][1], "bands for {input}");
        assert_eq!(
            look.edge.is_some(),
            output["border"] != "none",
            "edge for {input}"
        );
    }
}

#[test]
fn geometry() {
    let root = fixture();
    for (input, output) in cases(&root, "seeded") {
        let got = seeded(
            input["id"].as_i64().unwrap(),
            input["salt"].as_i64().unwrap(),
        );
        assert!(
            close(got, output.as_f64().unwrap()),
            "seeded({input}) = {got}"
        );
    }
    for (input, output) in cases(&root, "bookSize") {
        let size = book_size(&book(input));
        assert_eq!(
            u64::from(size.thickness),
            output["thickness"],
            "thickness {input}"
        );
        assert_eq!(u64::from(size.title), output["title"], "title {input}");
        assert!(
            close(size.length, output["length"].as_f64().unwrap()),
            "length {input}"
        );
    }
    for (input, output) in cases(&root, "spineBands") {
        assert_eq!(
            spine_band(&book(input)),
            output.as_str(),
            "spineBands({input})"
        );
    }
    for (input, output) in cases(&root, "bookLean") {
        let lean = book_lean(&book(&input["book"]), input["atFoot"].as_bool().unwrap());
        assert!(
            close(lean.tilt, output["tilt"].as_f64().unwrap()),
            "tilt {input}"
        );
        assert_eq!(i64::from(lean.nudge), output["nudge"], "nudge {input}");
    }
    for (input, output) in cases(&root, "stackBooks") {
        let books: Vec<ShelfBook> = input["books"]
            .as_array()
            .unwrap()
            .iter()
            .map(book)
            .collect();
        let towers = input["towers"].as_u64().unwrap() as usize;
        let want: Vec<Vec<usize>> = serde_json::from_value(output.clone()).unwrap();
        assert_eq!(
            stack_books(&books, towers),
            want,
            "stackBooks({towers} towers, {} books)",
            books.len()
        );
    }
}

#[test]
fn prep_and_add() {
    let root = fixture();
    for (input, output) in cases(&root, "ingredientColor") {
        assert_eq!(
            ingredient_color(input.as_str().unwrap()),
            output,
            "ingredientColor({input})"
        );
    }
    for (input, output) in cases(&root, "jobProgress") {
        let position = input["position"].as_u64().map(|p| p as u32);
        assert_eq!(
            job_progress(input["status"].as_str().unwrap(), position),
            output.as_str().unwrap(),
            "jobProgress({input})"
        );
    }
}
