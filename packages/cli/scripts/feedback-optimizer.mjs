import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

export const feedbackRuntimePackages = ["solid-js", "@solidjs/signals", "@solidjs/web"];
const runtimeImport = id => { const part = id.split(">").at(-1).trim(); return feedbackRuntimePackages.some(pkg => part === pkg || part.startsWith(pkg + "/")); };
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const inside = (tree, path) => { const part = relative(tree, path); return part !== ".." && !part.startsWith(".." + sep) && !isAbsolute(part); };

export function configureFeedbackOptimizer(config, { record, sources } = {}) {
  const configured = new Set();
  for (const [environment, options] of [["client", config.optimizeDeps], ...Object.entries(config.environments ?? {}).map(([name, row]) => [name, row.optimizeDeps])]) {
    if (!options || configured.has(options)) continue;
    configured.add(options);
    // Vite externalizes excluded bare imports in included dependency bundles.
    // Keep useful CJS includes, including nested non-runtime dependencies.
    options.include = (options.include ?? []).filter(id => !runtimeImport(id));
    options.noDiscovery = false;
    options.exclude = [...new Set([...(options.exclude ?? []), ...feedbackRuntimePackages])];
    if (record && sources) {
      const observe = (id, code) => {
        const path = id.split("?")[0];
        if (existsSync(path)) {
          const canonical = realpathSync(path);
          assert(!runtimeFile(canonical, record), `Solid runtime reached dependency optimization: ${canonical}`);
          record(canonical);
          const row = { environment, id, path: canonical, sha256: hash(readFileSync(canonical)), ...(code !== undefined ? { transformedSha256: hash(code) } : {}) };
          sources.set(JSON.stringify(row), row);
        } else {
          const row = { environment, id, ...(code !== undefined ? { transformedSha256: hash(code) } : { sourceUnavailable: true }) };
          sources.set(JSON.stringify(row), row);
        }
      };
      options.rolldownOptions ??= {};
      options.rolldownOptions.plugins = [{ name: "solid-checker-optimizer-inputs", transform(code, id) { observe(id, code); } }, options.rolldownOptions.plugins ?? []];
      options.esbuildOptions ??= {};
      options.esbuildOptions.plugins = [{ name: "solid-checker-optimizer-inputs", setup(build) {
        build.onLoad({ filter: /.*/ }, args => { observe(args.path); return undefined; });
      } }, ...(options.esbuildOptions.plugins ?? [])];
    }
  }
}

export function feedbackRuntimeArtifact(path, record) {
  let directory = dirname(path);
  while (dirname(directory) !== directory) {
    const metadata = join(directory, "package.json");
    if (existsSync(metadata)) {
      const name = JSON.parse(readFileSync(metadata, "utf8")).name;
      if (typeof name === "string") record?.(metadata);
      return feedbackRuntimePackages.includes(name) ? { name, root: realpathSync(directory) } : null;
    }
    directory = dirname(directory);
  }
  return null;
}
const runtimeFile = (path, record) => Boolean(feedbackRuntimeArtifact(path, record));

export async function resolveFeedbackRuntime(id, importer, options, resolveId, instances, record) {
  const resolved = await resolveId(id, importer, { ...options, skipSelf: true });
  if (!resolved || resolved.external) return resolved;
  const [path, query = ""] = resolved.id.split("?");
  if (!isAbsolute(path) || !existsSync(path)) return resolved;
  const canonical = realpathSync(path), artifact = feedbackRuntimeArtifact(canonical, record);
  if (!artifact) return resolved;
  const previous = instances.get(artifact.name);
  assert(!previous || previous === artifact.root, `Multiple installed Solid runtime instances resolved: ${artifact.name}`);
  instances.set(artifact.name, artifact.root);
  // Vite versions even excluded node_modules imports with browserHash. A
  // newly discovered dependency changes that hash without necessarily
  // reloading, which would instantiate a second copy of the excluded runtime.
  const parameters = query.split("&").filter(part => part && !new URLSearchParams(part).has("v")).join("&");
  return { ...resolved, id: canonical + (parameters ? "?" + parameters : "") };
}

