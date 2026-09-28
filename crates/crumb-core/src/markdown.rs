use crate::model::Recipe;

/// Plain-text/Markdown rendering used by the Claude connector.
pub fn recipe_to_markdown(r: &Recipe) -> String {
    let meta: Vec<String> = [
        ("Prep", &r.prep_time),
        ("Cook", &r.cook_time),
        ("Additional", &r.freeze_time),
        ("Total", &r.total_time),
        ("Yield", &r.recipe_yield),
        ("Category", &r.recipe_category),
        ("Cuisine", &r.recipe_cuisine),
        ("By", &r.author),
    ]
    .iter()
    .filter_map(|(label, v)| {
        v.as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| format!("{label}: {v}"))
    })
    .collect();

    let mut out = vec![format!("# {}", r.title)];
    if let Some(d) = r.description.as_deref().filter(|d| !d.is_empty()) {
        out.extend(["".into(), d.to_string()]);
    }
    if !meta.is_empty() {
        out.extend(["".into(), meta.join(" · ")]);
    }

    out.extend(["".into(), "## Ingredients".into()]);
    for s in &r.ingredients {
        if let Some(name) = &s.name {
            out.extend(["".into(), format!("### {name}")]);
        }
        out.extend(s.items.iter().map(|i| format!("- {i}")));
    }

    out.extend(["".into(), "## Instructions".into()]);
    let mut step = 0;
    for s in &r.instructions {
        if let Some(name) = &s.name {
            out.extend(["".into(), format!("### {name}")]);
        }
        for item in &s.items {
            step += 1;
            out.push(format!("{step}. {item}"));
        }
    }

    if let Some(n) = r.notes.as_deref().filter(|n| !n.is_empty()) {
        out.extend(["".into(), "## Notes".into(), n.to_string()]);
    }
    // Where it came from, never a share link it was saved from (the file may be handed on)
    if let Some(u) = crate::source::source_url(r) {
        out.extend(["".into(), format!("Source: {u}")]);
    }
    out.join("\n")
}
