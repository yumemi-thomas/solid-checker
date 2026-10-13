// New consumers evaluated with the existing extractor frozen. Packages were
// present in the generic census; this is not an independently blinded study.
const app = body => `export default function App() { ${body} }`;
const pair = (id, pkg, name, rule, imports, misuse, correct) => ({ id: `holdout-${id}`, package: `@solid-primitives/${pkg}`, export: name, rule,
  misuse: imports + "\n" + misuse, correct: imports + "\n" + correct });
export default [
  pair("listener-owner", "event-listener", "createEventListener", "missing-owner", 'import { createEventListener } from "@solid-primitives/event-listener";',
    `createEventListener(new EventTarget(), "click", () => {}); ${app('return <p>listener</p>;')}`,
    app('createEventListener(new EventTarget(), "click", () => {}); return <p>listener</p>;')),
  pair("event-accessor", "event-listener", "createEventSignal", "strict-read-untracked", 'import { createEventSignal } from "@solid-primitives/event-listener";',
    app('const event = createEventSignal(new EventTarget(), "click"); const value = event(); return <p>{String(value)}</p>;'),
    app('const event = createEventSignal(new EventTarget(), "click"); return <p>{String(event())}</p>;')),
  pair("keyboard-owner", "keyboard", "createKeyDown", "missing-owner", 'import { createKeyDown } from "@solid-primitives/keyboard";',
    `createKeyDown("Escape", () => {}); ${app('return <p>keyboard</p>;')}`,
    app('createKeyDown("Escape", () => {}); return <p>keyboard</p>;')),
  pair("set-union", "set", "union", "strict-read-untracked", 'import { union } from "@solid-primitives/set";',
    app('const read = union(new Set([1]), new Set([2])); const value = read(); return <p>{value.size}</p>;'),
    app('const read = union(new Set([1]), new Set([2])); return <p>{read().size}</p>;')),
  pair("set-intersection", "set", "intersection", "strict-read-untracked", 'import { intersection } from "@solid-primitives/set";',
    app('const read = intersection(new Set([1]), new Set([1])); const value = read(); return <p>{value.size}</p>;'),
    app('const read = intersection(new Set([1]), new Set([1])); return <p>{read().size}</p>;')),
  pair("trigger-read", "trigger", "createTrigger", "strict-read-untracked", 'import { createTrigger } from "@solid-primitives/trigger";',
    app('const [track] = createTrigger(); track(); return <p>trigger</p>;'),
    app('const [track] = createTrigger(); return <p>{String(track())}</p>;')),
  { id: "holdout-explicit-listener-disposal", package: "@solid-primitives/event-listener", export: "makeEventListener", rule: "missing-owner", expectWarning: false,
    misuse: 'import { makeEventListener } from "@solid-primitives/event-listener"; const stop = makeEventListener(new EventTarget(), "click", () => {}); stop(); ' + app('return <p>explicit disposal</p>;') },
  { id: "holdout-global-event-bus", package: "@solid-primitives/event-bus", export: "createEventBus", rule: "missing-owner", expectWarning: false,
    misuse: 'import { createEventBus } from "@solid-primitives/event-bus"; const bus = createEventBus<string>(); const stop = bus.listen(() => {}); bus.emit("value"); stop(); bus.clear(); ' + app('return <p>global bus</p>;') },
  { id: "holdout-untracked-trigger", package: "@solid-primitives/trigger", export: "createTrigger", rule: "strict-read-untracked", expectWarning: false,
    misuse: 'import { untrack } from "solid-js"; import { createTrigger } from "@solid-primitives/trigger"; ' + app('const [track] = createTrigger(); untrack(track); return <p>explicit snapshot</p>;') },
  { id: "holdout-readonly-set", package: "@solid-primitives/set", export: "readonlySet", rule: "strict-read-untracked", expectWarning: false,
    misuse: 'import { ReactiveSet, readonlySet } from "@solid-primitives/set"; ' + app('const value = readonlySet(new ReactiveSet([1])); const snapshot = value.size; return <p>{snapshot}</p>;') },
];
export const typeRejected = [
  { id: "holdout-invalid-handler", package: "@solid-primitives/event-listener", source: 'import { createEventListener } from "@solid-primitives/event-listener"; createEventListener(new EventTarget(), "click", 42);' },
  { id: "holdout-invalid-key", package: "@solid-primitives/keyboard", source: 'import { createKeyDown } from "@solid-primitives/keyboard"; createKeyDown(42, () => {});' },
];
