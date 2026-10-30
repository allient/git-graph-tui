---
title: Deploying the docs
description: Host this documentation site for free.
---

The site builds to plain static files, so any static host works, and the hosts below are free for open-source projects.

```sh
bun install
bun run build:docs       # output: apps/docs/dist
```

Set `SITE_URL` to the final address when building (for example `SITE_URL=https://git-graph-tui.pages.dev bun run build:docs`), so the sitemap and canonical links use it.

## Cloudflare (recommended)

The site is deployed to [Cloudflare Workers](https://developers.cloudflare.com/workers/static-assets/) as static files only, with no Worker script. Requests to static assets are free and unlimited, bandwidth is free, and Cloudflare's DDoS protection is included on every plan.

The `Deploy docs` GitHub Actions workflow (`.github/workflows/deploy-docs.yml`) builds the site and runs `wrangler deploy` (configured in `apps/docs/wrangler.jsonc`):

- after every successful release (a `v*` tag push), from the released commit;
- or by hand: **Actions → Deploy docs → Run workflow**.

It needs these settings in the GitHub repository (**Settings → Secrets and variables → Actions**):

| Name | Kind | Value |
| --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | Secret | An API token made from the **Edit Cloudflare Workers** template |
| `CLOUDFLARE_ACCOUNT_ID` | Secret | Your account ID (Cloudflare dashboard → Workers & Pages, right side) |
| `SITE_URL` | Variable (optional) | The site's address, for example `https://git-graph-tui-docs.<your-subdomain>.workers.dev` or your own domain |

The first deploy creates the `git-graph-tui-docs` Worker. To use your own domain, add it under the Worker's **Settings → Domains & Routes** and set `SITE_URL` to it.

To deploy from your machine instead: `bun run deploy:docs` (it asks you to log in to Cloudflare the first time).

## Other free hosts

| Host | Build command | Output directory |
| --- | --- | --- |
| [GitHub Pages](https://docs.github.com/pages) (with a GitHub Actions workflow) | `bun install && bun run build:docs` | `apps/docs/dist` |
| [Netlify](https://www.netlify.com/) | `bun install && bun run build:docs` | `apps/docs/dist` |
| [Vercel](https://vercel.com/) | `bun install && bun run build:docs` | `apps/docs/dist` |

On GitHub Pages under `https://allient.github.io/git-graph-tui/`, the site lives under a sub-path: also set `base: '/git-graph-tui'` in `apps/docs/astro.config.mjs`.
