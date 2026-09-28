/** The settings page: the Crumb's address, the offer on or off, and the sites it's off for. */
import { crumbOrigin, load, save } from "./crumb"

const form = document.getElementById("address-form") as HTMLFormElement
const address = document.getElementById("address") as HTMLInputElement
const hint = document.getElementById("address-hint") as HTMLParagraphElement
const prompt = document.getElementById("prompt") as HTMLInputElement
const mutedSection = document.getElementById("muted-section") as HTMLElement
const mutedList = document.getElementById("muted") as HTMLUListElement
const defaultHint = hint.textContent ?? ""

function say(text: string, error = false) {
  hint.textContent = text
  hint.classList.toggle("error", error)
  address.setAttribute("aria-invalid", String(error))
}

async function render() {
  const settings = await load()
  if (document.activeElement !== address) address.value = settings.crumb
  prompt.checked = settings.prompt
  mutedSection.hidden = settings.muted.length === 0
  mutedList.replaceChildren(
    ...settings.muted.map((site) => {
      const item = document.createElement("li")
      const name = document.createElement("span")
      name.textContent = site
      const allow = document.createElement("button")
      allow.type = "button"
      allow.textContent = "Offer again"
      allow.addEventListener("click", async () => {
        const { muted } = await load()
        await save({ muted: muted.filter((s) => s !== site) })
      })
      item.append(name, allow)
      return item
    }),
  )
}

form.addEventListener("submit", async (e) => {
  e.preventDefault()
  if (!address.value.trim()) {
    await save({ crumb: "" })
    return say(defaultHint)
  }
  const origin = crumbOrigin(address.value)
  if (!origin) return say("That doesn't look like a web address.", true)
  await save({ crumb: origin })
  address.value = origin
  say("Saved. Recipe pages will offer to open in this Crumb.")
})

prompt.addEventListener("change", () => void save({ prompt: prompt.checked }))

chrome.storage.onChanged.addListener(() => void render())
void render()
