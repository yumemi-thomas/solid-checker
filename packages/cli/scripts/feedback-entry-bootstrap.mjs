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
  const origin = new URL(documentUrl).origin, base = server.config.base;
  const entries = [], client = server.environments?.client ?? server;
  for (const row of rows) {
    assert(row.url, "Browser feedback requires external module entries; inline module entries cannot be bootstrapped exactly");
    const url = new URL(row.url);
    assert.equal(url.origin, origin, "Browser feedback cannot bootstrap a foreign module entry");
    let request = url.pathname + url.search;
    if (request.startsWith(base)) request = "/" + request.slice(base.length);
    if (request === "/@vite/client") continue;
    let id = decodeURIComponent(request);
    if (id.startsWith("/@id/")) id = id.slice(5).replace("__x00__", "\0");
    const resolved = await client.pluginContainer.resolveId(id);
    assert(resolved && !resolved.external, `Browser feedback cannot resolve module entry: ${row.url}`);
    entries.push({ url: row.url, request, id: resolved.id });
  }
  assert(entries.length, "Browser feedback needs a resolvable application module entry");
  return entries;
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

export function validateFeedbackDocument(final, documents, navigated = 0) {
  assert(final?.id && final.state?.stats, "Browser feedback bootstrap did not bind the loaded document; runtime collection is unavailable");
  assert(!documents || documents.loaded >= navigated, "Browser feedback bootstrap missed a navigated document; runtime collection is unavailable");
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
