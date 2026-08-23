// npm registry access for benchmark discovery.
//
// Discovery is the only network-enabled part of the benchmark; execution reads
// the pinned manifest. Everything here is therefore concentrated behind one
// module so `--offline` execution can be proven to touch no endpoint.

const DEFAULT_REGISTRY = "https://registry.npmjs.org";
const DEFAULT_DOWNLOADS_API = "https://api.npmjs.org";

// The abbreviated packument carries dist-tags, dependencies, peerDependencies
// and dist.integrity for every version at a fraction of the full document.
const ABBREVIATED = "application/vnd.npm.install-v1+json";

export function encodePackageName(name) {
  return name.startsWith("@") ? `@${encodeURIComponent(name.slice(1))}` : encodeURIComponent(name);
}

// Discovery enumerates several npm orgs and then fetches a packument per
// package, which is enough traffic to earn a 429 from the registry. The
// registry answers those with a `retry-after` header naming the seconds to
// wait -- observed at 35, far longer than an exponential backoff starting at
// 250ms will ever reach. Ignoring it turns a routine throttle into a failed
// discovery run, so a throttled response waits exactly as long as the
// registry asked and does not consume one of the ordinary retry attempts.
const THROTTLED = new Set([429, 503]);
const MAX_THROTTLE_WAIT_MS = 120_000;
const MAX_THROTTLE_RETRIES = 6;

function throttleDelayMs(response) {
  const header = response.headers?.get?.("retry-after");
  const seconds = Number(header);
  if (Number.isFinite(seconds) && seconds > 0) {
    return Math.min(seconds * 1000, MAX_THROTTLE_WAIT_MS);
  }
  // No usable header: the registry still wants us to slow down, and retrying
  // in milliseconds would just earn another 429.
  return 5000;
}

async function requestJson(url, { accept, attempts = 4, fetchImpl = fetch, allowMissing = false } = {}) {
  let lastError;
  let throttleRetries = 0;
  for (let attempt = 1; attempt <= attempts; attempt++) {
    try {
      const response = await fetchImpl(url, { headers: accept ? { accept } : {} });
      if (THROTTLED.has(response.status) && throttleRetries < MAX_THROTTLE_RETRIES) {
        throttleRetries++;
        attempt--;
        await new Promise(resolve => setTimeout(resolve, throttleDelayMs(response)));
        continue;
      }
      if (response.status === 404 || response.status === 405) {
        if (allowMissing) return null;
        throw new Error(`${url} returned ${response.status}`);
      }
      if (!response.ok) throw new Error(`${url} returned ${response.status}`);
      return await response.json();
    } catch (error) {
      lastError = error;
      if (attempt === attempts) break;
      await new Promise(resolve => setTimeout(resolve, 250 * 2 ** (attempt - 1)));
    }
  }
  throw lastError;
}

export class Registry {
  constructor({
    registry = process.env.SOLID_BENCHMARK_REGISTRY ?? DEFAULT_REGISTRY,
    downloadsApi = process.env.SOLID_BENCHMARK_DOWNLOADS_API ?? DEFAULT_DOWNLOADS_API,
    fetchImpl = fetch,
    concurrency = 12
  } = {}) {
    this.registry = registry.replace(/\/+$/, "");
    this.downloadsApi = downloadsApi.replace(/\/+$/, "");
    this.fetchImpl = fetchImpl;
    this.concurrency = concurrency;
    this.packuments = new Map();
  }

  async packument(name) {
    if (this.packuments.has(name)) return this.packuments.get(name);
    const document = await requestJson(`${this.registry}/${encodePackageName(name)}`, {
      accept: ABBREVIATED,
      fetchImpl: this.fetchImpl,
      allowMissing: true
    });
    this.packuments.set(name, document);
    return document;
  }

  /**
   * Public org listing. Unlike the search endpoint this is authoritative: it
   * returns the org's complete package set rather than a relevance-ranked page
   * that also matches unrelated names.
   */
  async orgPackages(scope) {
    const document = await requestJson(`${this.registry}/-/org/${encodeURIComponent(scope)}/package`, {
      fetchImpl: this.fetchImpl,
      allowMissing: true
    });
    return document ? Object.keys(document).sort() : [];
  }

  async searchAll(text, { pageSize = 250, maxPages = 40 } = {}) {
    const names = new Set();
    let from = 0;
    for (let page = 0; page < maxPages; page++) {
      const url = `${this.registry}/-/v1/search?text=${encodeURIComponent(text)}&size=${pageSize}&from=${from}`;
      const document = await requestJson(url, { fetchImpl: this.fetchImpl, allowMissing: true });
      const objects = document?.objects ?? [];
      for (const entry of objects) if (entry?.package?.name) names.add(entry.package.name);
      if (objects.length < pageSize) break;
      from += objects.length;
      if (typeof document.total === "number" && from >= document.total) break;
    }
    return [...names].sort();
  }

  async downloads(name, period = "last-month") {
    try {
      const document = await requestJson(
        `${this.downloadsApi}/downloads/point/${period}/${encodePackageName(name)}`,
        { fetchImpl: this.fetchImpl, allowMissing: true, attempts: 2 }
      );
      if (!document || typeof document.downloads !== "number") return null;
      return { count: document.downloads, period, start: document.start ?? null, end: document.end ?? null };
    } catch {
      return null;
    }
  }

  async mapConcurrent(items, worker) {
    const results = new Array(items.length);
    let cursor = 0;
    const runners = Array.from({ length: Math.min(this.concurrency, items.length) }, async () => {
      while (cursor < items.length) {
        const index = cursor++;
        results[index] = await worker(items[index], index);
      }
    });
    await Promise.all(runners);
    return results;
  }
}

/** Declared Solid ranges for one published version, runtime and peer alike. */
export function solidRanges(versionDocument, packages) {
  const ranges = {};
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    const declared = versionDocument?.[field];
    if (!declared) continue;
    for (const name of packages) {
      if (typeof declared[name] !== "string") continue;
      ranges[field] ??= {};
      ranges[field][name] = declared[name];
    }
  }
  return ranges;
}

export function flattenRanges(ranges) {
  const flattened = {};
  // A runtime dependency is the stronger statement; a peer range only widens
  // what the package tolerates. The precedence order is fixed here so the
  // result never depends on JSON key ordering.
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies"]) {
    for (const [name, range] of Object.entries(ranges[field] ?? {})) {
      flattened[name] ??= range;
    }
  }
  return flattened;
}
