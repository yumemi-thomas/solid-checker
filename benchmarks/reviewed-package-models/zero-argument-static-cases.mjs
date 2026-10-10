import assert from 'node:assert/strict';
import { read } from './catalog.mjs';
const path = process.env.REVIEWED_MODEL_ZERO_ARGUMENT_SELECTION; assert(path);
const selection = read(path);
export default selection.rows.map(row => ({ id: `zero-static-${row.package.slice('@solid-primitives/'.length)}-${row.export}`,
  package: row.package, export: row.export, class: 'automatically selected zero-argument ownership observation', rule: 'missing-owner',
  misuse: `import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)}; export const value = candidate();`,
  correct: `import { createRoot } from 'solid-js'; import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)}; export const value = createRoot(() => { const result = candidate(); return result; });` }));
