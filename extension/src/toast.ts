/**
 * The little card the extension shows on a page, in a closed shadow root so the page's
 * styles can't reach it (nor its styles the page). Crumb's Green Tile look: a paper card,
 * one butter button for the main action.
 */

export interface ToastAction {
  label: string
  primary?: boolean
  run: () => void
}

export interface ToastOptions {
  title: string
  detail?: string
  /** A short line under the detail, wrapped rather than cut off. */
  note?: string
  actions: ToastAction[]
  /** Gone after this long unless the pointer or focus is on it. */
  timeoutMs?: number
}

// The favicon (web/public/favicon.svg), inline: nothing is fetched from the extension, so
// pages can't tell it's installed by probing for its files
const ICON = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" aria-hidden="true"><defs><mask id="bite"><rect width="64" height="64" fill="#fff"/><circle cx="47.5" cy="15" r="8.5" fill="#000"/><circle cx="53" cy="25" r="6.8" fill="#000"/><circle cx="39" cy="10.5" r="5.5" fill="#000"/></mask></defs><rect width="64" height="64" rx="16" fill="#2f6b4f"/><g mask="url(#bite)"><circle cx="32.5" cy="36" r="20" fill="#9c5a2c" opacity=".35"/><circle cx="31" cy="34" r="20" fill="#f8d59a"/><circle cx="31" cy="34" r="20" fill="none" stroke="#e7b36a" stroke-width="2.2"/></g><g fill="#6b3a22"><ellipse cx="23" cy="28" rx="3.2" ry="2.8" transform="rotate(-20 23 28)"/><ellipse cx="34" cy="40" rx="3" ry="2.6" transform="rotate(15 34 40)"/><ellipse cx="22.5" cy="42" rx="2.4" ry="2.1"/><ellipse cx="35" cy="27.5" rx="2.3" ry="2"/><ellipse cx="42" cy="37" rx="2" ry="1.8"/></g><g fill="#f8d59a"><circle cx="54" cy="36.5" r="2"/><circle cx="57.5" cy="31" r="1.3"/><circle cx="51" cy="42" r="1.2"/></g></svg>`

const CLOSE = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>`

const STYLE = `
:host { all: initial; }
.card {
  --paper: #fffdf8; --text: #1c2b22; --muted: #56635a; --line: #e3dfd0;
  --butter: #f3da8b; --butter-hover: #edcf6f; --on-butter: #1c2b22; --ghost-hover: #efeadd;
  position: fixed; z-index: 2147483647; right: 16px; bottom: 16px;
  width: min(384px, calc(100vw - 32px)); box-sizing: border-box;
  display: grid; grid-template-columns: 40px 1fr auto; gap: 4px 12px; align-items: start;
  padding: 16px; border: 1px solid var(--line); border-radius: 16px;
  background: var(--paper); color: var(--text);
  box-shadow: 0 12px 32px rgb(28 43 34 / 0.18), 0 2px 6px rgb(28 43 34 / 0.08);
  font: 15px/1.4 ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  animation: rise 180ms ease-out;
}
@media (prefers-color-scheme: dark) {
  .card {
    --paper: #1d2721; --text: #efe9da; --muted: #a8b3aa; --line: #2c3830;
    --butter: #f0d582; --butter-hover: #e8c96a; --on-butter: #141c17; --ghost-hover: #26322b;
  }
}
@media (max-width: 480px) { .card { right: 16px; left: 16px; width: auto; } }
@media (prefers-reduced-motion: reduce) { .card { animation: none; } }
@keyframes rise { from { opacity: 0; transform: translateY(8px); } }
.icon { width: 40px; height: 40px; grid-row: span 2; }
.icon svg { width: 100%; height: 100%; display: block; }
.title { margin: 0; font-weight: 700; font-size: 15px; }
.detail {
  grid-column: 2; margin: 0; color: var(--muted); font-size: 14px;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.note { grid-column: 2 / -1; margin: 0; color: var(--muted); font-size: 14px; }
.close {
  grid-row: 1; grid-column: 3; display: grid; place-items: center;
  width: 32px; height: 32px; margin: -8px -8px 0 0; padding: 0; border: 0; border-radius: 999px;
  background: none; color: var(--muted); cursor: pointer;
}
.close svg { width: 18px; height: 18px; }
.actions { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
button.act {
  min-height: 40px; padding: 0 16px; border-radius: 12px; border: 1px solid var(--line);
  background: none; color: var(--text); font: inherit; font-weight: 700; cursor: pointer;
}
button.act.primary { background: var(--butter); color: var(--on-butter); border-color: transparent; }
button.act.primary:hover { background: var(--butter-hover); }
button.act:not(.primary):hover, .close:hover { background: var(--ghost-hover); }
button:focus-visible { outline: 2px solid #2f6b4f; outline-offset: 2px; }
`

/** An inline icon from the markup above (parsed as SVG, never as HTML). */
function svg(markup: string): Node {
  const doc = new DOMParser().parseFromString(markup, "image/svg+xml")
  return document.importNode(doc.documentElement, true)
}

let current: HTMLElement | null = null

/** Takes down the card, if one is up. */
export function hideToast() {
  current?.remove()
  current = null
}

/** Shows the card (replacing any other). */
export function showToast(options: ToastOptions) {
  hideToast()
  const host = document.createElement("crumb-extension-toast")
  const root = host.attachShadow({ mode: "closed" })
  const style = document.createElement("style")
  style.textContent = STYLE
  const card = document.createElement("div")
  card.className = "card"
  card.setAttribute("role", "dialog")
  card.setAttribute("aria-label", options.title)

  const icon = document.createElement("div")
  icon.className = "icon"
  icon.append(svg(ICON))
  const title = document.createElement("p")
  title.className = "title"
  title.textContent = options.title
  const close = document.createElement("button")
  close.className = "close"
  close.type = "button"
  close.setAttribute("aria-label", "Close")
  close.append(svg(CLOSE))
  close.addEventListener("click", hideToast)
  card.append(icon, title, close)

  if (options.detail) {
    const detail = document.createElement("p")
    detail.className = "detail"
    detail.textContent = options.detail
    detail.title = options.detail
    card.append(detail)
  }
  if (options.note) {
    const note = document.createElement("p")
    note.className = "note"
    note.textContent = options.note
    card.append(note)
  }
  const actions = document.createElement("div")
  actions.className = "actions"
  for (const action of options.actions) {
    const button = document.createElement("button")
    button.type = "button"
    button.className = action.primary ? "act primary" : "act"
    button.textContent = action.label
    button.addEventListener("click", () => {
      hideToast()
      action.run()
    })
    actions.append(button)
  }
  card.append(actions)
  root.append(style, card)

  // Esc closes it; it waits while it's being used
  card.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hideToast()
  })
  if (options.timeoutMs) {
    let timer = setTimeout(hideToast, options.timeoutMs)
    const hold = () => clearTimeout(timer)
    const resume = () => {
      clearTimeout(timer)
      timer = setTimeout(hideToast, options.timeoutMs)
    }
    card.addEventListener("pointerenter", hold)
    card.addEventListener("focusin", hold)
    card.addEventListener("pointerleave", resume)
    card.addEventListener("focusout", resume)
  }

  document.documentElement.append(host)
  current = host
}
