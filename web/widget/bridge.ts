/**
 * The MCP Apps bridge: JSON-RPC over postMessage between this frame and ChatGPT. Just what the
 * shelf needs: start-up, the tool result that opened it, calling a tool, opening a link,
 * posting a message and asking for more room.
 */

export interface ToolResult {
  content?: { type: string; text?: string }[]
  structuredContent?: Record<string, unknown>
  _meta?: Record<string, unknown>
  isError?: boolean
}

export interface HostContext {
  theme?: "light" | "dark"
  displayMode?: string
}

interface Handlers {
  onResult: (result: ToolResult) => void
  onContext: (context: HostContext) => void
}

const PROTOCOL = "2026-01-26"
const pending = new Map<number, { resolve: (v: any) => void; reject: (e: Error) => void }>()
let nextId = 1

function send(message: Record<string, unknown>) {
  window.parent.postMessage({ jsonrpc: "2.0", ...message }, "*")
}

function request<T = any>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  const id = nextId++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    send({ id, method, params })
  })
}

function notify(method: string, params: Record<string, unknown> = {}) {
  send({ method, params })
}

export async function start(handlers: Handlers) {
  window.addEventListener("message", (event) => {
    if (event.source !== window.parent) return
    const m = event.data
    if (!m || m.jsonrpc !== "2.0") return
    if (m.id != null && !m.method) {
      const waiting = pending.get(m.id)
      pending.delete(m.id)
      if (m.error) waiting?.reject(new Error(m.error.message ?? "Request failed"))
      else waiting?.resolve(m.result)
    } else if (m.method === "ui/notifications/tool-result") {
      handlers.onResult(m.params as ToolResult)
    } else if (m.method === "ui/notifications/host-context-changed") {
      handlers.onContext(m.params as HostContext)
    } else if (m.id != null && m.method === "ping") {
      send({ id: m.id, result: {} })
    }
  })

  const init = await request("ui/initialize", {
    protocolVersion: PROTOCOL,
    appInfo: { name: "Crumb shelf", version: "1" },
    appCapabilities: { availableDisplayModes: ["inline", "fullscreen"] },
  })
  if (init?.hostContext) handlers.onContext(init.hostContext as HostContext)
  notify("ui/notifications/initialized")

  // Tell the host how tall the content is, so the frame fits it
  let last = 0
  const app = document.getElementById("app")!
  new ResizeObserver(() => {
    const height = Math.ceil(app.getBoundingClientRect().height)
    if (height === last) return
    last = height
    notify("ui/notifications/size-changed", { width: window.innerWidth, height })
  }).observe(app)

  // ChatGPT also hands the opening result over as a global, in case it arrived first
  const early = (window as any).openai?.toolOutput
  if (early) {
    handlers.onResult({
      structuredContent: early,
      _meta: (window as any).openai?.toolResponseMetadata,
    })
  }
}

export const callTool = (name: string, args: Record<string, unknown>) =>
  request<ToolResult>("tools/call", { name, arguments: args })

export const openLink = (url: string) => request("ui/open-link", { url }).catch(() => {})

export const say = (text: string) =>
  request("ui/message", { role: "user", content: { type: "text", text } }).catch(() => {})

export const showFullscreen = () =>
  request("ui/request-display-mode", { mode: "fullscreen" }).catch(() => {})
