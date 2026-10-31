"use strict";

const CACHE_PREFIX = `plataformer:${self.registration.scope}:`;
const CACHE_NAME = `${CACHE_PREFIX}__BUILD_ID__`;
const ASSETS = [
  "./",
  "./index.html",
  "./app.css",
  "./app.js",
  "./manifest.webmanifest",
  "./mq_js_bundle.js",
  "./platformer.wasm",
  "./icons/icon-192.png",
  "./icons/icon-512.png",
  "./icons/icon-maskable-512.png",
  "./icons/apple-touch-icon.png",
].map((path) => new URL(path, self.registration.scope).href);

self.addEventListener("install", (event) => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE_NAME);
    await cache.addAll(ASSETS.map((url) => new Request(url, { cache: "reload" })));
  })());
});

self.addEventListener("activate", (event) => {
  event.waitUntil((async () => {
    const names = await caches.keys();
    await Promise.all(names
      .filter((name) => name.startsWith(CACHE_PREFIX) && name !== CACHE_NAME)
      .map((name) => caches.delete(name)));
    await self.clients.claim();
  })());
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin ||
      !url.href.startsWith(self.registration.scope)) return;
  url.search = "";
  if (request.mode !== "navigate" && !ASSETS.includes(url.href)) return;
  event.respondWith((async () => {
    const cache = await caches.open(CACHE_NAME);
    const key = request.mode === "navigate"
      ? new URL("./index.html", self.registration.scope).href
      : url.href;
    const cached = await cache.match(key);
    return cached || fetch(request);
  })());
});
