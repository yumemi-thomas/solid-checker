// Reuse the same typed callback experiment. Full package identity in output
// names avoids collisions among packages outside the Solid-primitives scope.
import cases from './callback-cases.mjs';
import { relative } from 'node:path';
import { hash, read } from './catalog.mjs';
const selection = read(process.env.REVIEWED_MODEL_INPUT_SELECTION);
const apps = new URL('../../rust/target/app-import-metric/apps', import.meta.url).pathname;
export default cases.map(entry => ({ ...entry,
  app: relative(apps, selection.rows.find(row => row.package === entry.package && row.export === entry.provenance.export).project),
  id: `cross-${hash(entry.package).slice(7, 19)}-${hash(entry.provenance.export).slice(7, 19)}-${entry.provenance.role}` }));
