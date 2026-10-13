// Two explicit shared validation flows. Public member types admit a flow;
// their names do not establish its behavior or authorize a static finding.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [selectionArg, outputArg] = process.argv.slice(2), selectionPath = resolve(selectionArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), rows = [], observations = [];
for (const row of selection.rows) {
  const path = row.project + '/src/__validation_protocol.ts';
  const code = `import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)};
const result = candidate(${row.arguments.join(', ')});`;
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), dirname(path)).options;
  const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
  host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true) : get(file, ...args);
  host.fileExists = file => file === path || exists(file);
  const program = ts.createProgram([path], options, host), source = program.getSourceFile(path), checker = program.getTypeChecker();
  const location = source.statements[1].declarationList.declarations[0].name, resultType = checker.getTypeAtLocation(location);
  const protocols = [];
  for (const members of [['parse'], ['~standard', 'validate']]) {
    let type = resultType, property;
    for (const member of members) { property = checker.getPropertyOfType(type, member); if (!property) break; type = checker.getTypeOfSymbolAtLocation(property, location); }
    if (!property) continue;
    const signatures = type.getCallSignatures().filter(s => s.minArgumentCount <= 1 && s.getParameters().length >= 1);
    if (!signatures.length) continue;
    protocols.push({ members, arguments: ['1'], declarations: property.declarations?.map(d => ({ path: d.getSourceFile().fileName,
      start: d.getStart(), sha256: hash(d.getSourceFile().text) })) ?? [], signature: checker.signatureToString(signatures[0]) });
  }
  if (protocols.length) rows.push({ ...row, protocol: protocols[0], availableProtocols: protocols });
  observations.push({ package: row.package, export: row.export, admittedProtocols: protocols.map(p => p.members) });
}
const summary = { exportsConsidered: selection.rows.length, exportsWithValidationFlow: rows.length,
  packagesWithValidationFlow: new Set(rows.map(r => r.package)).size };
writeFileSync(output, JSON.stringify({ ...selection, rows, summary, parentSelectionPath: selectionPath,
  parentSelectionSha256: hash(readFileSync(selectionPath)), observations,
  protocolProducer: new URL('./cross-protocol-selection.mjs', import.meta.url).pathname }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
