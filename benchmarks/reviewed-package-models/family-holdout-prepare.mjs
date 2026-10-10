// Freeze consumers and the actual declarations before browser execution.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import cases from './family-holdout-cases.mjs';
import { hash, packageDigest, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [outputArg] = process.argv.slice(2), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const detector = resolve('rust/target/family-holdout-detector-freeze.json'), frozen = read(detector);
assert(new Date(frozen.frozenAt) < new Date());
for (const pin of frozen.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const declarations = new Map(), rows = [];
for (const item of cases) {
  const pkg = frozen.packages.find(pkg => pkg.package === item.package); assert(pkg);
  const root = join(output, item.id), path = join(root, 'src/main.tsx'); mkdirSync(join(root, 'src'), { recursive: true });
  symlinkSync(join(pkg.project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(path, item.source);
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
    { customConditions: ['browser', 'development'] }), allowJs: true }, root).options;
  const program = ts.createProgram([path], options), errors = ts.getPreEmitDiagnostics(program)
    .filter(d => d.category === ts.DiagnosticCategory.Error)
    .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  for (const source of program.getSourceFiles().filter(source => source.isDeclarationFile)) {
    const real = realpathSync(source.fileName), digest = hash(readFileSync(real));
    if (declarations.has(real)) assert.equal(declarations.get(real), digest); else declarations.set(real, digest);
  }
  rows.push({ id: item.id, package: item.package, sourceSha256: hash(item.source),
    flowSha256: hash(item.flow.toString()), provenance: item.provenance, publishedTypingErrors: errors });
}
assert.equal(new Set(rows.map(row => row.id)).size, rows.length);
const tooling = resolve('rust/target/app-import-metric/apps/helge-dev');
const report = { authority: false, certification: false, frozenAt: new Date().toISOString(),
  detector: { path: detector, sha256: hash(readFileSync(detector)) }, rows,
  files: ['family-holdout-cases.mjs', 'family-holdout-prepare.mjs'].map(name => {
    const path = new URL(name, import.meta.url).pathname; return { path, sha256: hash(readFileSync(path)) };
  }), declarations: [...declarations].map(([path, sha256]) => ({ path, sha256 })),
  tooling: ['typescript', 'vite', '@solidjs/vite-plugin', 'playwright'].map(name => {
    const root = packageRoot(tooling, name); return { package: name, root, digest: packageDigest(root) };
  }) };
writeFileSync(join(output, 'population.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ consumers: rows.length, packages: new Set(rows.map(row => row.package)).size,
  declarations: declarations.size, typingErrors: rows.filter(row => row.publishedTypingErrors.length)
    .map(row => ({ id: row.id, errors: row.publishedTypingErrors })) }, null, 2));
