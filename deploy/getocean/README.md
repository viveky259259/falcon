# falcon.getocean.dev

The Falcon site is built from `site/` and published by GitHub Pages at
<https://falcon.vivekyadav.online>. GitHub Pages serves one custom domain per
repository, so `falcon.getocean.dev` is served by the Cloudflare Worker in this
folder instead. It fetches each path from the Pages site and returns it, so the
two domains always show the same build.

## Deploy

The zone lives in **Faiz@getocean.dev's Account**, which `wrangler.toml` pins
by `account_id`. Run this from this folder with a Cloudflare login that has
both of these in that account:

- **Workers Scripts: Edit** on the account, to upload the Worker.
- **Workers Routes: Edit** on the `getocean.dev` zone, to attach or change the
  `falcon.getocean.dev` custom domain. Without it the upload succeeds but the
  domain step fails.

```bash
npx wrangler@4 deploy
```

Wrangler creates the `falcon.getocean.dev` DNS record and its certificate. The
Worker does not need redeploying when the site changes; redeploy only when
`src/index.js` or `wrangler.toml` change.

## Behaviour

- Only `GET` and `HEAD` are served; other methods get `405`.
- Redirects from Pages that point at its own host are rewritten to
  `falcon.getocean.dev`, so visitors stay on this domain.
- Absolute URLs inside pages, such as the link-preview image, still point at
  `falcon.vivekyadav.online`.
