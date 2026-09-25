/**
 * Tiny toaster, no framework. `flash()` queues a toast for the next page, since
 * every navigation here is a full page load.
 */
import { read, remove, write } from "./storage"

export interface Toast {
  title: string
  description?: string
  tone?: "default" | "success" | "error" | "primary"
  duration?: number
}

const FLASH = "crumb:flash"

function container(): HTMLElement {
  let el = document.getElementById("toaster")
  if (!el) {
    el = document.createElement("div")
    el.id = "toaster"
    el.setAttribute("aria-live", "polite")
    el.setAttribute("role", "status")
    document.body.append(el)
  }
  return el
}

export function toast(t: Toast) {
  const el = document.createElement("div")
  el.className = `toast toast-${t.tone ?? "default"}`
  const title = document.createElement("p")
  title.className = "toast-title"
  title.textContent = t.title
  el.append(title)
  if (t.description) {
    const d = document.createElement("p")
    d.className = "toast-description"
    d.textContent = t.description
    el.append(d)
  }
  const dismiss = () => {
    el.classList.add("toast-leave")
    setTimeout(() => el.remove(), 200)
  }
  el.addEventListener("click", dismiss)
  container().append(el)
  setTimeout(dismiss, t.duration ?? (t.tone === "error" ? 8000 : 4000))
}

/** Show a toast after the next navigation. */
export function flash(t: Toast) {
  write(FLASH, t, "session")
}

export function showFlash() {
  const t = read<Toast | null>(FLASH, null, "session")
  if (t) {
    remove(FLASH, "session")
    toast(t)
  }
}
