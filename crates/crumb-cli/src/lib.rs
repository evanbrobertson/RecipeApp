//! The `crumb` command line: a thin client over `crumb-client`, with `crumb-core` for the logic
//! every client shares (scaling, the text and Markdown a recipe is shown as). Made first for agents
//! and scripts: plain lines, `--json`, stable exit codes, and nothing that prompts or needs a TTY.
//! Every link goes to the server; this never fetches a recipe site itself.

mod config;
mod error;
mod organise;
pub mod skill;

use std::io::Read;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use crumb_client::{Client, ImportInput, Preview, RecipeFormat};
use crumb_core::ingredients::scale_ingredient;
use crumb_core::model::Recipe;
use serde_json::{Value, json};

pub use error::Failure;

#[derive(Parser)]
#[command(
    name = "crumb",
    version,
    about = "Your Crumb recipe box, from a terminal"
)]
pub struct Cli {
    /// The Crumb server's address
    #[arg(long, global = true, env = "CRUMB_SERVER")]
    server: Option<String>,
    /// An API token, made on the account page (overrides the saved one)
    #[arg(long, global = true, env = "CRUMB_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// Which saved server and token to use
    #[arg(long, global = true, default_value = "default", env = "CRUMB_PROFILE")]
    profile: String,
    /// Where credentials are kept (default: ~/.config/crumb)
    #[arg(long, global = true, env = "CRUMB_CONFIG_DIR", hide = true)]
    config_dir: Option<PathBuf>,
    /// Print machine-readable JSON on stdout
    #[arg(long, global = true)]
    json: bool,
    /// Print only ids
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Save a server and token, after checking they work
    Login {
        /// Read the token from stdin instead of --token
        #[arg(long)]
        token_stdin: bool,
    },
    /// Forget the saved token
    Logout,
    /// Show the server and whether the token works
    Whoami,
    /// List recipes, newest first, or search them
    Ls {
        /// Words to search for
        query: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Show a recipe, by id or by a title that matches only one
    Show {
        recipe: String,
        /// Multiply the ingredients (2, 0.5, 1.5)
        #[arg(long)]
        scale: Option<f64>,
        #[arg(long, value_enum, default_value_t = Shown::Text)]
        format: Shown,
    },
    /// Save a recipe from a link, from text on stdin (-), or from a file
    Add {
        /// A link, `-` for stdin, or a file of recipe text
        source: String,
        /// Read it and say what would be saved, without saving
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Log that you cooked a recipe
    Cooked {
        id: i64,
        /// Take back the latest cook
        #[arg(long)]
        undo: bool,
    },
    /// Write a recipe as Markdown or Crumb's backup JSON
    Export {
        id: i64,
        #[arg(long, value_enum, default_value_t = Exported::Md)]
        format: Exported,
        /// A file to write instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Pick a recipe for me
    Random,
    /// What to cook next, with why
    Next {
        #[arg(long, default_value_t = 5)]
        limit: u32,
    },
    /// Change fields on a recipe: crumb set 12 title="New" servings=4 notes=""
    Set {
        id: i64,
        /// field=value pairs; an empty value clears the field
        #[arg(required = true)]
        fields: Vec<String>,
    },
    /// Move recipes to the Trash (restorable for 30 days)
    Rm {
        #[arg(required = true)]
        ids: Vec<i64>,
        /// Needed to delete more than one at once
        #[arg(short, long)]
        yes: bool,
    },
    /// The Trash: list it, or restore a recipe. Emptying is only in the browser
    Trash {
        #[command(subcommand)]
        action: Option<TrashAction>,
    },
    /// Cookbooks
    Books {
        #[command(subcommand)]
        action: Option<BooksAction>,
    },
    /// A public link to a recipe (or a cookbook with --book); prints just the URL
    Share {
        id: i64,
        /// The id is a cookbook's
        #[arg(long)]
        book: bool,
        /// Stop sharing it
        #[arg(long)]
        stop: bool,
    },
    /// Call any /api route: crumb api GET /api/health
    Api {
        method: String,
        path: String,
        /// A JSON body
        #[arg(long)]
        data: Option<String>,
    },
    /// Check the server and the token
    Doctor,
    /// Install this CLI's agent skill (for Claude Code and other agents), or print it
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Print a shell completion script
    Completions { shell: clap_complete::Shell },
}

#[derive(Subcommand)]
enum SkillAction {
    /// Write ~/.agents/skills/crumb/SKILL.md and link it into ~/.claude/skills
    Install,
    /// Print the skill to stdout
    Print,
}

#[derive(Subcommand)]
enum TrashAction {
    /// What's in the Trash
    Ls,
    /// Put a recipe back
    Restore { id: i64 },
}

#[derive(Subcommand)]
enum BooksAction {
    /// The cookbooks
    Ls,
    /// A cookbook and its recipes
    Show { id: i64 },
    /// Make a cookbook
    New {
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Rename a cookbook
    Rename { id: i64, name: String },
    /// Delete a cookbook (its recipes stay in the box)
    Rm {
        id: i64,
        #[arg(short, long)]
        yes: bool,
    },
    /// Put recipes in a cookbook
    Add {
        book: i64,
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Take a recipe out of a cookbook
    Remove { book: i64, id: i64 },
}

#[derive(Clone, Copy, ValueEnum)]
enum Shown {
    Text,
    Markdown,
    Json,
}

#[derive(Clone, Copy, ValueEnum)]
enum Exported {
    Md,
    Json,
}

/// What a run printed and how it ended.
#[derive(Debug, Default)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

struct Run<'a> {
    cli: &'a Cli,
    stdin: &'a mut dyn Read,
    out: String,
    err: String,
}

pub async fn run(cli: Cli, stdin: &mut dyn Read) -> Outcome {
    let mut run = Run {
        cli: &cli,
        stdin,
        out: String::new(),
        err: String::new(),
    };
    let code = match run.dispatch().await {
        Ok(()) => 0,
        Err(failure) => {
            if cli.json {
                run.err.push_str(&format!("{}\n", failure.json()));
            } else {
                run.err.push_str(&format!("crumb: {}\n", failure.message));
                if let Some(hint) = &failure.hint {
                    run.err.push_str(&format!("{hint}\n"));
                }
            }
            failure.code
        }
    };
    Outcome {
        stdout: run.out,
        stderr: run.err,
        code,
    }
}

type Done = Result<(), Failure>;

impl Run<'_> {
    fn line(&mut self, text: impl AsRef<str>) {
        self.out.push_str(text.as_ref());
        self.out.push('\n');
    }

    fn note(&mut self, text: impl AsRef<str>) {
        if !self.cli.quiet && !self.cli.json {
            self.err.push_str(text.as_ref());
            self.err.push('\n');
        }
    }

    fn json_out(&mut self, value: &Value) {
        self.line(serde_json::to_string_pretty(value).unwrap_or_default());
    }

    fn config_dir(&self) -> Result<PathBuf, Failure> {
        config::dir(self.cli.config_dir.as_deref())
            .ok_or_else(|| Failure::usage("No home directory to keep credentials in"))
    }

    fn saved(&self) -> config::Profile {
        self.config_dir()
            .ok()
            .and_then(|dir| config::load(&dir).profiles.get(&self.cli.profile).cloned())
            .unwrap_or_default()
    }

    fn server(&self) -> Result<String, Failure> {
        self.cli
            .server
            .clone()
            .or_else(|| self.saved().server)
            .ok_or_else(|| {
                Failure::plain(
                    error::USAGE,
                    "No Crumb server set",
                    Some("Run: crumb login --server https://your-crumb.example --token <token>"),
                )
            })
    }

    fn client(&self) -> Result<Client, Failure> {
        let server = self.server()?;
        let token = self
            .cli
            .token
            .clone()
            .or_else(|| self.saved().token)
            .ok_or_else(|| {
                Failure::plain(
                    error::NOT_SIGNED_IN,
                    "No token",
                    Some("Make one on the Crumb account page, then: crumb login --token <token>"),
                )
            })?;
        Ok(Client::with_token(&server, &token)?)
    }

    async fn dispatch(&mut self) -> Done {
        let cli = self.cli;
        match &cli.command {
            Command::Login { token_stdin } => self.login(*token_stdin).await,
            Command::Logout => self.logout(),
            Command::Whoami => self.whoami().await,
            Command::Ls { query, limit } => self.ls(query.as_deref(), *limit).await,
            Command::Show {
                recipe,
                scale,
                format,
            } => self.show(recipe, *scale, *format).await,
            Command::Add { source, dry_run } => self.add(source, *dry_run).await,
            Command::Cooked { id, undo } => self.cooked(*id, *undo).await,
            Command::Export { id, format, output } => {
                self.export(*id, *format, output.as_deref()).await
            }
            Command::Random => self.random().await,
            Command::Next { limit } => self.next(*limit).await,
            Command::Set { id, fields } => self.set(*id, fields).await,
            Command::Rm { ids, yes } => self.rm(ids, *yes).await,
            Command::Trash { action } => self.trash(action.as_ref()).await,
            Command::Books { action } => self.books(action.as_ref()).await,
            Command::Share { id, book, stop } => self.share(*id, *book, *stop).await,
            Command::Api { method, path, data } => self.api(method, path, data.as_deref()).await,
            Command::Doctor => self.doctor().await,
            Command::Skill { action } => match action {
                SkillAction::Print => {
                    self.out.push_str(skill::SKILL);
                    Ok(())
                }
                SkillAction::Install => {
                    let home = std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .ok_or_else(|| Failure::usage("No home directory to install into"))?;
                    for line in skill::install(&home).map_err(Failure::usage)? {
                        self.line(line);
                    }
                    Ok(())
                }
            },
            Command::Completions { shell } => {
                let mut bytes = Vec::new();
                clap_complete::generate(*shell, &mut Cli::command(), "crumb", &mut bytes);
                self.out.push_str(&String::from_utf8_lossy(&bytes));
                Ok(())
            }
        }
    }

    async fn login(&mut self, token_stdin: bool) -> Done {
        let server = self.server()?;
        let token = if token_stdin {
            let mut text = String::new();
            self.stdin
                .read_to_string(&mut text)
                .map_err(|e| Failure::usage(format!("Couldn't read the token: {e}")))?;
            text.trim().to_string()
        } else {
            self.cli.token.clone().unwrap_or_default()
        };
        if token.is_empty() {
            return Err(Failure::plain(
                error::USAGE,
                "Give the token with --token, CRUMB_TOKEN or --token-stdin",
                Some("Make one on the Crumb account page, under API tokens"),
            ));
        }
        let client = Client::with_token(&server, &token)?;
        client.recipes(None, Some(1)).await?;
        let dir = self.config_dir()?;
        let mut saved = config::load(&dir);
        saved.profiles.insert(
            self.cli.profile.clone(),
            config::Profile {
                server: Some(client.base_url().to_string()),
                token: Some(token),
            },
        );
        config::save(&dir, &saved)
            .map_err(|e| Failure::usage(format!("Couldn't save credentials: {e}")))?;
        if self.cli.json {
            self.json_out(&json!({"ok": true, "server": client.base_url()}));
        } else {
            self.line(format!("Signed in to {}", client.base_url()));
        }
        Ok(())
    }

    fn logout(&mut self) -> Done {
        let dir = self.config_dir()?;
        let mut saved = config::load(&dir);
        let had = saved.profiles.remove(&self.cli.profile).is_some();
        config::save(&dir, &saved)
            .map_err(|e| Failure::usage(format!("Couldn't update credentials: {e}")))?;
        if self.cli.json {
            self.json_out(&json!({"ok": true, "removed": had}));
        } else {
            self.line(if had {
                "Forgot the saved token. Revoke it on the account page to stop it working"
            } else {
                "No saved token"
            });
        }
        Ok(())
    }

    async fn whoami(&mut self) -> Done {
        let client = self.client()?;
        client.recipes(None, Some(1)).await?;
        if self.cli.json {
            self.json_out(&json!({"server": client.base_url(), "signedIn": true}));
        } else {
            self.line(format!("{} (token works)", client.base_url()));
        }
        Ok(())
    }

    async fn doctor(&mut self) -> Done {
        let server = self.server()?;
        let anon = Client::new(&server)?;
        anon.health().await?;
        self.note(format!("server: {server} is up"));
        let client = self.client()?;
        client.recipes(None, Some(1)).await?;
        self.note("token: works");
        if self.cli.json {
            self.json_out(&json!({"server": server, "reachable": true, "tokenWorks": true}));
        } else if self.cli.quiet {
            self.line("ok");
        }
        Ok(())
    }

    async fn ls(&mut self, query: Option<&str>, limit: u32) -> Done {
        let found = self.client()?.recipes(query, Some(limit)).await?;
        if self.cli.json {
            self.json_out(&serde_json::to_value(&found).unwrap_or_default());
        } else if self.cli.quiet {
            for r in &found {
                self.line(r.id.to_string());
            }
        } else {
            for r in &found {
                self.line(format!(
                    "{}\t{}\t{}\t{}",
                    r.id,
                    r.title,
                    r.total_time.as_deref().unwrap_or(""),
                    r.recipe_category.as_deref().unwrap_or("")
                ));
            }
        }
        Ok(())
    }

    /// A numeric id, or a title fragment that matches exactly one recipe.
    async fn resolve(&mut self, client: &Client, what: &str) -> Result<i64, Failure> {
        if let Ok(id) = what.trim_start_matches('#').parse::<i64>() {
            return Ok(id);
        }
        let found = client.recipes(Some(what), Some(20)).await?;
        let exact: Vec<_> = found
            .iter()
            .filter(|r| r.title.eq_ignore_ascii_case(what))
            .collect();
        match (found.len(), exact.len()) {
            (0, _) => Err(Failure::not_found(format!("No recipe matches “{what}”"))),
            (1, _) => Ok(found[0].id),
            (_, 1) => Ok(exact[0].id),
            _ => {
                let mut message = format!("“{what}” matches more than one recipe:");
                for r in &found {
                    message.push_str(&format!("\n{}\t{}", r.id, r.title));
                }
                Err(Failure::usage(message))
            }
        }
    }

    async fn show(&mut self, what: &str, scale: Option<f64>, format: Shown) -> Done {
        let client = self.client()?;
        let id = self.resolve(&client, what).await?;
        let mut recipe = client.recipe(id).await?;
        if let Some(factor) = scale {
            if !(factor.is_finite() && factor > 0.0) {
                return Err(Failure::usage("--scale needs a number above zero"));
            }
            scale_recipe(&mut recipe, factor);
        }
        if self.cli.quiet {
            self.line(recipe.id.to_string());
            return Ok(());
        }
        match (self.cli.json, format) {
            (true, _) | (_, Shown::Json) => {
                self.json_out(&serde_json::to_value(&recipe).unwrap_or_default());
            }
            (_, Shown::Markdown) => {
                self.out
                    .push_str(&crumb_core::format::recipe_to_markdown(&recipe));
            }
            (_, Shown::Text) => {
                self.out.push_str(&crumb_core::format::recipe_to_text(
                    &recipe,
                    Some(&format!("{}/recipes/{}", client.base_url(), recipe.id)),
                ));
            }
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        Ok(())
    }

    async fn add(&mut self, source: &str, dry_run: bool) -> Done {
        let client = self.client()?;
        let input = if source == "-" {
            let mut text = String::new();
            self.stdin
                .read_to_string(&mut text)
                .map_err(|e| Failure::usage(format!("Couldn't read stdin: {e}")))?;
            ImportInput::Text(text)
        } else if source.starts_with("http://") || source.starts_with("https://") {
            ImportInput::Url(source.to_string())
        } else {
            let text = std::fs::read_to_string(source).map_err(|e| {
                Failure::usage(format!(
                    "“{source}” isn't a link, and can't be read as a file: {e}"
                ))
            })?;
            ImportInput::Text(text)
        };

        if let ImportInput::Url(url) = &input {
            match client.preview(url).await? {
                Preview::Saved { id, title } => {
                    if self.cli.json {
                        self.json_out(&json!({"id": id, "title": title, "isNew": false}));
                    } else if self.cli.quiet {
                        self.line(id.to_string());
                    } else {
                        self.line(format!("Already in your box as #{id}: {title}"));
                    }
                    return Ok(());
                }
                Preview::Ready { recipe } => {
                    let ingredients: usize = recipe.ingredients.iter().map(|s| s.items.len()).sum();
                    self.note(format!(
                        "{} · {} · {} ingredients",
                        recipe.title,
                        recipe.recipe_yield.as_deref().unwrap_or("no yield"),
                        ingredients
                    ));
                    if dry_run {
                        if self.cli.json {
                            self.json_out(&serde_json::to_value(&*recipe).unwrap_or_default());
                        } else if !self.cli.quiet {
                            self.line(format!("Would save: {}", recipe.title));
                        }
                        return Ok(());
                    }
                }
                Preview::Import => {
                    if dry_run {
                        self.line("A video or shared cookbook: it would be queued, not previewed");
                        return Ok(());
                    }
                }
            }
        } else if dry_run {
            self.line("Would save the text as a recipe");
            return Ok(());
        }

        let mut progress = Vec::new();
        let imported = client
            .import_with_progress(input, |line| progress.push(line.to_string()))
            .await?;
        for line in progress {
            self.note(line);
        }
        let r = &imported.recipe;
        if self.cli.json {
            self.json_out(&json!({
                "id": r.id,
                "title": r.title,
                "isNew": imported.is_new,
                "cookbook": imported.cookbook,
            }));
        } else if self.cli.quiet {
            self.line(r.id.to_string());
        } else if imported.is_new {
            self.line(format!("Saved as #{}: {}", r.id, r.title));
        } else {
            self.line(format!("Already in your box as #{}: {}", r.id, r.title));
        }
        Ok(())
    }

    async fn cooked(&mut self, id: i64, undo: bool) -> Done {
        let client = self.client()?;
        let stats = if undo {
            client.undo_cooked(id, None).await?
        } else {
            client.cooked(id).await?.stats
        };
        if self.cli.json {
            self.json_out(&serde_json::to_value(&stats).unwrap_or_default());
        } else if self.cli.quiet {
            self.line(id.to_string());
        } else {
            self.line(format!(
                "{} cook{} logged{}",
                stats.count,
                if stats.count == 1 { "" } else { "s" },
                stats
                    .last_cooked_at
                    .map(|t| format!(", last {t}"))
                    .unwrap_or_default()
            ));
        }
        Ok(())
    }

    async fn export(
        &mut self,
        id: i64,
        format: Exported,
        output: Option<&std::path::Path>,
    ) -> Done {
        let client = self.client()?;
        let download = client
            .export_recipe(
                id,
                match format {
                    Exported::Md => RecipeFormat::Markdown,
                    Exported::Json => RecipeFormat::Json,
                },
            )
            .await?;
        match output {
            Some(path) => {
                std::fs::write(path, &download.bytes).map_err(|e| {
                    Failure::usage(format!("Couldn't write {}: {e}", path.display()))
                })?;
                self.note(format!("Wrote {}", path.display()));
            }
            None => self.out.push_str(&String::from_utf8_lossy(&download.bytes)),
        }
        Ok(())
    }

    async fn random(&mut self) -> Done {
        let r = self.client()?.random_recipe(None, &[]).await?;
        if self.cli.json {
            self.json_out(&serde_json::to_value(&r).unwrap_or_default());
        } else if self.cli.quiet {
            self.line(r.id.to_string());
        } else {
            self.line(format!("{}\t{}", r.id, r.title));
        }
        Ok(())
    }

    async fn next(&mut self, limit: u32) -> Done {
        let s = self.client()?.suggestions(limit, 0, &[]).await?;
        if self.cli.json {
            self.json_out(&serde_json::to_value(&s.items).unwrap_or_default());
        } else if self.cli.quiet {
            for item in &s.items {
                self.line(item.recipe.id.to_string());
            }
        } else {
            for item in &s.items {
                self.line(format!(
                    "{}\t{}\t{}",
                    item.recipe.id, item.recipe.title, item.reason
                ));
            }
        }
        Ok(())
    }
}

fn scale_recipe(recipe: &mut Recipe, factor: f64) {
    for section in &mut recipe.ingredients {
        for item in &mut section.items {
            *item = scale_ingredient(item, factor);
        }
    }
}
