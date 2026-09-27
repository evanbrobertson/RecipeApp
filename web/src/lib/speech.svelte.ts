import { read, write } from "./storage"

const KEY = "crumb:cook:speak"

// Kitchen shorthand some voices spell out letter by letter
const SPOKEN: [RegExp, string][] = [
  [/\btbsps?\b|\btbs\b/gi, "tablespoons"],
  [/\btsps?\b/gi, "teaspoons"],
  [/\bozs?\b/gi, "ounces"],
  [/\blbs?\b/gi, "pounds"],
  [/\bmins?\b/gi, "minutes"],
  [/\bhrs?\b/gi, "hours"],
  [/(\d)\s*°\s*F\b/g, "$1 degrees Fahrenheit"],
  [/(\d)\s*°\s*C\b/g, "$1 degrees Celsius"],
  [/(\d)\s*°/g, "$1 degrees"],
  [/(\d)\s*-\s*(\d)/g, "$1 to $2"],
]

export function spokenText(text: string): string {
  return SPOKEN.reduce((s, [re, to]) => s.replace(re, to), text)
}

/**
 * Reads cook mode steps aloud with the browser's own speech synthesis.
 * The on/off choice is remembered on this device.
 */
export function stepReader() {
  const supported = typeof window !== "undefined" && "speechSynthesis" in window
  const state = $state({ on: supported && read<boolean>(KEY, false) })

  function stop() {
    if (supported) window.speechSynthesis.cancel()
  }

  function speak(text: string) {
    if (!supported || !state.on) return
    const synth = window.speechSynthesis
    synth.cancel()
    const lang = document.documentElement.lang || navigator.language
    // One utterance per sentence: Chrome cuts long ones off after about 15 seconds
    const sentences = spokenText(text).match(/[^.!?;]+[.!?;]*/g) ?? [text]
    for (const sentence of sentences) {
      if (!sentence.trim()) continue
      const u = new SpeechSynthesisUtterance(sentence.trim())
      u.lang = lang
      u.rate = 0.95
      synth.speak(u)
    }
  }

  function toggle() {
    state.on = !state.on
    write(KEY, state.on)
    if (!state.on) stop()
    // iOS only lets a page talk after it has spoken inside a tap; the step itself follows
    else window.speechSynthesis.speak(new SpeechSynthesisUtterance(""))
  }

  // Leaving the page stops the voice mid-sentence rather than talking over the next one
  if (supported) window.addEventListener("pagehide", stop)

  return {
    supported,
    get on() {
      return state.on
    },
    toggle,
    speak,
    stop,
  }
}
