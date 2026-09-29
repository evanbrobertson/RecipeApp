/**
 * Sites whose terms of service forbid automated fetching (data/site-terms.toml, the same
 * reviewed list the server uses). Bun's bundler turns the TOML into JSON at build time, so the
 * extension carries the list itself and makes no request of its own to know it.
 *
 * Crumb's server never fetches such a site; the extension is how its recipes get in, because
 * the page is read here, in the cook's own browser. This only decides what to say about that.
 */
import list from "../../data/site-terms.toml"

export interface TermsSite {
  name: string
  hosts: string[]
}

const sites: TermsSite[] = (list as { site: TermsSite[] }).site

/** The listed site `hostname` belongs to (a listed host or any subdomain of it), or null. */
export function termsSite(hostname: string, all: TermsSite[] = sites): TermsSite | null {
  const host = hostname.toLowerCase().replace(/\.$/, "")
  return (
    all.find((s) =>
      s.hosts.some((h) => host === h || host.endsWith(`.${h.replace(/^www\./, "")}`)),
    ) ?? null
  )
}

/** The short line for a listed site's page, or null on any other site. */
export function termsNote(hostname: string): string | null {
  const site = termsSite(hostname)
  return site
    ? `${site.name}'s terms don't allow Crumb's server to fetch it, so Crumb will read it from this page.`
    : null
}
