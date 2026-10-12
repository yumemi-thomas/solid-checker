import assert from "node:assert/strict";

// Runs in an inert browser document. Use the browser's HTML/base/URL rules,
// including raw-text scripts, entities and captured /@id/ entry spellings.
export function feedbackHtmlEntries({ html, url }) {
  const document = new DOMParser().parseFromString(html, "text/html");
  const base = new URL(document.querySelector("base[href]")?.getAttribute("href") ?? url, url).href;
  return [...document.querySelectorAll('script[type="module"]')].map(script => ({
    url: script.hasAttribute("src") ? new URL(script.getAttribute("src"), base).href : null,
    inline: script.hasAttribute("src") ? null : script.textContent
  }));
}

export async function resolveFeedbackEntries(rows, documentUrl, server) {
  // Vite's base middleware strips the raw (unnormalized) base, not config.base.
  const origin = new URL(documentUrl).origin, base = server.config.rawBase ?? server.config.base;
  const entries = [], client = server.environments?.client ?? server;
  for (const row of rows) {
    assert(row.url, "Browser feedback requires external module entries; inline module entries cannot be bootstrapped exactly");
    const url = new URL(row.url);
    assert.equal(url.origin, origin, "Browser feedback cannot bootstrap a foreign module entry");
    const request = normalizeFeedbackEntryRequest(url.pathname + url.search, base);
    if (request === "/@vite/client") continue;
    const resolved = await client.pluginContainer.resolveId(request);
    assert(resolved && !resolved.external, `Browser feedback cannot resolve module entry: ${row.url}`);
    entries.push({ url: row.url, request, id: resolved.id });
  }
  assert(entries.length, "Browser feedback needs a resolvable application module entry");
  return entries;
}

// The selected entry for a request at the client transform boundary. Vite's
// HTTP middleware has already base-stripped, timestamp-stripped, URI-decoded
// and unwrapped that request; decoding it again would throw on a literal `%`
// or change its identity, so it is compared exactly as Vite passes it.
export function selectedFeedbackEntry(entries, request) {
  return entries.find(row => row.request === request);
}

// Vite 8's base/transform middleware helpers are private. Mirror its exact
// transport operations, including preserving URI-reserved query characters.
export function normalizeFeedbackEntryRequest(request, base = "/") {
  // Vite's baseMiddleware (installed only for a base other than "/"): when the
  // clean pathname starts with the raw base, stripBase the whole URL — an exact
  // match becomes "/", a `${base}/…` prefix is removed, anything else is kept.
  if (base !== "/" && request.replace(/[?#].*$/s, "").startsWith(base)) {
    const devBase = base.endsWith("/") ? base : base + "/";
    if (request === base) request = "/";
    else if (request.startsWith(devBase)) request = request.slice(devBase.length - 1);
  }
  request = decodeURI(request.replace(/\bt=\d{13}&?\b/, "").replace(/[?&]$/, "")).replace("__x00__", "\0");
  request = request.replace(/(\?|&)import=?(?:&|$)/, "$1").replace(/[?&]$/, "");
  return request.startsWith("/@id/") ? request.slice(5).replace("__x00__", "\0") : request;
}

// Leave the entry bytes otherwise intact. A first static import evaluates the
// bootstrap before the entry's other dependencies, even for async script tags.
// The identity map includes each UTF-16 column, so attribution stays exact.
export function bootstrapFeedbackEntry(code, id, specifier) {
  const vlq = value => {
    let bits = value < 0 ? -value * 2 + 1 : value * 2, text = "";
    do { const digit = bits % 32; bits = Math.floor(bits / 32); text += "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"[digit + (bits ? 32 : 0)]; } while (bits);
    return text;
  };
  const lines = code.split("\n");
  return { code: `import ${JSON.stringify(specifier)};\n${code}`, map: {
    version: 3, names: [], sources: [id], sourcesContent: [code],
    mappings: ";" + lines.map((line, index) => (index ? "AAC" + vlq(-lines[index - 1].length) : "AAAA") + ",CAAC".repeat(line.length)).join(";")
  } };
}

export function validateFeedbackDocument(final) {
  assert(final?.id && final.state?.stats, "Browser feedback bootstrap did not bind the loaded document; runtime collection is unavailable");
}

export function pinFeedbackEntryOutput(result, id, specifier, base) {
  const expected = base + "@id/" + specifier.replace("\0", "__x00__");
  assert(result?.code.startsWith(`import ${JSON.stringify(expected)};`),
    `Completed entry output does not start with the collector import: ${id}`);
  return result.code;
}

export function completeFeedbackEntryOutput(result, id, specifier, base) {
  assert(typeof result?.code === "string", "Selected entry has no completed Vite output");
  const prefix = `import ${JSON.stringify(base + "@id/" + specifier.replace("\0", "__x00__"))};\n`;
  // Import analysis itself can prepend Vite's HMR/dynamic-import helpers.
  // Reassert ordering at the HTTP transform boundary, after every hook. The
  // already-resolved bootstrap dependency remains in Vite's module graph;
  // importing it again does not evaluate it twice. Shift the completed map.
  if (!result.code.startsWith(prefix)) {
    assert(!result.map || typeof result.map.mappings === "string", "Completed entry source map cannot be shifted exactly");
    result = { ...result, code: prefix + result.code,
      map: result.map ? { ...result.map, mappings: ";" + result.map.mappings } : null };
  }
  pinFeedbackEntryOutput(result, id, specifier, base);
  return result;
}

export function validateFeedbackServedEntries(urls, entries, auxiliary) {
  assert(Array.isArray(urls), "Browser feedback could not observe the served document's module entries");
  const selected = new Set(entries.map(row => row.url)), allowed = new Set([...selected, ...auxiliary]);
  assert(urls.every(url => url && allowed.has(url)), "Served HTML contains an unselected module entry; browser feedback cannot prove bootstrap order");
  assert([...selected].every(url => urls.includes(url)), "Served HTML replaced a selected module entry; browser feedback cannot prove bootstrap order");
}

export function feedbackEntryBootstrapPlugin(entries, specifier, observe = () => {}) {
  return { name: "solid-checker-feedback-entry-bootstrap", enforce: "post",
    transform(code, id, options) {
      if (options?.ssr || (this.environment && this.environment.name !== "client") || !entries.has(id)) return null;
      const result = bootstrapFeedbackEntry(code, id, specifier);
      observe(id, code, result.code);
      return result;
    }
  };
}
