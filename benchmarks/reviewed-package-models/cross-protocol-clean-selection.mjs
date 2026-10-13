// Drive returned validators only after their construction control succeeds.
// Prefer the shared result-returning interface when its public types expose it.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [protocolArg, validationArg, outputArg] = process.argv.slice(2);
const protocolPath = resolve(protocolArg), validationPath = resolve(validationArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(protocolPath), validation = read(validationPath);
const clean = new Set(validation.results.filter(r => r.executed && r.controlClean).map(r => r.package + '\0' + r.export));
const rows = selection.rows.filter(r => clean.has(r.package + '\0' + r.export)).map(row => ({ ...row,
  protocol: row.availableProtocols.find(p => p.members.length === 2) ?? row.protocol }));
assert(rows.length);
writeFileSync(output, JSON.stringify({ ...selection, rows, protocolDiscoveryPath: protocolPath,
  protocolDiscoverySha256: hash(readFileSync(protocolPath)), historyPath: validationPath,
  historySha256: hash(readFileSync(validationPath)), summary: { exportsWithValidationFlow: rows.length,
    packagesWithValidationFlow: new Set(rows.map(r => r.package)).size,
    refusedConstructionControls: selection.rows.length - rows.length } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
