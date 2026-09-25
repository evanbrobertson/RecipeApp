export interface KitchenTimer {
  id: number
  label: string
  total: number
  endsAt: number
  done: boolean
}

let audio: AudioContext | null = null

/** A cheerful three-note chime (no audio files needed). */
function chime() {
  try {
    audio ??= new AudioContext()
    const now = audio.currentTime
    ;[880, 1175, 1568].forEach((freq, i) => {
      const osc = audio!.createOscillator()
      const gain = audio!.createGain()
      osc.type = "sine"
      osc.frequency.value = freq
      gain.gain.setValueAtTime(0.0001, now + i * 0.18)
      gain.gain.exponentialRampToValueAtTime(0.3, now + i * 0.18 + 0.02)
      gain.gain.exponentialRampToValueAtTime(0.0001, now + i * 0.18 + 0.5)
      osc.connect(gain).connect(audio!.destination)
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

/** Timers that keep running while you move between steps (and pages). */
export function useKitchenTimers() {
  const timers = useState<KitchenTimer[]>("kitchen-timers", () => [])
  const now = useState("kitchen-now", () => Date.now())
  const toast = useToast()

  if (import.meta.client) {
    useIntervalFn(() => {
      now.value = Date.now()
      for (const t of timers.value) {
        if (!t.done && t.endsAt <= now.value) {
          t.done = true
          chime()
          navigator.vibrate?.([200, 100, 200, 100, 400])
          toast.add({ title: `⏰ ${t.label} is up!`, color: "primary", duration: 15000 })
        }
      }
    }, 500)
  }

  function start(label: string, seconds: number) {
    // Unlock audio on this user gesture so the chime can play later (iOS)
    try {
      audio ??= new AudioContext()
      void audio.resume()
    } catch {
      // ignore
    }
    timers.value.push({
      id: Date.now(),
      label,
      total: seconds,
      endsAt: Date.now() + seconds * 1000,
      done: false,
    })
  }

  function remove(id: number) {
    timers.value = timers.value.filter((t) => t.id !== id)
  }

  const remaining = (t: KitchenTimer) => Math.max(0, (t.endsAt - now.value) / 1000)

  return { timers, start, remove, remaining }
}
