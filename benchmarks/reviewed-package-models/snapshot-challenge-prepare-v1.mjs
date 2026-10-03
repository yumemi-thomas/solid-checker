// Authenticate the sealed detector, then freeze fresh consumers and real types.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import cases from './snapshot-challenge-cases-v1.mjs';
import { hash, closurePins, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const output = resolve(process.argv[2]), detector = resolve('rust/target/snapshot-v4-detector-freeze.json'), frozen = read(detector);
assert(!existsSync(output)); for (const pin of frozen.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
assert.equal(hash(readFileSync(frozen.baseline.path)), frozen.baseline.sha256);
mkdirSync(output, { recursive: true }); const declarations = new Map(), packages = new Map(), rows = [];
for (const item of cases) {
  const project = resolve('rust/target/app-import-metric/apps', item.app), pkgRoot = packageRoot(project, item.package);
  if (!packages.has(item.package)) packages.set(item.package, { package: item.package, project, root: pkgRoot, pins: closurePins(pkgRoot) });
  const root = join(output, item.id), path = join(root, 'src/main.tsx'); mkdirSync(join(root, 'src'), { recursive: true });
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(path, item.source);
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
    { customConditions: ['browser', 'development'] }), allowJs: true }, root).options,
    program = ts.createProgram([path], options), errors = ts.getPreEmitDiagnostics(program)
      .filter(d => d.category === ts.DiagnosticCategory.Error)
      .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  if (item.provenance.expectedTypingCode) assert(errors.some(error => error.code === item.provenance.expectedTypingCode));
  else assert.deepEqual(errors, [], item.id);
  for (const source of program.getSourceFiles().filter(source => source.isDeclarationFile)) {
    const real = realpathSync(source.fileName), digest = hash(readFileSync(real));
    if (declarations.has(real)) assert.equal(declarations.get(real), digest); else declarations.set(real, digest);
  }
  rows.push({ id: item.id, package: item.package, sourceSha256: hash(item.source), flowSha256: hash(item.flow.toString()),
    provenance: item.provenance, publishedTypingErrors: errors });
}
assert.equal(new Set(rows.map(row => row.id)).size, rows.length);
const report = { authority: false, certification: false, frozenAt: new Date().toISOString(),
  detector: { path: detector, sha256: hash(readFileSync(detector)) }, packages: [...packages.values()], rows,
  files: ['snapshot-challenge-cases-v1.mjs', 'snapshot-challenge-prepare-v1.mjs'].map(name => {
    const path = new URL(name, import.meta.url).pathname; return { path, sha256: hash(readFileSync(path)) };
  }), declarations: [...declarations].map(([path, sha256]) => ({ path, sha256 })) };
writeFileSync(join(output, 'population.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ consumers: rows.length, packages: packages.size, declarations: declarations.size,
  excluded: rows.filter(row => row.publishedTypingErrors.length).map(row => row.id) }));
