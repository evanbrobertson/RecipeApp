//! Phase 2: changing the box: fields, the Trash, cookbooks, sharing, and the `api` escape hatch.

use crumb_client::ShareKind;
use serde_json::{Map, Value, json};

use super::{BooksAction, Done, Failure, Run, TrashAction, error};

/// The names `set` takes, and the API field each means.
const FIELDS: [(&str, &str); 13] = [
    ("title", "title"),
    ("description", "description"),
    ("notes", "notes"),
    ("author", "author"),
    ("servings", "recipeYield"),
    ("yield", "recipeYield"),
    ("recipeYield", "recipeYield"),
    ("category", "recipeCategory"),
    ("recipeCategory", "recipeCategory"),
    ("cuisine", "recipeCuisine"),
    ("recipeCuisine", "recipeCuisine"),
    ("url", "url"),
    ("video", "video"),
];
const TIMES: [(&str, &str); 6] = [
    ("prepTime", "prepTime"),
    ("cookTime", "cookTime"),
    ("totalTime", "totalTime"),
    ("prep", "prepTime"),
    ("cook", "cookTime"),
    ("time", "totalTime"),
];

fn patch_from(pairs: &[String]) -> Result<Map<String, Value>, Failure> {
    let mut patch = Map::new();
    for pair in pairs {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| Failure::usage(format!("“{pair}” should be field=value")))?;
        let api = FIELDS
            .iter()
            .chain(TIMES.iter())
            .find(|(name, _)| name.eq_ignore_ascii_case(key.trim()))
            .map(|(_, api)| *api)
            .ok_or_else(|| {
                let names: Vec<&str> = FIELDS
                    .iter()
                    .chain(TIMES.iter())
                    .map(|(name, _)| *name)
                    .collect();
                Failure::usage(format!(
                    "Can't set “{key}”. Fields: {}. For ingredients and steps, use `crumb api PATCH`",
                    names.join(", ")
                ))
            })?;
        let value = if value.is_empty() {
            Value::Null
        } else {
            json!(value)
        };
        patch.insert(api.to_string(), value);
    }
    Ok(patch)
}

