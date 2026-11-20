// Serves falcon.getocean.dev from the GitHub Pages site.
//
// GitHub Pages allows one custom domain per repository, and the site already
// uses falcon.vivekyadav.online. This Worker answers on the second domain by
// fetching the same path from the Pages site, so both domains always show the
// same build and nothing extra has to be deployed when the site changes.

const ORIGIN = "https://falcon.vivekyadav.online";

// Request headers worth passing through for a static site.
const FORWARDED = [
  "accept",
  "accept-encoding",
  "accept-language",
  "if-modified-since",
  "if-none-match",
  "range",
  "user-agent",
];

export default {
  async fetch(request) {
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("Method Not Allowed", {
        status: 405,
        headers: { allow: "GET, HEAD" },
      });
    }

    const url = new URL(request.url);
    const upstream = new URL(url.pathname + url.search, ORIGIN);
    const headers = new Headers();
    for (const name of FORWARDED) {
      const value = request.headers.get(name);
      if (value !== null) headers.set(name, value);
    }

    const response = await fetch(upstream, {
      method: request.method,
      headers,
      redirect: "manual",
    });

    // Pages redirects (for example /docs to /docs/) name its own host. Point
    // them back at this domain so visitors stay on falcon.getocean.dev.
    const location = response.headers.get("location");
    if (location === null) return response;
    const target = new URL(location, upstream);
    if (target.host !== upstream.host) return response;
    target.protocol = url.protocol;
    target.host = url.host;
    const rewritten = new Headers(response.headers);
    rewritten.set("location", target.toString());
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: rewritten,
    });
  },
};
