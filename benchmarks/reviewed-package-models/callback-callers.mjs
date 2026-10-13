// Classify only the immediate, source-mapped caller of an exactly identified
// application callback write. Missing frames and missing source bytes stay open.
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import { join } from 'node:path';
import { callbackWriteAt } from './callback-sites.mjs';
import { hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';

export function callerRoots(project, name) {
  const roots = new Map();
  function visit(root) {
    root = realpathSync(root); if (roots.has(root)) return;
    const manifest = read(join(root, 'package.json'));
    roots.set(root, { package: manifest.name, version: manifest.version });
    for (const dependency of Object.keys({ ...manifest.dependencies, ...manifest.peerDependencies })) {
      try { visit(packageRoot(root, dependency)); }
      catch (error) {
        if (!manifest.peerDependenciesMeta?.[dependency]?.optional && !manifest.optionalDependencies?.[dependency]) throw error;
      }
    }
  }
  visit(packageRoot(project, name));
  for (const root of nativeRuntimeRoots(project)) visit(root);
  return roots;
}

export function callbackCaller(diagnostic, consumer, sites, roots) {
  const target = callbackWriteAt(diagnostic, consumer, sites);
  if (!target) return { kind: 'unmapped-write' };
  const frames = diagnostic.originalFrames ?? [];
  const index = frames.findIndex(frame => frame && callbackWriteAt({ originalLocation: frame }, consumer, sites)?.start === target.start);
  if (index < 0) return { kind: 'unknown', reason: 'Callback frame unavailable', id: target.id };
  let caller = frames[index + 1], callerFileBasis = 'source-mapped';
  if (!caller) {
    // Vite's unbundled filesystem URL identifies the served input module even
    // when that module has no source map. It supplies no original source span.
    // Never derive package identity from an optimized bundle's file name.
    const raw = diagnostic.frames?.[index + 1];
    if (raw) try {
      const url = new URL(raw.path);
      const application = new URL(diagnostic.frames[index].path);
      if (url.origin === application.origin && url.pathname.startsWith('/@fs/') && [...url.searchParams.keys()].every(k => ['v', 'import', 't'].includes(k))) {
        caller = { path: decodeURIComponent(url.pathname.slice('/@fs'.length)), servedLocation: raw };
        callerFileBasis = 'served-input-module';
      }
    } catch { /* Missing or opaque immediate caller stays open. */ }
  }
  if (!caller) return { kind: 'unknown', reason: 'Immediate caller frame unavailable', id: target.id };
  if (!existsSync(caller.path)) return { kind: 'unknown', reason: 'Caller source bytes unavailable', id: target.id, caller };
  const path = realpathSync(caller.path);
  if (path === realpathSync(consumer)) return { kind: 'application', id: target.id, caller, callerFileBasis };
  // The nearest physical package root owns the bytes, including when roots nest.
  const owned = [...roots].sort(([a], [b]) => b.length - a.length).find(([root]) => path.startsWith(root + '/'));
  if (!owned) return { kind: 'unknown', reason: 'Caller outside the authenticated package/runtime graph', id: target.id, caller };
  const [, identity] = owned, native = ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(identity.package);
  return { kind: native ? 'solid-runtime' : 'dependency', id: target.id, package: identity.package, version: identity.version,
    caller: { ...caller, path }, callerFileBasis, sourceSha256: hash(readFileSync(path)) };
}
