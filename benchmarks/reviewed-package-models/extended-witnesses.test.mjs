import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ts } from './lower.mjs';
import { extendedWitness, standardSeeds } from './extended-witnesses.mjs';
test('binary callback witnesses use the actual numeric argument and keep nominal refusals', () => {
  const path = join(mkdtempSync(join(tmpdir(), 'binary-witness-')), 'input.ts');
  const declarations = 'declare function binary(fn: (size: number) => Uint8Array): void;\n' +
    'declare class Nominal { private secret: number; }\n' +
    'declare function opaque(value: Nominal): void;';
  writeFileSync(path, declarations + '\n' + standardSeeds.map((seed, i) => `const seed${i} = ${seed};`).join('\n'));
  const options = { strict: true, noEmit: true, target: ts.ScriptTarget.ESNext, skipLibCheck: true };
  const program = ts.createProgram([path], options), source = program.getSourceFile(path), checker = program.getTypeChecker();
  const seedTypes = source.statements.slice(3).map(s => checker.getTypeAtLocation(s.declarationList.declarations[0].name));
  const parameter = source.statements[0].parameters[0], value = extendedWitness(checker, checker.getTypeAtLocation(parameter), parameter, seedTypes);
  assert.equal(value, 'arg0 => new Uint8Array(arg0)');
  const allocate = Function(`return (${value});`)(); assert.equal(allocate(7).length, 7);
  writeFileSync(path, declarations + `\nbinary(${value});`);
  assert.equal(ts.getPreEmitDiagnostics(ts.createProgram([path], options)).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const nominal = source.statements[2].parameters[0]; assert.equal(extendedWitness(checker, checker.getTypeAtLocation(nominal), nominal, seedTypes), null);
});
