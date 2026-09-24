import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js"
import { createMcpServer } from "../lib/mcp"

/**
 * Remote MCP endpoint for the Claude connector (Streamable HTTP, stateless).
 * Add `https://<your-app>/mcp` as a custom connector in Claude.
 */
export default defineEventHandler(async (event) => {
  const origin = publicOrigin(event)

  if (!hasValidAccessToken(event)) {
    setResponseStatus(event, 401)
    setHeader(
      event,
      "www-authenticate",
      `Bearer resource_metadata="${origin}/.well-known/oauth-protected-resource/mcp"`,
    )
    return { error: "unauthorized", error_description: "Connect this app to Claude to get a token" }
  }

  // Stateless server: no standalone SSE stream or sessions to terminate
  if (event.method !== "POST") {
    setResponseStatus(event, 405)
    setHeader(event, "allow", "POST")
    return { jsonrpc: "2.0", error: { code: -32000, message: "Method not allowed" }, id: null }
  }

  const body = await readBody(event)
  const server = createMcpServer(origin)
  const transport = new StreamableHTTPServerTransport({
    sessionIdGenerator: undefined,
    enableJsonResponse: true,
  })

  const res = event.node.res
  const done = new Promise<void>((resolve) => {
    res.on("close", resolve)
    res.on("finish", resolve)
  })
  res.on("close", () => {
    void transport.close()
    void server.close()
  })

  await server.connect(transport)
  await transport.handleRequest(event.node.req, res, body)
  await done
})
