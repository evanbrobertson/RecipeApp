# Other blogging platforms: Substack, Ghost, Blogger, Shopify

A research spike from 2026-09-29. The question was whether Crumb should recognise these four platforms
the way it recognises WordPress (`src/scraper/platforms.rs`, `src/scraper/fallbacks.rs`, `scrape_plan`,
the `platform` column in `sites.db`). No code was changed.

**Verified** means a request made from the research sandbox on 2026-09-29. The sandbox reaches the web
through a cloud egress proxy, not from Railway, so "not blocked" means not blocked from there. **Read**
means taken from documentation or terms without testing. No host was blocked by the proxy.

## Summary

| | Substack | Ghost | Blogger | Shopify blogs |
|---|---|---|---|---|
| Cheap detection | Yes. `*.substack.com`; `substackcdn.com` in HTML; `x-served-by: Substack` | Yes. `<meta name="generator" content="Ghost 6.x">`; `data-key` + `data-api` in the head | Yes. `*.blogspot.com`; `<meta content='blogger' name='generator'/>`; `server: GSE` | Yes. `cdn.shopify.com` in the `Link` header; `powered-by: Shopify`; `Shopify.shop = "x.myshopify.com"` |
| Public API, full post by URL | Yes. `/api/v1/posts/{slug}` (no key) | Yes. Content API with the key the page publishes | Yes. `/feeds/posts/default?alt=json&path=` (no key) | Partly. Storefront GraphQL without a token; the article body is often empty |
| Recipe JSON-LD on the page | No (NewsArticle only) | Only if the publisher adds it (Alison Roman does) | No (BlogPosting microdata) | Only through a recipe app (Recipe Kit) |
| Terms ban automated access | **Yes. Platform-wide** | ghost.org has a clause, but it binds Ghost's customers | Only against robots.txt; the default allows posts | Shopify's own terms bind merchants; **many stores' own terms ban it** |
| Bot blocking seen | None | None | None | None |
| Recommendation | **Skip the API. A person decides whether to list `substack.com`** | **Later** (small) | **Skip the API**; build a prose read instead | **Skip the API**; help the Flagger find store terms |

The main finding is that none of the four blocked a server fetch. On every platform, the page itself
carried everything the API did. The WordPress API earns its place because many recipe blogs put their
pages behind a bot check and leave the REST API open. None of these platforms showed that pattern.

