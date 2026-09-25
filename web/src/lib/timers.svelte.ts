/**
 * Kitchen timers that survive page loads: each one is stored with its absolute end
 * time, so every page's dock picks them up and the chime fires wherever you are.
 */
import { read, write } from "./storage"
import { toast } from "./toast"

export interface KitchenTimer {
  id: number
  label: string
  total: number
  endsAt: number
  done: boolean
}

const KEY = "crumb:timers"

let audio: AudioContext | null = null

/** A cheerful three-note chime (no audio files needed). */
function chime() {
  try {
    audio ??= new AudioContext()
    const ctx = audio
    const now = ctx.currentTime
    ;[880, 1175, 1568].forEach((freq, i) => {
      const osc = ctx.createOscillator()
      const gain = ctx.createGain()
      osc.type = "sine"
      osc.frequency.value = freq
      gain.gain.setValueAtTime(0.0001, now + i * 0.18)
      gain.gain.exponentialRampToValueAtTime(0.3, now + i * 0.18 + 0.02)
      gain.gain.exponentialRampToValueAtTime(0.0001, now + i * 0.18 + 0.5)
      osc.connect(gain).connect(ctx.destination)
      osc.start(now + i * 0.18)
      osc.stop(now + i * 0.18 + 0.55)
    })
  } catch {
    // audio unavailable
  }
}

export function formatClock(seconds: number) {
  const s = Math.max(0, Math.round(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  return h
    ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`
    : `${m}:${String(sec).padStart(2, "0")}`
}

class Timers {
  list = $state<KitchenTimer[]>(read<KitchenTimer[]>(KEY, []))
  now = $state(Date.now())

  constructor() {
    setInterval(() => this.tick(), 500)
    // Another tab started or dismissed a timer
    addEventListener("storage", (e) => {
      if (e.key === KEY) this.list = read<KitchenTimer[]>(KEY, [])
    })
  }

  private save() {
    write(KEY, this.list)
  }

  private tick() {
    this.now = Date.now()
    let changed = false
    for (const t of this.list) {
      if (!t.done && t.endsAt <= this.now) {
        t.done = true
        changed = true
        // A timer that ran out while no page was open just shows as done
        if (this.now - t.endsAt < 5000 && document.visibilityState === "visible") {
          chime()
          navigator.vibrate?.([200, 100, 200, 100, 400])
        }
        toast({ title: `⏰ ${t.label} is up!`, tone: "primary", duration: 15000 })
      }
    }
    if (changed) this.save()
  }

  start(label: string, seconds: number) {
    // Unlock audio on this user gesture so the chime can play later (iOS)
    try {
      audio ??= new AudioContext()
      void audio.resume()
    } catch {
      // ignore
    }
    this.list.push({
      id: Date.now(),
      label,
      total: seconds,
      endsAt: Date.now() + seconds * 1000,
      done: false,
    })
    this.save()
  }

  remove(id: number) {
    this.list = this.list.filter((t) => t.id !== id)
    this.save()
  }

  remaining(t: KitchenTimer) {
    return Math.max(0, (t.endsAt - this.now) / 1000)
  }
}

export const timers = new Timers()
