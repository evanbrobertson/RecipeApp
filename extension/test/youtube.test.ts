import { describe, expect, test } from "bun:test"
import {
  findAll,
  json3Text,
  jsonAfter,
  looksLikeRecipe,
  MAX_TRANSCRIPT,
  pickTrack,
  playerCaptionUrl,
  readPlayer,
  transcriptPanelText,
  videoId,
} from "../src/youtube"

describe("videoId", () => {
  test("every shape of a YouTube video link", () => {
    for (const url of [
      "https://www.youtube.com/watch?v=Xy_djhH3WE4&t=122s",
      "https://m.youtube.com/watch?v=Xy_djhH3WE4",
      "https://youtu.be/Xy_djhH3WE4?si=abc",
      "https://www.youtube.com/shorts/Xy_djhH3WE4",
      "https://www.youtube.com/live/Xy_djhH3WE4?feature=share",
    ]) {
      expect(videoId(url)).toBe("Xy_djhH3WE4")
    }
  })

  test("not a video", () => {
    for (const url of [
      undefined,
      "not a url",
      "https://www.youtube.com/",
      "https://www.youtube.com/watch",
      "https://www.youtube.com/@chef",
      "https://notyoutube.com/watch?v=Xy_djhH3WE4",
      "https://www.youtube.com/watch?v=<script>",
    ]) {
      expect(videoId(url)).toBeNull()
    }
  })
})

const player = {
  videoDetails: {
    videoId: "Xy_djhH3WE4",
    title: "Weeknight pad thai",
    lengthSeconds: "612",
    author: "Noodle Co",
    shortDescription: 'Ingredients:\n200 g noodles\n{"not": "json"} and a } brace',
    thumbnail: {
      thumbnails: [
        { url: "https://i.ytimg.com/vi/Xy_djhH3WE4/default.jpg", width: 120 },
        { url: "https://i.ytimg.com/vi/Xy_djhH3WE4/maxresdefault.jpg", width: 1280 },
      ],
    },
  },
  captions: {
    playerCaptionsTracklistRenderer: {
      captionTracks: [
        { baseUrl: "/api/timedtext?v=x&lang=de", languageCode: "de" },
        { baseUrl: "/api/timedtext?v=x&lang=en&kind=asr", languageCode: "en", kind: "asr" },
        { baseUrl: "/api/timedtext?v=x&lang=en-GB", languageCode: "en-GB" },
      ],
    },
  },
}

describe("reading the watch page", () => {
  test("finds the player response however the page wraps it", () => {
    const html = `<script>var ytInitialPlayerResponse = ${JSON.stringify(player)};var meta = {"a": 1};</script>`
    expect(jsonAfter(html, "ytInitialPlayerResponse = ")).toEqual(player)
    expect(jsonAfter("<html></html>", "ytInitialPlayerResponse = ")).toBeNull()
    expect(jsonAfter("ytInitialPlayerResponse = {broken", "ytInitialPlayerResponse = ")).toBeNull()
  })

  test("the video's details and caption tracks", () => {
    const { video, tracks } = readPlayer(player)
    expect(video).toEqual({
      title: "Weeknight pad thai",
      description: 'Ingredients:\n200 g noodles\n{"not": "json"} and a } brace',
      author: "Noodle Co",
      thumbnail: "https://i.ytimg.com/vi/Xy_djhH3WE4/maxresdefault.jpg",
      duration: 612,
    })
    expect(tracks).toHaveLength(3)
    expect(readPlayer(null)).toEqual({ video: {} as never, tracks: [] })
  })

  test("the cook's own English captions before automatic ones", () => {
    const { tracks } = readPlayer(player)
    expect(pickTrack(tracks)?.languageCode).toBe("en-GB")
    expect(pickTrack(tracks.slice(0, 2))?.kind).toBe("asr")
    expect(pickTrack(tracks.slice(0, 1))).toBeNull()
  })
})

describe("caption words", () => {
  test("a json3 track as one paragraph", () => {
    const json = {
      events: [
        { tStartMs: 0 },
        { segs: [{ utf8: "so today" }, { utf8: " we're making" }] },
        { segs: [{ utf8: "\n" }] },
        { segs: [{ utf8: "pad thai" }] },
      ],
    }
    expect(json3Text(json)).toBe("so today we're making pad thai")
    expect(json3Text({})).toBe("")
  })

  test("the transcript panel's segments, wherever they're nested", () => {
    const seg = (text: string) => ({
      transcriptSegmentRenderer: { snippet: { runs: [{ text }] } },
    })
    const json = {
      actions: [
        { panel: { body: { segments: [seg("Soak the noodles."), seg("Fry the tofu.")] } } },
      ],
    }
    expect(transcriptPanelText(json)).toBe("Soak the noodles. Fry the tofu.")
    expect(findAll({ a: { b: 1 }, c: [{ b: 2 }] }, "b")).toEqual([1, 2])
  })

  test("cut to what Crumb keeps", () => {
    const long = { events: [{ segs: [{ utf8: "word ".repeat(MAX_TRANSCRIPT) }] }] }
    expect(json3Text(long).length).toBe(MAX_TRANSCRIPT)
  })
})

test("offers only on videos that say they're a recipe", () => {
  expect(looksLikeRecipe({ title: "Pad thai", description: "Full recipe below" })).toBe(true)
  expect(looksLikeRecipe({ title: "Ingredients you need for pad thai" })).toBe(true)
  expect(looksLikeRecipe({ title: "My trip to Bangkok", description: "Vlog" })).toBe(false)
})

test("the player's own caption request, as json3", () => {
  const loaded = [
    "https://www.youtube.com/s/player/abc/base.js",
    "https://www.youtube.com/api/timedtext?v=other&lang=en&pot=zzz&fmt=srv3",
    "https://www.youtube.com/api/timedtext?v=Xy_djhH3WE4&lang=en&pot=abc&fmt=srv3",
  ]
  const url = new URL(playerCaptionUrl(loaded, "Xy_djhH3WE4")!)
  expect(url.searchParams.get("pot")).toBe("abc")
  expect(url.searchParams.get("fmt")).toBe("json3")
  expect(playerCaptionUrl(loaded.slice(0, 2), "Xy_djhH3WE4")).toBeNull()
})