The real gap is different. Substack and Blogger posts, and many Shopify and Ghost posts, have **no
structured recipe**. The recipe is prose, or a heading with a list, in the post body. Today
`parse_recipe_html` only reads JSON-LD, then microdata, so these pages fail with "Couldn't find a
recipe on that page". An API can't fix that. A prose read of the post body would (see
[Cross-cutting](#cross-cutting-a-prose-read-of-the-post-body)).

---

## Substack

### Detection (verified)

- **The host.** `{pub}.substack.com` is unmistakable. It could go in `platform_from_url`, and the post
  path is always `/p/{slug}`.
- **Custom domains.** Examples are `www.lennysnewsletter.com`, `www.slowboring.com` and
  `www.vittlesmagazine.com`. All three answered with these headers:
  `x-served-by: Substack`, `x-cluster: substack`, `x-sub: {pub}`, `x-powered-by: Express` and
  `server: cloudflare`. Their HTML references `substackcdn.com` (the `Link` header has
  `<https://substackcdn.com>; rel=preconnect`). `Fetched::Page` only carries `link`, so the header check
  would need `crumb-fetch` to pass the header through. The HTML marker `substackcdn.com` is enough
  without that.
- `/tos` on a custom domain redirects with a 302 to `https://substack.com/tos`. The footer links
  "Terms of Use" to `https://substack.com/tos`.

### API (verified)

| Request | Result |
|---|---|
| `GET https://juliaturshen.substack.com/api/v1/posts/easy-roasted-tomato-sauce-d1d` | 200 JSON, 80 KB. Its keys include `body_html` (69 KB), `title`, `subtitle`, `cover_image`, `canonical_url`, `audience`, `post_date`, `publishedBylines` and `postTags` |
| same, `the-simplest-curried-chickpeas` (`audience: only_paid`) | 200. `body_html` is only the free preview (23 KB) and ends in "Become a paid subscriber" |
| same, an unknown slug | 404 `{"error":"Post not found","type":"single"}` |
| `GET /api/v1/archive?sort=new&limit=5` (Alison Roman, Ottolenghi, Smitten Kitchen, Julia Turshen) | 200 JSON array of posts (`slug`, `audience`, `canonical_url`) |
| The same with `User-Agent: curl/8` (lennysnewsletter.com) | 200 |

The API is undocumented and has no key. Custom domains serve it too. The post page is served as full
HTML (297 KB, `cf-cache-status: HIT`) with the same body in it, so the API adds nothing when the page
loads.

### Recipe data (verified)

The page has one JSON-LD block. It is `NewsArticle`, with `BreadcrumbList`, `Person` and
`ImageObject`, and **no `Recipe`**. Julia Turshen's post is prose ("STEP ONE: procure a lot of
tomatoes…"). Ottolenghi's paid preview has `<strong>` recipe names and `<ul>` ingredient lists. Crumb
imports none of these today. Paid posts only give the preview, on both the page and the API.

### Terms (verified, read in full)

[Substack Terms of Use](https://substack.com/tos), last updated 2025-04-21, section "Acceptable Use
Policy":

> You also agree that you will not contribute any Post or otherwise use Substack in a manner that: …
> "Crawls," "scrapes," or "spiders" any page, data, or portion of Substack (through use of manual or
> automated means); Copies or stores any significant portion of the content on Substack; …

The clause is platform-wide and applies to every reader. It also covers custom domains, which are
"Substack". It has the same shape as the People Inc clause already in `data/site-terms.toml` ("any
manual or automated software… to 'scrape'"). By that precedent it is an explicit ban on automated
scraping. The README also says "a clause that also forbids copying by hand does not count as a reason
to list a site". A person should decide how that sentence applies, since it reads against the People
Inc entries too.

**If it is listed:**
- The entry would be `hosts = ["substack.com"]` (which covers `*.substack.com`), with
  `image_hosts = ["substackcdn.com", "substack-post-media.s3.amazonaws.com"]`.
- Custom domains can't be caught by a host list before a fetch. Two options: let the Flagger find them
  one at a time (their footer "Terms of Use" links to substack.com/tos), or add a platform veto that
  stops a scrape as soon as a response shows `substackcdn.com` / `x-served-by: Substack`, and have
  site memory remember that host. The second option still sends one request before the veto, so it
  weakens the "no request goes out" guarantee for custom domains only.
- The extension only reads JSON-LD or a recipe card. On a Substack post it would find neither, so the
  nudge toward the extension would also fail until the extension gets a prose read.

### Bot blocking

None seen. Pages and the API answered 200 with a Firefox UA and with `curl/8`.

### Recommendation: **skip the API**

The API only duplicates the page, and the terms forbid scraping. A person should decide whether to
list `substack.com`. If Substack is not listed, the only useful work is the prose read below.

---

## Ghost

### Detection (verified)

Two sites were checked: `www.alisoneroman.com`, which is Ghost(Pro) on a custom domain, and
`gather-and-sow.ghost.io`.

- `<meta name="generator" content="Ghost 6.65">`
- Portal and search scripts in the head carry the Content API key and address:
  ```html
  <script src="https://cdn.jsdelivr.net/ghost/portal@~2.71/umd/portal.min.js"
    data-ghost="https://www.alisoneroman.com/" data-key="3473ce2252305060f99e25c8ca"
    data-api="https://alisoneroman.ghost.io/ghost/api/content/" …>
  <script src="…/sodo-search.min.js" data-key="3473ce2252305060f99e25c8ca"
    data-sodo-search="https://alisoneroman.ghost.io/" …>
  ```
- Headers: `server: openresty`, `via: 1.1 varnish` (×3), `x-cache`. The `Link` header has no marker.
- `{sub}.ghost.io` is unmistakable from the link. Self-hosted Ghost can only be detected from the page.

### API (verified; key rules read)

The [Ghost docs](https://docs.ghost.org/content-api) say: "Content API keys are provided via a query
parameter in the URL. These keys are safe for use in browsers and other insecure environments, as they
only ever provide access to public data." They also say "your admin domain may differ from your site
domain" (Ghost(Pro) uses `*.ghost.io`).

| Request | Result |
|---|---|
| `GET https://www.alisoneroman.com/ghost/api/content/posts/?key=…` | **302** to `https://alisoneroman.ghost.io/ghost/api/content/…`, a different host |
| `GET https://alisoneroman.ghost.io/ghost/api/content/posts/?key=…&fields=slug,url,visibility` | 200. 737 posts, with `visibility` `public` or `paid` |
| `GET …/posts/slug/braised-chicken-piccata-2/?key=…` | 200. Its keys include `html` (3.5 KB: `<h2>Ingredients</h2><ul>…`, `<h2>Preparation</h2>`), `codeinjection_head`, `feature_image`, `url` and `canonical_url` |
| `GET …/posts/slug/perfect-pancakes-perfect-waffles/?key=…` (paid) | 200, `html` empty |
| any request without `key` | 302, then an error |

### Recipe data (verified)

Ghost itself only emits `Article`. Alison Roman adds a full schema.org `Recipe` through per-post code
injection, and the API returns it in `codeinjection_head`. The page carries the same JSON-LD, so
Crumb imports her recipes today from the page. Posts without code injection are headings and lists
(`h2 Ingredients` + `ul`, `h2 Preparation`), which the prose read would handle.

### Terms (verified)

[Ghost.org Terms of Service](https://ghost.org/terms/), section 2.2 "Prohibitions":

> By using any Service, you agree not to … Attempt to access or search any Service, or download
> without authorization data (including Content) from any Service through the use of any engine,
> software, tool, agent, device or mechanism (including spiders, robots, crawlers, data mining tools or
> the like) other than the Software and/or search agents provided by Ghost Foundation or by other
> generally available third-party web browsers

"Services" means the ghost.org Website, the Software and the Hosted Service (Ghost(Pro)). The terms
are an agreement with Ghost's customers, meaning the people who sign up. They don't clearly bind a
reader of a publisher's site, and they don't apply to self-hosted Ghost at all. "Without
authorization" is also hard to square with a key the publisher put in the page for public use.
**This does not justify a platform-wide listing.** Each publisher's own terms apply, and the Flagger
already checks those per host.

### Bot blocking

None seen. Pages, the API and a `curl/8` UA all got 200.

### Recommendation: **later** (about 1 to 1.5 days)

The API would only help when a Ghost page is blocked, and none were. There is also a catch: the key
comes from the page, so a blocked page can't provide it. It would have to come from site memory
(`api_root` holding the `data-api` address and key, which are public facts about a host) or from a
`*.ghost.io` link. The code is cheap and the data is clean, so it's worth building when a blocked
Ghost site turns up.

Sketch:
- `Platform::Ghost` (`"ghost"`). In `platform_from_url`, match a host ending `.ghost.io`. In
  `detect_platform`, match `content="Ghost ` or `data-ghost=`.
- `platforms::ghost_lead(html, page)`: read `data-key` and `data-api` from `script[data-key][data-api]`.
  Unlike `wp_lead`, it must accept an API on another host, but only the page's own host or a
  `*.ghost.io` host, so a page can't point the fetch anywhere else. The key must be hex, 26
  characters.
- `fallbacks::fetch_ghost`: `GET {api}posts/slug/{slug}/?key=…&fields=url,title,html,feature_image,codeinjection_head,custom_excerpt`.
  Check `same_page(url, post.url)`. Read `codeinjection_head` with `parse_recipe_html` first (its
  JSON-LD), then `html` with the prose read. The slug is the last path segment, as `slug_of` finds it
  for WordPress.
- Generalise `WpLead` into a `Lead` enum, `Method::WordPress` into a platform-API method (keep the
  `wordpress-api` label for old site memory), and `Steps.wordpress` into a platform-API switch. There is
  no blind attempt on a blocked page (that stays WordPress-only), only when the platform is known.

---

## Blogger (Blogspot)

### Detection (verified)

- Hosts: `*.blogspot.com` (plus the country TLDs such as `blogspot.co.uk`, which redirect).
- `<meta content='blogger' name='generator'/>`, `server: GSE`.
- Every page names its blog and post ids:
  `https://www.blogger.com/feeds/{blogId}/posts/default` and
  `…/feeds/{postId}/comments/default` in `<link rel="alternate" type="application/atom+xml">`.
  These work on custom domains too.

### API (verified)

The Blogger feeds are public. The documented [Blogger API v3](https://developers.google.com/blogger/docs/3.0/reference/posts/getByPath)
`posts/bypath` needs a Google API key. The feed below does not need one.

| Request | Result |
|---|---|
| `GET {blog}/feeds/posts/default?alt=json&max-results=3` | 200 JSON. `feed.entry[]` has `content.$t` (full HTML), unless the blog serves short feeds: technicolorkitcheninenglish and andersenrecipes return only `summary` |
| `GET {blog}/feeds/posts/default?alt=json&path=/2026/04/80-isnt-for-sissis.html` (iliketobakeandcook, 40th newest post) | 200, exactly that post (35 KB content). The `path` parameter is undocumented but works |
| same, a path that doesn't exist | 404 |
| `GET {blog}/feeds/posts/default/{postId}?alt=json` | 200, that post |
| `GET https://www.blogger.com/feeds/{blogId}/posts/default?alt=json&path=…` | 200, the same post, served from Google's host rather than the blog's |

### Recipe data (verified)

There is no JSON-LD. Microdata is `schema.org/BlogPosting` and `Person` only. The Grandma's Quick Fix
post is plain lines separated by `<br>`: "2 lbs carrots / 3 tbsp melted butter / … / Preheat oven to
400-degrees. …". `crumb_core::text_parser::parse_recipe_text` is built for text like that. The feed's
`content.$t` is the same HTML as the page's post body.

### Terms (verified)

[Google Terms of Service](https://policies.google.com/terms), effective 2026-07-30, which cover Blogger:

> using automated means to access content from any of our services in violation of the
> machine-readable instructions on our web pages (for example, robots.txt files that disallow
> crawling, training, or other activities)

The ban is tied to robots.txt. A blogspot blog's default robots.txt
(`grandmasquickfixrecipes.blogspot.com/robots.txt`, verified) is
`User-agent: * / Disallow: /search / Disallow: /share-widget / Allow: /`, so posts and `/feeds/` are
allowed. Crumb ignores robots.txt on purpose, but here Google's terms make robots.txt the rule. That
only matters for a blog with a custom robots.txt that disallows `/`, which is rare. **Blogger should
not be listed.** If Blogger support is built, it could check the blog's robots.txt before the feed.

### Bot blocking

None seen. `curl/8` got 200 on a post. A custom domain in front of Cloudflare could block, and the
`www.blogger.com/feeds/{blogId}` form would get round that. No such case was found.

### Recommendation: **skip the API; build the prose read**

The page loads, and the feed holds the same HTML. What Blogger needs is a way to read an unstructured
post (below). The feed would be about half a day on top, only for a blocked custom domain:
`platform_from_url` for `blogspot.*`, and a lead holding `(feed base, postId)` from the comments-feed
link.

---

## Shopify store blogs

### Detection (verified)

Checked flybyjing.com, diasporaco.com, graza.co, brightland.co and burlapandbarrel.com.

- `powered-by: Shopify` header. The `Link` header (which `Fetched::Page` already carries) has
  `<https://cdn.shopify.com>; rel="preconnect"` and `</.well-known/ucp>; rel="ucp"`.
- HTML: `Shopify.shop = "getgraza.myshopify.com"`, `cdn.shopify.com`, `/cdn/shop/t/…`.
- Articles live at `/blogs/{blog}/{article}`. That path is a strong hint, but not unmistakable before
  a fetch.

### API (verified)

| Request | Result |
|---|---|
| `GET /blogs/recipes/{article}.json` (4 stores) | **404** `{"errors":"Not Found"}`. Unlike `/products/{x}.json`, articles have no `.json` view |
| `GET /blogs/recipes/{article}.atom`, `?format=json` | 404 |
| `GET /blogs/recipes.atom` (graza) | 200 Atom, 30 entries with only `<summary>` |
| `GET /blogs/recipes.atom` (diaspora) | 200 Atom, 30 entries with `<content>` |
| `GET /products.json?limit=1` (graza) | 200 JSON (`products[]` with `body_html`) |
| `POST /api/2025-07/graphql.json` `{ blog(handle:"recipes"){ articleByHandle(handle:"blueberry-muffins"){ title contentHtml image{url} } } }`, **no token** | 200. The Storefront API allows Blogs and Articles without a token ([docs](https://shopify.dev/docs/api/storefront): "Products and Collections, Selling Plans, Search, Pages, Blogs, and Articles, Cart"; complexity limit 1,000; "automated traffic … [is] limited", with Web Bot Auth to raise it). `contentHtml` was `""` (graza), 165 characters (brightland), 293 characters (burlap) and 789 characters (diaspora, "FOR THE FILLING") |

Food brands build recipe pages from theme sections, metafields or recipe apps, so the article body is
usually a stub. The recipe isn't in what the API returns.

### Recipe data (verified)

Four of the four article pages had JSON-LD. It was `Article`, `WebPage` and `Organization` from the
theme. Only Burlap & Barrel had a `Recipe`, from the Recipe Kit app (`recipekit` markup), and Crumb
already reads that from the page.

### Terms (verified)

- [Shopify Terms of Service](https://www.shopify.com/legal/terms): "You agree not to access the
  Services or monitor any material or information from the Services using any robot, spider, scraper,
  or other automated means." This is Shopify's agreement with **merchants** (account holders). It
  doesn't bind visitors to a store's storefront. The [Storefront API](https://shopify.dev/docs/api/storefront)
  docs say "You can't use Storefront API to duplicate existing Shopify functionality" and point to the
  API terms, which are aimed at app developers.
- **The stores' own terms** (`/policies/terms-of-service`, 5 checked) often forbid scraping:
  - Burlap & Barrel and Fly By Jing use Shopify's old template, Section 12 "Prohibited uses": "you are
    prohibited from using the site or its content: … (i) to spam, phish, pharm, pretext, spider, crawl,
    or scrape".
  - Diaspora Co and Brightland: "Attempt to access or search the Services or Content or download
    Content from the Services, through the use of any engine, software, tool, agent, device or
    mechanism (including spiders, robots, crawlers, data mining tools or the like) other than the
    software and/or search agents provided by Company or other generally available third-party web
    browsers".
  - Graza: no such clause.

  These are per-store bans, so they belong in `site-terms.toml` one store at a time, through the
  Flagger and a person's review. They are not a platform listing. One gap: the Flagger's fallback paths
  are `/terms-of-service`, `/terms-of-use` and `/terms`. Shopify's are at `/policies/terms-of-service`.
  The footer link usually finds them, but adding that path to `find_terms` (or trying it when the
  platform is Shopify) would be a one-line improvement.

### Bot blocking

None seen from the sandbox. `curl/8` got 200 on an article. The stores are behind Cloudflare, and
Shopify says automated traffic is rate-limited.

### Recommendation: **skip the API**

The Storefront API rarely holds the recipe, and many stores' terms forbid scraping anyway. It is worth
recognising Shopify only to point the Flagger at `/policies/terms-of-service` (about an hour).

---

## Cross-cutting: a prose read of the post body

The platform APIs above wouldn't save any recipe that fails today. A fallback for a page that loaded but
has no recipe would. `judge` gives `Verdict::Failed("Couldn't find a recipe on that page.")` there, and
this fallback would take the post body instead:

1. **Find the body.**
   - Substack: `.body.markup` (or `body_html`).
   - Ghost: `.gh-content` / `.post-content`.
   - Blogger: `.post-body`.
   - Shopify: `.article__content` / `.rte`.
   - Otherwise `article`.
2. **Read it.** First as headed lists: `platforms::headed_lists` already turns `h2 Ingredients` + `ul`
   into sections. Otherwise run the text through `crumb_core::text_parser::parse_recipe_text` (the
   `<br>` line style on Blogger). Accept the result only when it has both ingredients and steps.
3. **Use Wee Chef only if a key is set.** It already parses pasted text (`llm.rs`), and it could take
   posts like Julia Turshen's "STEP ONE: …" prose, which the heuristics can't. This is the same path as
   pasting the post's text.

The result must be marked as a guess (for example, the Jev check flagging it for review) so a blog post
with a shopping list isn't saved as a recipe. The extension would need the same read (the post body,
capped) to be useful on Substack.

Effort: about 2 to 3 days with tests. The pages to test against are the ones above.

## Where a new platform plugs in (for later)

- `platforms::Platform`: a new variant, with `as_str`/`parse` (the `sites.db` `platform` column is free
  text, so no migration).
- `platform_from_url`: host suffixes (`.ghost.io`, `.blogspot.*`, `.substack.com`).
  `detect_platform`: the HTML markers above. It takes `html` only, so the `Link` header is where
  Shopify's marker is.
- `Seen::of` / the lead: today it is `Option<WpLead>`. It would become an enum per platform, and site
  memory's `api_root` would hold the Ghost `data-api` + key or the Blogger feed base.
- `Method::WordPress` / `Steps.wordpress` / `SCRAPE_WORDPRESS`: generalise these to "the platform's
  API" and keep `wordpress-api` as the stored label.
- `scrape_plan` step 4 tries the WordPress API blindly on a blocked page. Keep that for WordPress only.
  Other platforms should be tried only when known from the link or site memory.
- Every new request goes through `crumb_fetch::guard` like the WordPress one, and `site_terms::guard`
  still runs first in `scrape_page`.
