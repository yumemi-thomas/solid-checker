// New consumers for the class source experiment. Typing precedes execution.
const cases = [];
const prelude = `import { flush, untrack } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function add(id, packageName, imports, setup, expression, update, desired, provenance) {
  cases.push({ id: 'class-' + id, package: packageName,
    source: `${prelude}\n${imports}\nfunction App() { ${setup}\nh.update = () => { ${update}; flush(); }; return <p id='value'>{String(${expression})}</p>; }\nh.dispose = render(() => <App />, document.getElementById('root')!);`,
    provenance: { family: 'class-source-footprint', authoredMutation: true, desired, ...provenance },
    flow: async (page, step) => {
      await step('update', () => page.evaluate(() => globalThis.__experiment.update()));
      await page.waitForTimeout(120);
      await page.evaluate(desired => { globalThis.__experiment.values.behavior = { desired: String(desired), actual: document.getElementById('value')?.textContent }; }, desired);
      await step('dispose', () => page.evaluate(() => { globalThis.__experiment.dispose(); globalThis.__experiment.disposals++; }));
    } });
}
for (const item of [
  { id: 'map-get', package: 'map', name: 'ReactiveMap', setup: `const map = new ReactiveMap<string, number>([['key', 0]]);`, read: `map.get('key')`, update: `map.set('key', 1)`, desired: 1 },
  { id: 'map-size', package: 'map', name: 'ReactiveMap', setup: `const map = new ReactiveMap<string, number>([['key', 0]]);`, read: 'map.size', update: `map.set('other', 1)`, desired: 2 },
  { id: 'set-has', package: 'set', name: 'ReactiveSet', setup: `const map = new ReactiveSet<number>();`, read: 'map.has(1)', update: 'map.add(1)', desired: true },
  { id: 'weak-map-get', package: 'map', name: 'ReactiveWeakMap', setup: `const key = {}; const map = new ReactiveWeakMap<object, number>([[key, 0]]);`, read: 'map.get(key)', update: 'map.set(key, 1)', desired: 1 },
  { id: 'weak-set-has', package: 'set', name: 'ReactiveWeakSet', setup: `const key = {}; const map = new ReactiveWeakSet<object>();`, read: 'map.has(key)', update: 'map.add(key)', desired: true },
]) for (const bad of [true, false]) add(item.id + (bad ? '-target' : '-control'), '@solid-primitives/' + item.package,
  `import { ${item.name} } from '@solid-primitives/${item.package}';`, item.setup + (bad ? `const snapshot = ${item.read};` : ''), bad ? 'snapshot' : item.read, item.update, item.desired,
  { expectedIssue: bad, expectedCandidate: bad, role: bad ? 'target' : 'control', pair: item.id });
const imports = `import { ReactiveMap } from '@solid-primitives/map';`, map = `const map = new ReactiveMap<string, number>([['key', 0]]);`;
add('namespace-target', '@solid-primitives/map', `import * as Collection from '@solid-primitives/map';`,
  `const map = new Collection.ReactiveMap<string, number>([['key', 0]]); const snapshot = map.get('key');`, 'snapshot', `map.set('key', 1)`, 1,
  { role: 'target', expectedIssue: true, expectedCandidate: true });
add('computed-target', '@solid-primitives/map', imports, map + `const snapshot = map['get']('key');`, 'snapshot', `map.set('key', 1)`, 1,
  { role: 'target', expectedIssue: true, expectedCandidate: false, gap: 'computed method dispatch' });
add('iterator-consumed-target', '@solid-primitives/map', imports, map + `const snapshot = [...map.keys()].length;`, 'snapshot', `map.set('other', 1)`, 2,
  { role: 'target', expectedIssue: true, expectedCandidate: false, gap: 'iterator consumption effect' });
add('deferred-iterator-control', '@solid-primitives/map', imports, map + `const makeIterator = () => map.keys();`, `[...makeIterator()].length`, `map.set('other', 1)`, 2,
  { role: 'control', expectedIssue: false, expectedCandidate: false });
add('explicit-snapshot-control', '@solid-primitives/map', imports, map + `const snapshot = untrack(() => map.get('key'));`, 'snapshot', `map.set('key', 1)`, 0,
  { role: 'control', expectedIssue: false, expectedCandidate: false, intent: 'explicit untrack snapshot' });
add('implicit-snapshot-control', '@solid-primitives/map', imports, map + `const snapshot = map.get('key');`, 'snapshot', `map.set('key', 1)`, 0,
  { role: 'control', expectedIssue: false, expectedCandidate: true, intent: 'externally declared intentional snapshot' });
add('method-replaced-control', '@solid-primitives/map', imports, map + `map.get = () => 99; const snapshot = map.get('key');`, 'snapshot', `map.set('key', 1)`, 99,
  { role: 'control', expectedIssue: false, expectedCandidate: false });
add('escaped-instance-target', '@solid-primitives/map', imports, map + `const alias = map; const snapshot = map.get('key');`, 'snapshot', `alias.set('key', 1)`, 1,
  { role: 'target', expectedIssue: true, expectedCandidate: false, gap: 'instance escape' });
add('shadowed-import-control', '@solid-primitives/map', imports, `const ReactiveMap = Map; const map = new ReactiveMap<string, number>([['key', 0]]); const snapshot = map.get('key');`, 'snapshot', `map.set('key', 1)`, 0,
  { role: 'control', expectedIssue: false, expectedCandidate: false });
add('stored-event-control', '@solid-primitives/map', imports, map + `const read = () => map.get('key');`, 'read()', `map.set('key', 1)`, 1,
  { role: 'control', expectedIssue: false, expectedCandidate: false });
export default cases;
