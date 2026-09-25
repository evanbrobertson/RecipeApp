import { sqliteTable, text, integer, index, uniqueIndex } from "drizzle-orm/sqlite-core"
import type { RecipeSection, RecipeSource } from "#shared/utils/recipe"

const timestamp = (name: string) =>
  integer(name, { mode: "timestamp" })
    .notNull()
    .$defaultFn(() => new Date())

export const recipes = sqliteTable(
  "recipes",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    // Null for recipes pasted as text or added by Claude without a source link
    url: text("url").unique(),
    source: text("source").$type<RecipeSource>().notNull().default("url"),
    title: text("title").notNull(),
    description: text("description"),
    image: text("image"),
    author: text("author"),
    prepTime: text("prep_time"),
    cookTime: text("cook_time"),
    totalTime: text("total_time"),
    freezeTime: text("freeze_time"),
    recipeYield: text("recipe_yield"),
    recipeCategory: text("recipe_category"),
    recipeCuisine: text("recipe_cuisine"),
    ingredients: text("ingredients", { mode: "json" }).notNull().$type<RecipeSection[]>(),
    instructions: text("instructions", { mode: "json" }).notNull().$type<RecipeSection[]>(),
    nutrition: text("nutrition", { mode: "json" }).$type<Record<string, string>>(),
    notes: text("notes"),
    createdAt: timestamp("created_at"),
    updatedAt: timestamp("updated_at"),
  },
  (t) => [index("recipes_created_at_idx").on(t.createdAt)],
)

export type Recipe = typeof recipes.$inferSelect
export type NewRecipe = typeof recipes.$inferInsert

export const cookbooks = sqliteTable("cookbooks", {
  id: integer("id").primaryKey({ autoIncrement: true }),
  name: text("name").notNull(),
  description: text("description"),
  // Cloth colour of the book on the shelf (a key from BOOK_COLORS)
  color: text("color"),
  createdAt: timestamp("created_at"),
})

export type Cookbook = typeof cookbooks.$inferSelect

export const cookbookRecipes = sqliteTable(
  "cookbook_recipes",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    cookbookId: integer("cookbook_id")
      .notNull()
      .references(() => cookbooks.id, { onDelete: "cascade" }),
    recipeId: integer("recipe_id")
      .notNull()
      .references(() => recipes.id, { onDelete: "cascade" }),
  },
  (t) => [uniqueIndex("cookbook_recipes_unique").on(t.cookbookId, t.recipeId)],
)

/** OAuth clients registered by Claude (dynamic client registration). */
export const oauthClients = sqliteTable("oauth_clients", {
  id: text("id").primaryKey(),
  name: text("name"),
  redirectUris: text("redirect_uris", { mode: "json" }).notNull().$type<string[]>(),
  createdAt: timestamp("created_at"),
})

/** Authorization codes, access tokens and refresh tokens. Only SHA-256 hashes are stored. */
export const oauthTokens = sqliteTable(
  "oauth_tokens",
  {
    hash: text("hash").primaryKey(),
    kind: text("kind").$type<"code" | "access" | "refresh">().notNull(),
    clientId: text("client_id").notNull(),
    // Code-only fields for PKCE + redirect validation
    codeChallenge: text("code_challenge"),
    redirectUri: text("redirect_uri"),
    expiresAt: integer("expires_at", { mode: "timestamp" }).notNull(),
    createdAt: timestamp("created_at"),
  },
  (t) => [index("oauth_tokens_expires_idx").on(t.expiresAt)],
)
