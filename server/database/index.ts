import { mkdirSync } from "node:fs"
import { dirname, join } from "node:path"
import Database from "better-sqlite3"
import { drizzle } from "drizzle-orm/better-sqlite3"
import * as schema from "./schema"

export type DB = ReturnType<typeof drizzle<typeof schema>>

let _db: DB | null = null

/**
 * Resolves the SQLite file location. On Railway, attach a volume and the file
 * lands on it automatically via RAILWAY_VOLUME_MOUNT_PATH.
 */
export function databasePath(): string {
  if (process.env.DATABASE_PATH) return process.env.DATABASE_PATH
  if (process.env.RAILWAY_VOLUME_MOUNT_PATH) {
    return join(process.env.RAILWAY_VOLUME_MOUNT_PATH, "recipes.db")
  }
  return ".data/recipes.db"
}

const recipesTableSql = (name: string) => `
  CREATE TABLE IF NOT EXISTS ${name} (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    url text,
    source text DEFAULT 'url' NOT NULL,
    title text NOT NULL,
    description text,
    image text,
    author text,
    prep_time text,
    cook_time text,
    total_time text,
    freeze_time text,
    recipe_yield text,
    recipe_category text,
    recipe_cuisine text,
    ingredients text NOT NULL,
    instructions text NOT NULL,
    nutrition text,
    notes text,
    created_at integer NOT NULL,
    updated_at integer NOT NULL
  );
`

const bootstrapSql = `
  ${recipesTableSql("recipes")}
  CREATE UNIQUE INDEX IF NOT EXISTS recipes_url_unique ON recipes (url);
  CREATE INDEX IF NOT EXISTS recipes_created_at_idx ON recipes (created_at);

  CREATE TABLE IF NOT EXISTS cookbooks (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    name text NOT NULL,
    description text,
    created_at integer NOT NULL
  );

  CREATE TABLE IF NOT EXISTS cookbook_recipes (
    id integer PRIMARY KEY AUTOINCREMENT NOT NULL,
    cookbook_id integer NOT NULL REFERENCES cookbooks(id) ON DELETE CASCADE,
    recipe_id integer NOT NULL REFERENCES recipes(id) ON DELETE CASCADE
  );
  CREATE UNIQUE INDEX IF NOT EXISTS cookbook_recipes_unique ON cookbook_recipes (cookbook_id, recipe_id);

  CREATE TABLE IF NOT EXISTS oauth_clients (
    id text PRIMARY KEY NOT NULL,
    name text,
    redirect_uris text NOT NULL,
    created_at integer NOT NULL
  );

  CREATE TABLE IF NOT EXISTS oauth_tokens (
    hash text PRIMARY KEY NOT NULL,
    kind text NOT NULL,
    client_id text NOT NULL,
    code_challenge text,
    redirect_uri text,
    expires_at integer NOT NULL,
    created_at integer NOT NULL
  );
  CREATE INDEX IF NOT EXISTS oauth_tokens_expires_idx ON oauth_tokens (expires_at);
`

/**
 * Upgrades databases created by the previous version of the app:
 * url was NOT NULL and there were no source/updated_at columns.
 */
function upgradeLegacySchema(sqlite: Database.Database) {
  const columns = sqlite.prepare("PRAGMA table_info(recipes)").all() as {
    name: string
    notnull: number
  }[]
  if (columns.length === 0) return
  const byName = new Map(columns.map((c) => [c.name, c]))
  const needsRebuild =
    byName.get("url")?.notnull === 1 || !byName.has("source") || !byName.has("updated_at")
  if (!needsRebuild) return

  const legacyColumns = [
    "id",
    "url",
    "title",
    "description",
    "image",
    "author",
    "prep_time",
    "cook_time",
    "total_time",
    "freeze_time",
    "recipe_yield",
    "recipe_category",
    "recipe_cuisine",
    "ingredients",
    "instructions",
    "nutrition",
    "notes",
    "created_at",
  ].filter((c) => byName.has(c))
  const list = legacyColumns.join(", ")

  sqlite.pragma("foreign_keys = OFF")
  sqlite.transaction(() => {
    // SQLite's recommended table rebuild: create, copy, drop, rename
    sqlite.exec(recipesTableSql("recipes_new"))
    sqlite.exec(
      `INSERT INTO recipes_new (${list}, source, updated_at)
       SELECT ${list}, 'url', created_at FROM recipes`,
    )
    sqlite.exec("DROP TABLE recipes")
    sqlite.exec("ALTER TABLE recipes_new RENAME TO recipes")
  })()
  sqlite.pragma("foreign_keys = ON")
}

export function useDB(): DB {
  if (_db) return _db

  const file = databasePath()
  if (
    process.env.RAILWAY_ENVIRONMENT &&
    !process.env.RAILWAY_VOLUME_MOUNT_PATH &&
    !process.env.DATABASE_PATH
  ) {
    console.warn(
      "[db] No Railway volume attached: recipes will be lost on every deploy. Add a volume to this service.",
    )
  }
  mkdirSync(dirname(file), { recursive: true })

  const sqlite = new Database(file)
  sqlite.pragma("journal_mode = WAL")
  sqlite.pragma("synchronous = NORMAL")
  sqlite.pragma("busy_timeout = 5000")
  sqlite.pragma("foreign_keys = ON")

  upgradeLegacySchema(sqlite)
  sqlite.exec(bootstrapSql)

  _db = drizzle(sqlite, { schema })
  return _db
}
