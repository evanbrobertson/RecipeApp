import { claudeAvailable } from "../lib/claude-extract"
import { browserAvailable } from "../lib/browser"

export default defineEventHandler((event) => ({
  mcpUrl: `${publicOrigin(event)}/mcp`,
  authEnabled: authEnabled(),
  claudeParsing: claudeAvailable(),
  browserScraping: browserAvailable(),
}))
