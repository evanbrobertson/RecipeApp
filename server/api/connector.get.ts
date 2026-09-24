import { claudeAvailable } from "../lib/claude-extract"

export default defineEventHandler((event) => ({
  mcpUrl: `${publicOrigin(event)}/mcp`,
  authEnabled: authEnabled(),
  claudeParsing: claudeAvailable(),
}))