impl Run<'_> {
    pub(super) async fn set(&mut self, id: i64, pairs: &[String]) -> Done {
        let patch = patch_from(pairs)?;
        let r = self
            .client()?
            .patch_recipe(id, &Value::Object(patch))
            .await?;
        if self.cli.json {
            self.json_out(&serde_json::to_value(&r).unwrap_or_default());
        } else if self.cli.quiet {
            self.line(r.id.to_string());
        } else {
            self.line(format!("Updated #{}: {}", r.id, r.title));
        }
        Ok(())
    }

    pub(super) async fn rm(&mut self, ids: &[i64], yes: bool) -> Done {
        if ids.len() > 1 && !yes {
            return Err(Failure::usage(
                "Deleting more than one recipe needs -y (they go to the Trash, not for good)",
            ));
        }
        let client = self.client()?;
        let deleted = match ids {
            [one] => {
                client.delete_recipe(*one).await?;
                1
            }
            many => client.bulk_delete(many).await?,
        };
        if self.cli.json {
            self.json_out(&json!({"ok": true, "deleted": deleted, "ids": ids}));
        } else if self.cli.quiet {
            for id in ids {
                self.line(id.to_string());
            }
        } else {
            self.line(format!(
                "{deleted} moved to the Trash for 30 days. Put one back with: crumb trash restore <id>"
            ));
        }
        Ok(())
    }

    pub(super) async fn trash(&mut self, action: Option<&TrashAction>) -> Done {
        let client = self.client()?;
        match action {
            None | Some(TrashAction::Ls) => {
                let items = client.trash().await?;
                if self.cli.json {
                    self.json_out(&serde_json::to_value(&items).unwrap_or_default());
                } else if self.cli.quiet {
                    for t in &items {
                        self.line(t.id.to_string());
                    }
                } else {
                    for t in &items {
                        self.line(format!("{}\t{}\tgone {}", t.id, t.title, t.purge_at));
                    }
                }
            }
            Some(TrashAction::Restore { id }) => {
                let (r, _) = client.restore_recipe(*id).await?;
                if self.cli.json {
                    self.json_out(&json!({"id": r.id, "title": r.title}));
                } else if self.cli.quiet {
                    self.line(r.id.to_string());
                } else {
                    self.line(format!("Restored #{}: {}", r.id, r.title));
                }
            }
        }
        Ok(())
    }

    pub(super) async fn books(&mut self, action: Option<&BooksAction>) -> Done {
        let client = self.client()?;
        match action {
            None | Some(BooksAction::Ls) => {
                let books = client.cookbooks().await?;
                if self.cli.json {
                    self.json_out(&serde_json::to_value(&books).unwrap_or_default());
                } else if self.cli.quiet {
                    for b in &books {
                        self.line(b.id.to_string());
                    }
                } else {
                    for b in &books {
                        self.line(format!("{}\t{}\t{}", b.id, b.name, b.recipe_count));
                    }
                }
            }
            Some(BooksAction::Show { id }) => {
                let book = client.cookbook(*id).await?;
                if self.cli.json {
                    self.json_out(&serde_json::to_value(&book).unwrap_or_default());
                } else if self.cli.quiet {
                    for r in &book.recipes {
                        self.line(r.id.to_string());
                    }
                } else {
                    self.line(format!("{}\t{}", book.id, book.name));
                    for r in &book.recipes {
                        self.line(format!("{}\t{}", r.id, r.title));
                    }
                }
            }
            Some(BooksAction::New { name, description }) => {
                let b = client
                    .create_cookbook(name, description.as_deref(), None)
                    .await?;
                self.said(
                    &json!({"id": b.id, "name": b.name}),
                    b.id,
                    format!("Made cookbook #{}: {}", b.id, b.name),
                );
            }
            Some(BooksAction::Rename { id, name }) => {
                let b = client.patch_cookbook(*id, &json!({"name": name})).await?;
                self.said(
                    &json!({"id": b.id, "name": b.name}),
                    b.id,
                    format!("Renamed #{} to {}", b.id, b.name),
                );
            }
            Some(BooksAction::Rm { id, yes }) => {
                if !yes {
                    return Err(Failure::usage(
                        "Deleting a cookbook needs -y (its recipes stay in your box)",
                    ));
                }
                client.delete_cookbook(*id).await?;
                self.said(
                    &json!({"ok": true, "id": id}),
                    *id,
                    format!("Deleted cookbook #{id}"),
                );
            }
            Some(BooksAction::Add { book, ids }) => {
                let added = client.add_to_cookbook(*book, ids).await?;
                self.said(
                    &json!({"ok": true, "added": added}),
                    *book,
                    format!("Added {added} to cookbook #{book}"),
                );
            }
            Some(BooksAction::Remove { book, id }) => {
                client.remove_from_cookbook(*book, *id).await?;
                self.said(
                    &json!({"ok": true}),
                    *book,
                    format!("Took #{id} out of cookbook #{book}"),
                );
            }
        }
        Ok(())
    }

    /// One result line in whichever way output was asked for: JSON, just the id, or words.
    fn said(&mut self, json: &Value, id: i64, words: String) {
        if self.cli.json {
            self.json_out(json);
        } else if self.cli.quiet {
            self.line(id.to_string());
        } else {
            self.line(words);
        }
    }

    pub(super) async fn share(&mut self, id: i64, book: bool, stop: bool) -> Done {
        let client = self.client()?;
        let kind = if book {
            ShareKind::Cookbook
        } else {
            ShareKind::Recipe
        };
        if stop {
            client.stop_share(kind, id).await?;
            self.said(&json!({"ok": true}), id, "The link stopped working".into());
            return Ok(());
        }
        let share = client.create_share(kind, id).await?;
        if self.cli.json {
            self.json_out(&serde_json::to_value(&share).unwrap_or_default());
        } else {
            self.line(&share.url);
        }
        Ok(())
    }

    pub(super) async fn api(&mut self, method: &str, path: &str, data: Option<&str>) -> Done {
        let body = data
            .map(serde_json::from_str::<Value>)
            .transpose()
            .map_err(|e| Failure::usage(format!("--data isn't JSON: {e}")))?;
        let (status, text) = self.client()?.raw(method, path, body.as_ref()).await?;
        self.out.push_str(&text);
        if !text.ends_with('\n') {
            self.out.push('\n');
        }
        match status {
            200..=299 => Ok(()),
            401 | 403 => Err(Failure::plain(
                error::NOT_SIGNED_IN,
                format!("{status}"),
                None,
            )),
            404 => Err(Failure::plain(error::NOT_FOUND, "404", None)),
            429 => Err(Failure::plain(error::RATE_LIMITED, "429", None)),
            _ => Err(Failure::plain(1, format!("{status}"), None)),
        }
    }
}
