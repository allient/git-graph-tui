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

Free, fast worldwide, and unlimited bandwidth for static sites.

In the Cloudflare dashboard, create an application from the GitHub repository (Workers & Pages) with:

| Setting | Value |
| --- | --- |
| Build command | `bun install && bun run build:docs` |
| Output directory | `apps/docs/dist` |
| Root directory | `/` (the repository root) |
| Environment variable | `SITE_URL` = your site's address |

Every push to `main` deploys, and pull requests get preview URLs.

## Other free hosts

| Host | Build command | Output directory |
| --- | --- | --- |
| [GitHub Pages](https://docs.github.com/pages) (with a GitHub Actions workflow) | `bun install && bun run build:docs` | `apps/docs/dist` |
| [Netlify](https://www.netlify.com/) | `bun install && bun run build:docs` | `apps/docs/dist` |
| [Vercel](https://vercel.com/) | `bun install && bun run build:docs` | `apps/docs/dist` |

On GitHub Pages under `https://allient.github.io/git-graph-tui/`, the site lives under a sub-path: also set `base: '/git-graph-tui'` in `apps/docs/astro.config.mjs`.
