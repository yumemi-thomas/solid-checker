import assert from "node:assert/strict";
import { AsyncLocalStorage } from "node:async_hooks";
import { createHash } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { isAbsolute, relative, sep } from "node:path";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const handler = hook => typeof hook === "function" ? hook : hook?.handler;
const wrapHook = (hook, wrapped) => typeof hook === "function" ? wrapped : { ...hook, handler: wrapped };
const inside = (root, path) => { const part = relative(root, path); return part !== ".." && !part.startsWith(".." + sep) && !isAbsolute(part); };

// Observe the application's middleware and virtual-module loaders. An absent
// physical index is admitted only after one of these exact config plugins
// serves HTML, and that HTML names a successfully resolved application entry.
// This creates no HTML or mount code. Hook/served-output digests supplement the
// on-disk config dependency pins, including generators living in packages.
export async function observeFeedbackEntry(plugins = []) {
  const scope = new AsyncLocalStorage(), html = [], modules = new Map(), generators = [];
  const pin = (plugin, hook, fn) => {
    const input = { plugin, hook, sha256: hash(Function.prototype.toString.call(fn)) };
    generators.push(input); return input;
  };
  const flatten = async entries => {
    const result = [];
    for (const entry of await entries ?? []) {
      const plugin = await entry;
      if (!plugin) continue;
      if (Array.isArray(plugin)) { result.push(...await flatten(plugin)); continue; }
      const wrapped = { ...plugin };
      const load = handler(plugin.load);
      if (load) {
        const generator = pin(plugin.name, "load", load);
        wrapped.load = wrapHook(plugin.load, async function(id, ...args) {
          const result = await load.call(this, id, ...args);
          const code = typeof result === "string" ? result : result?.code;
          if (typeof code === "string") modules.set(id, { id, sha256: hash(code), generator });
          return result;
        });
      }
      const configure = handler(plugin.configureServer);
      if (configure) {
        const generator = pin(plugin.name, "configureServer", configure);
        const install = async (server, hook, receiver) => {
          const use = server.middlewares.use;
          server.middlewares.use = function(...args) {
            const middleware = args.at(-1);
            if (typeof middleware === "function") {
              const input = pin(plugin.name, "middleware", middleware);
              // Connect distinguishes error middleware by its arity.
              args[args.length - 1] = middleware.length === 4
                ? function(error, req, res, next) { return scope.run(input, () => middleware.call(this, error, req, res, (...args) => scope.exit(() => next(...args)))); }
                : function(req, res, next) { return scope.run(input, () => middleware.call(this, req, res, (...args) => scope.exit(() => next(...args)))); };
            }
            return use.apply(this, args);
          };
          try { return await scope.run(generator, () => hook.call(receiver, server)); }
          finally { server.middlewares.use = use; }
        };
        wrapped.configureServer = wrapHook(plugin.configureServer, async function(server) {
          const post = await install(server, configure, this);
          if (post) return () => install(server, post, this);
        });
      }
      result.push(wrapped);
    }
    return result;
  };
  return {
    plugins: await flatten(plugins), html, modules, generators,
    observer: { name: "solid-checker-feedback-entry-provenance", enforce: "pre", configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const chunks = [], write = res.write, end = res.end;
        let bytes = 0;
        const collect = chunk => {
          if (chunk === undefined || chunk === null || typeof chunk === "function") return;
          const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
          bytes += data.length;
          if (bytes <= 1024 * 1024) chunks.push(data);
        };
        res.write = function(chunk, ...args) { collect(chunk); return write.call(this, chunk, ...args); };
        res.end = function(chunk, ...args) {
          collect(chunk);
          const generator = scope.getStore();
          if (generator && String(res.getHeader("content-type")).includes("text/html")) {
            if (html.length < 64) html.push({ path: req.originalUrl ?? req.url, status: res.statusCode,
              sha256: bytes <= 1024 * 1024 ? hash(Buffer.concat(chunks)) : null, bytes, generator });
          }
          return end.call(this, chunk, ...args);
        };
        // Passing to Vite's own fallback is not application provenance, even
        // when next() was called inside an app middleware's async context.
        scope.exit(next);
      });
    } }
  };
}

export async function admitFeedbackEntry({ response, scripts, origin, appTree, server, provenance, record }) {
  assert(response && response.status() >= 200 && response.status() < 300, "Application HTML entry did not return success");
  assert((response.headers()["content-type"] ?? "").includes("text/html"), "Application entry did not serve HTML");
  const url = new URL(response.url()), body = await response.body();
  const html = provenance.html.findLast(row => row.path === url.pathname + url.search && row.sha256 === hash(body) && row.status === response.status());
  assert(html, "Generated HTML was not served by an application config middleware (or exceeded 1 MiB)");
  const entries = [];
  for (const src of scripts) {
    const entry = new URL(src, response.url());
    if (entry.origin !== origin || entry.pathname === "/@vite/client" || entry.pathname === "/@id/virtual:solid-checker-feedback-entry") continue;
    const request = entry.pathname + entry.search;
    // Vite's HTTP transform middleware unwraps /@id/ before calling this API.
    // Reproduce that transport decoding for generated virtual entry URLs.
    const moduleUrl = entry.pathname.startsWith("/@id/")
      ? decodeURI(entry.pathname.slice("/@id/".length)).replace("__x00__", "\0") + entry.search : request;
    const transformed = await server.transformRequest(moduleUrl);
    if (!transformed) continue;
    const module = await server.moduleGraph.getModuleByUrl(moduleUrl), id = module?.id;
    const generated = provenance.modules.get(id);
    if (generated) entries.push({ url: request, ...generated });
    else if (module?.file && existsSync(module.file) && inside(appTree, realpathSync(module.file))) {
      record(module.file); entries.push({ url: request, path: realpathSync(module.file) });
    }
  }
  assert(entries.length, "Application-generated HTML has no proven application module entry");
  return { kind: "application-served-html", html, entries, generators: provenance.generators };
}
