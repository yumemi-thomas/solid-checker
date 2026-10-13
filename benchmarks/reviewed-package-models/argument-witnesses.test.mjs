import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ts } from './lower.mjs';
import { seeds, synthesize } from './argument-witnesses.mjs';
test('structural witnesses preserve published callback and DOM requirements without assertions', () => {
  const dir = mkdtempSync(join(tmpdir(), 'solid-argument-types-')), path = join(dir, 'input.ts');
  const declarations = `declare function candidate(value: { target: HTMLElement; callback: (event: Event) => boolean; values: readonly number[]; enabled?: boolean }): void;
declare class Nominal { private secret: number; }
declare function opaque(value: Nominal): void;`;
  writeFileSync(path, declarations + '\n' + seeds.map((seed, i) => `const seed${i} = ${seed};`).join('\n'));
  const options = { strict: true, noEmit: true, target: ts.ScriptTarget.ESNext, skipLibCheck: true }, program = ts.createProgram([path], options), source = program.getSourceFile(path), checker = program.getTypeChecker();
  const seedTypes = source.statements.slice(3).map(s => checker.getTypeAtLocation(s.declarationList.declarations[0].name));
  const parameter = source.statements[0].parameters[0], type = checker.getTypeAtLocation(parameter), values = [0, 1, 2].map(profile => synthesize(checker, type, parameter, seedTypes, profile));
  assert(values.every(v => v !== null && !v.includes(' as ')));
  writeFileSync(path, declarations + '\n' + values.map(value => `candidate(${value});`).join('\n'));
  assert.equal(ts.getPreEmitDiagnostics(ts.createProgram([path], options)).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const nominal = source.statements[2].parameters[0]; assert.equal(synthesize(checker, checker.getTypeAtLocation(nominal), nominal, seedTypes), null);
});
