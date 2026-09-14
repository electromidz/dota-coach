/**
 * App-shell service worker.
 *
 * Deliberately small, and deliberately not a data cache. Three rules:
 *
 *   1. **Never cache the API.** Every `/api/` response is personal, session-
 *      scoped and time-sensitive; a stale benchmark or a cached entitlement is
 *      worse than no answer at all. Those requests are not intercepted, so the
 *      browser handles them exactly as it would without a worker.
 *   2. **Cache only what is immutable.** Next emits content-hashed assets under
 *      `/_next/static/`; those can be served from the cache forever. Everything
 *      else is fetched fresh.
 *   3. **Navigations fall back, they do not go stale.** A page load goes to the
 *      network; if the network is gone, the offline page explains that rather
 *      than the browser's dinosaur.
 *
 * Bumping CACHE_VERSION retires every previous cache on the next activation.
 */

const CACHE_VERSION = "v1";
const SHELL_CACHE = `dota-coach-shell-${CACHE_VERSION}`;
const OFFLINE_URL = "/offline";

const PRECACHE = [OFFLINE_URL, "/icons/icon-192.png"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(SHELL_CACHE)
      .then((cache) => cache.addAll(PRECACHE))
      // A failed precache must not leave a half-installed worker in place.
      .then(() => self.skipWaiting())
      .catch(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith("dota-coach-") && key !== SHELL_CACHE)
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;

  // Only plain GETs are ever served from a cache; a POST is an action, and
  // replaying one from storage would be a bug with consequences.
  if (request.method !== "GET") return;

  const url = new URL(request.url);

  // Cross-origin — including every call to the backend — is none of our
  // business.
  if (url.origin !== self.location.origin) return;
  if (url.pathname.startsWith("/api/")) return;

  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request).catch(() =>
        caches.match(OFFLINE_URL).then(
          (cached) =>
            cached ??
            new Response("You are offline.", {
              status: 503,
              headers: { "Content-Type": "text/plain" },
            }),
        ),
      ),
    );
    return;
  }

  // Content-hashed build output: the URL changes when the bytes change, so a
  // cache hit is always correct and always current.
  if (url.pathname.startsWith("/_next/static/") || url.pathname.startsWith("/icons/")) {
    event.respondWith(
      caches.match(request).then((cached) => {
        if (cached) return cached;

        return fetch(request).then((response) => {
          if (response.ok) {
            const copy = response.clone();
            caches.open(SHELL_CACHE).then((cache) => cache.put(request, copy));
          }
          return response;
        });
      }),
    );
  }
});
