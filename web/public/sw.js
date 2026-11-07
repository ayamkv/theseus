// Caches the page and its assets so repeat loads come from disk, and adds the
// cross-origin isolation headers when the server does not set them.
const VERSION = "v2";
const CACHE = `lemonade-${VERSION}`;
const SHELL = [
  "./index.html",
  "./manifest.webmanifest",
  "./icon-180.png",
  "./icon-192.png",
  "./icon-512.png",
];

/// Add the headers the game needs in order to use SharedArrayBuffer.
function isolate(response) {
  if (!response || response.status === 0) return response;
  const headers = new Headers(response.headers);
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Cross-Origin-Embedder-Policy", "require-corp");
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(SHELL))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith("lemonade-") && key !== CACHE)
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== location.origin) return;
  event.respondWith(request.mode === "navigate" ? page(request, event) : asset(request));
});

/// Serve the cached page immediately and refresh it in the background. Only the
/// first visit has no cached page and must wait for the network.
async function page(request, event) {
  const cache = await caches.open(CACHE);
  const update = fetch(request).then(async (response) => {
    if (response.ok) await cache.put("./index.html", response.clone());
    return response;
  });
  const cached = await cache.match("./index.html");
  if (!cached) return isolate(await update);
  event.waitUntil(update.catch(() => {}));
  return isolate(cached);
}

/// Cache first: the wasm, game data and hashed assets never change in place.
async function asset(request) {
  const cache = await caches.open(CACHE);
  const cached = await cache.match(request);
  if (cached) return isolate(cached);
  const response = await fetch(request);
  if (response.ok) await cache.put(request, response.clone());
  return isolate(response);
}
