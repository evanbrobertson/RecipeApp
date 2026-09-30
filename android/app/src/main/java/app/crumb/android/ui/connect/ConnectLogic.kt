package app.crumb.android.ui.connect

// The Connect page's wording that isn't already shared Rust logic (web/src/pages/connect.astro).

/** The line the Claude Code section tells the cook to run. */
fun claudeCommand(mcpUrl: String): String = "claude mcp add --transport http recipes $mcpUrl"
