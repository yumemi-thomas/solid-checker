import assert from 'node:assert/strict';
import { hash, read } from './catalog.mjs';
const path = process.env.REVIEWED_MODEL_INPUT_SELECTION; assert(path);
export default read(path).rows.map(row => ({ id: `argument-static-${row.package.slice('@solid-primitives/'.length)}-${row.export}-${hash(row.export).slice(7, 15)}`,
  package: row.package, export: row.export, class: 'automatically synthesized ownership observation', rule: 'missing-owner',
  misuse: `import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)}; candidate(${row.arguments.join(', ')});`,
  correct: `import { createRoot } from 'solid-js'; import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)}; createRoot(() => { candidate(${row.arguments.join(', ')}); });` }));
