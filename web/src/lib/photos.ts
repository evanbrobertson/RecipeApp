/**
 * Recipes from photos. With Wee Chef, the pages go to the server, which hands them to the
 * vision model (`POST /api/recipes/import/photos`). Without it, tesseract.js reads them
 * right here in the browser and the text goes through the ordinary text import. The OCR
 * engine (about 3 MB, self-hosted) is only loaded on that path, with a dynamic import.
 */
import { api, ApiError, inlineData } from "./api"

export const MAX_PHOTOS = 6
/** Longest side sent anywhere; the server scales to the same size. */
const LONG_EDGE = 1568
const QUALITY = 0.85

export interface PhotoResult {
  id: number
  title: string
  isNew: boolean
  /** Read by OCR on this device, not by Wee Chef: worth checking the amounts. */
  onDevice: boolean
}

/** Pick out the photos (by type, or by extension when the browser doesn't say). */
export function isPhoto(file: File): boolean {
  return file.type.startsWith("image/") || /\.(jpe?g|png|webp|gif|heic|heif|avif)$/i.test(file.name)
}

/**
 * An upright JPEG no bigger than 1568 px, which also leaves the location and other EXIF
 * behind. When the browser can't decode the file (HEIC outside Safari, say), the original
 * is returned with `decoded: false` and the server's 415 explains.
 */
export async function shrink(file: File): Promise<{ blob: Blob; decoded: boolean }> {
  let bitmap: ImageBitmap
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" })
  } catch {
    return { blob: file, decoded: false }
  }
  try {
    const scale = Math.min(1, LONG_EDGE / Math.max(bitmap.width, bitmap.height))
    const w = Math.max(1, Math.round(bitmap.width * scale))
    const h = Math.max(1, Math.round(bitmap.height * scale))
    const canvas = document.createElement("canvas")
    canvas.width = w
    canvas.height = h
    const ctx = canvas.getContext("2d")
    if (!ctx) return { blob: file, decoded: false }
    // Transparent PNGs go on white paper, not black
    ctx.fillStyle = "#fff"
    ctx.fillRect(0, 0, w, h)
    ctx.drawImage(bitmap, 0, 0, w, h)
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", QUALITY),
    )
    return blob ? { blob, decoded: true } : { blob: file, decoded: false }
  } finally {
    bitmap.close()
  }
}

let vision: Promise<boolean> | undefined

/** Whether Wee Chef reads photos here: from the page data when it's there, else the API. */
export function visionAvailable(): Promise<boolean> {
  const inline = inlineData<{ vision?: boolean; connector?: { vision?: boolean } }>()
  const known = inline?.vision ?? inline?.connector?.vision
  if (typeof known === "boolean") return Promise.resolve(known)
  vision ??= api<{ vision?: boolean }>("/api/connector")
    .then((c) => c.vision === true)
    .catch(() => false)
  return vision
}

/** The OCR worker starts from a blob: URL, so every path it gets must be absolute. */
function abs(path: string): string {
  return new URL(path, location.href).href
}

/** OCR in this browser, one page after another, joined into one text. */
async function readOnDevice(pages: Blob[], status: (s: string) => void): Promise<string> {
  status("Getting the reader ready…")
  const [{ createWorker }, workerUrl, coreUrl] = await Promise.all([
    import("tesseract.js/dist/tesseract.esm.min.js").then((m) => m.default),
    import("tesseract.js/dist/worker.min.js?url").then((m) => m.default),
    (simd()
      ? import("tesseract.js-core/tesseract-core-simd-lstm.wasm.js?url")
      : import("tesseract.js-core/tesseract-core-lstm.wasm.js?url")
    ).then((m) => m.default),
  ])
  let page = 0
  const worker = await createWorker("eng", 1, {
    workerPath: abs(workerUrl),
    corePath: abs(coreUrl),
    langPath: abs("/ocr"),
    gzip: true,
    logger: (m) => {
      if (m.status === "recognizing text" && page) {
        const pct = Math.round(m.progress * 100)
        status(
          pages.length > 1
            ? `Reading page ${page} of ${pages.length}… ${pct}%`
            : `Reading the photo… ${pct}%`,
        )
      }
    },
  })
  try {
    const texts: string[] = []
    for (const blob of pages) {
      page++
      const { data } = await worker.recognize(blob)
      texts.push(data.text.trim())
    }
    return texts.filter(Boolean).join("\n\n")
  } finally {
    void worker.terminate()
  }
}

/** WebAssembly SIMD (wasm-feature-detect's probe); older Safari lacks it. */
function simd(): boolean {
  try {
    return WebAssembly.validate(
      new Uint8Array([
        0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0,
        253, 15, 253, 98, 11,
      ]),
    )
  } catch {
    return false
  }
}

/**
 * Reads the pages (in order) as one recipe and saves it. `status` gets short progress
 * lines for the UI. Throws an Error with a readable message.
 */
export async function importPhotos(
  files: Blob[],
  opts: { note?: string; status?: (s: string) => void } = {},
): Promise<PhotoResult> {
  const status = opts.status ?? (() => {})
  const n = files.length
  if (await visionAvailable()) {
    status(`Reading ${n} photo${n === 1 ? "" : "s"}…`)
    const form = new FormData()
    files.forEach((f, i) => form.append("photo", f, `page-${i + 1}.jpg`))
    const note = opts.note?.trim()
    if (note) form.append("text", note)
    try {
      const res = await api<{ id: number; title: string; isNew: boolean }>(
        "/api/recipes/import/photos",
        { method: "POST", form },
      )
      return { ...res, onDevice: false }
    } catch (err) {
      // Wee Chef was switched off since the page loaded: read them here instead
      if (!(err instanceof ApiError && err.status === 400 && /Wee Chef/.test(err.message)))
        throw err
    }
  }
  const text = await readOnDevice(files, status)
  if (text.replace(/\s/g, "").length < 10)
    throw new Error("Couldn't make out any writing. Try a closer, sharper photo in good light.")
  status("Sorting out the recipe…")
  const res = await api<{ id: number; title: string; isNew: boolean }>("/api/recipes/import", {
    method: "POST",
    body: { text },
  })
  return { ...res, onDevice: true }
}
