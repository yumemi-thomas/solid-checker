// Freeze the authored matrix, published typings and detector inputs together.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import cases from './family-matrix-cases.mjs';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const output = resolve(process.argv[2]); assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const repo = resolve(new URL('../..', import.meta.url).pathname), apps = join(repo, 'rust/target/app-import-metric/apps');
const rows = [], pins = new Map(), models = [];
for (const entry of cases) {
  const project = resolve(apps, entry.app), path = join(project, 'src/__family_input.tsx');
  for (const root of nativeRuntimeRoots(project)) assert.equal(read(join(root, 'package.json')).version, '2.0.0-rc.9');
  if (!pins.has(entry.package)) {
    const root = packageRoot(project, entry.package), manifest = read(join(root, 'package.json'));
    pins.set(entry.package, closurePins(root));
    models.push({ package: entry.package, version: manifest.version, pins: pins.get(entry.package), exports: {} });
  }
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), project).options;
  const host = ts.createCompilerHost(options), original = host.getSourceFile;
  host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(file, entry.source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX) : original(file, ...args);
  const errors = ts.getPreEmitDiagnostics(ts.createProgram([path], options, host)).filter(d => d.category === ts.DiagnosticCategory.Error)
    .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  rows.push({ id: entry.id, package: entry.package, project, pins: pins.get(entry.package), sourceSha256: hash(entry.source), provenance: entry.provenance, typingErrors: errors });
}
const files = ['family-matrix-cases.mjs', 'prepare-family-matrix.mjs', 'family-matrix-static.mjs', 'family-matrix-feedback.mjs',
  'source-extractor.mjs', 'lower.mjs', 'demand-models.mjs', 'getter-paths.mjs', 'class-footprints.mjs', 'lifetime-audit.mjs'];
const frozen = files.map(name => { const path = new URL(name, import.meta.url).pathname; return { path, sha256: hash(readFileSync(path)) }; });
const catalogPath = join(output, 'catalog.json');
writeFileSync(catalogPath, JSON.stringify({ format: 'solid-checker-reviewed-model-experiment', authority: false, models }, null, 2) + '\n');
writeFileSync(join(output, 'selection.json'), JSON.stringify({ authority: false, certification: false, rows, frozen,
  catalogPath, catalogSha256: hash(readFileSync(catalogPath)),
  selectionProducer: new URL('./prepare-family-matrix.mjs', import.meta.url).pathname }, null, 2) + '\n');
console.log(JSON.stringify({ cases: rows.length, packages: pins.size, typingRefusals: rows.filter(r => r.typingErrors.length).map(r => ({ id: r.id, errors: r.typingErrors })) }, null, 2));