// Optimizer artifacts are ephemeral; validate them before cache teardown and
// retain their pins in the report rather than replaying deleted files later.
export function captureFeedbackOptimizer(cache, record, observed = new Map(), sources = new Map(), environment = "client",
  directory = join(cache, environment === "client" ? "deps" : "deps_" + environment)) {
  const metadataPath = join(directory, "_metadata.json");
  if (!existsSync(metadataPath)) {
    assert.equal(observed.size, 0, "Optimized dependency bytes were served without optimizer metadata");
    return { metadata: null, dependencies: [], chunks: [], inputs: [], sources: [] };
  }
  const inputs = new Map(), pin = path => {
    path = realpathSync(path);
    assert(inside(realpathSync(cache), path), "Optimizer artifact escaped this run's cache");
    const bytes = readFileSync(path), sha256 = hash(bytes);
    assert(!observed.has(path) || observed.get(path).sha256 === sha256, `Served optimized dependency changed: ${path}`);
    inputs.set(path, { path, sha256 }); return bytes;
  };
  const bytes = pin(metadataPath), metadata = JSON.parse(bytes);
  for (const field of ["hash", "lockfileHash", "configHash", "browserHash"]) assert(typeof metadata[field] === "string", `Optimizer metadata lacks ${field}`);
  const rows = field => Object.entries(metadata[field] ?? {}).sort(([a], [b]) => a.localeCompare(b)).map(([id, row]) => {
    assert(!runtimeImport(id), `Solid runtime was optimized: ${id}`);
    const file = resolve(dirname(metadataPath), row.file);
    pin(file);
    // Vite's optimizer emits maps with the bundled source list. An excluded
    // runtime imported by an included package must remain an external import,
    // never a bundled source (including aliased/relative runtime paths).
    let map;
    if (existsSync(file + ".map")) map = JSON.parse(pin(file + ".map"));
    else {
      // Rolldown omits maps for facades and synthetic built-in proxies. The
      // optimizer's input hook still guards their whole dependency graph.
      assert(sources.size, `Optimized dependency has no source inventory: ${file}`);
      map = { sources: [] };
    }
    for (const [index, source] of map.sources.entries()) {
      const path = resolve(dirname(file), map.sourceRoot ?? "", source);
      if (existsSync(path)) {
        assert(!runtimeFile(realpathSync(path), record), `Solid runtime was inlined into an optimized dependency: ${path}`);
        assert.equal(map.sourcesContent?.[index], readFileSync(path, "utf8"), `Optimized dependency source changed or was substituted: ${path}`);
        record(path);
      }
    }
    const src = row.src ? resolve(dirname(metadataPath), row.src) : null;
    if (src) { assert(!runtimeFile(realpathSync(src), record), `Solid runtime was optimized: ${src}`); record(src); }
    return { id, ...row, file, ...(src ? { src } : {}) };
  });
  const dependencies = rows("optimized"), chunks = rows("chunks");
  for (const pin of observed.values()) assert(inputs.has(pin.path), `Served optimizer output is absent from metadata: ${pin.path}`);
  return { metadata: { path: realpathSync(metadataPath), sha256: hash(bytes), hash: metadata.hash,
    lockfileHash: metadata.lockfileHash, configHash: metadata.configHash, browserHash: metadata.browserHash },
    dependencies, chunks, inputs: [...inputs.values()], sources: [...sources.values()].sort((a, b) => a.id.localeCompare(b.id)) };
}

export function createFeedbackOptimizerObserver(cache, record, sources = new Map()) {
  const generations = new Map();
  const directories = new Map([["client", join(cache, "deps")]]);
  const capture = environment => {
    const directory = directories.get(environment);
    assert(directory, "Optimized dependency is outside a configured environment");
    const metadata = join(directory, "_metadata.json");
    assert(existsSync(metadata), "Optimized dependency bytes were served without optimizer metadata");
    const key = environment + ":" + hash(readFileSync(metadata));
    let generation = generations.get(key);
    if (!generation) {
      generation = { environment, ...captureFeedbackOptimizer(cache, record, new Map(),
        new Map([...sources].filter(([, row]) => row.environment === environment)), environment, directory), transforms: [] };
      generations.set(key, generation);
    }
    return generation;
  };
  return {
    configure(config) {
      directories.clear();
      // getDepsCacheDir is private in Vite 8: its exact committed directory is
      // resolved cacheDir + deps + the configured environment's suffix.
      for (const [name, options] of Object.entries(config.environments ?? { client: config })) {
        const cacheDir = resolve(options.cacheDir ?? config.cacheDir), suffix = name === "client" ? "deps" : "deps_" + name;
        assert(inside(realpathSync(cache), join(realpathSync(cacheDir), suffix)), "Optimizer environment cache escaped this run's cache");
        // Metadata's relative paths use Vite's lexical cache path. Resolving
        // them against its canonical alias can change the meaning of `../`.
        const directory = join(cacheDir, suffix);
        directories.set(name, directory);
      }
    },
    observe(code, id) {
      const path = id.split("?")[0];
      if (!existsSync(path)) return false;
      const canonical = realpathSync(path);
      if (!inside(realpathSync(cache), canonical)) return false;
      const environment = [...directories].find(([, directory]) => existsSync(directory) && inside(realpathSync(directory), canonical))?.[0];
      assert(environment, "Optimized dependency is outside a configured committed environment");
      const generation = capture(environment);
      const pin = generation.inputs.find(row => row.path === canonical);
      assert(pin, `Served optimizer output is absent from metadata: ${canonical}`);
      assert.equal(hash(readFileSync(canonical)), pin.sha256, `Served optimized dependency changed within its generation: ${canonical}`);
      assert.equal(hash(code), pin.sha256, `A previous transform changed optimized dependency bytes: ${canonical}`);
      if (!generation.transforms.some(row => row.id === id)) generation.transforms.push({ id, ...pin });
      return true;
    },
    finish() {
      // Earlier generations were pinned when loaded; Vite may have replaced
      // their files during legitimate reoptimization. Validate the final ones
      // before teardown, without misidentifying old bytes as the new output.
      for (const [environment, directory] of directories) {
        if (!existsSync(join(directory, "_metadata.json"))) continue;
        const generation = capture(environment);
        captureFeedbackOptimizer(cache, record, new Map(generation.inputs.map(pin => [pin.path, pin])),
          new Map([...sources].filter(([, row]) => row.environment === environment)), environment, directory);
      }
      return { generations: [...generations.values()] };
    }
  };
}
