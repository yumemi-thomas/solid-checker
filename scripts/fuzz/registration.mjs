// Where a callback was registered. Solid reports a diagnostic with the stack of the moment it runs: for a callback handed to
// `onSettled`, `createTrackedEffect` or another primitive that runs it later, that stack holds the lines inside the callback and
// never the line of the registering call. The checker answers on the registering call (`onSettled(cb1)`), so a finding there has to
// be read as covering a diagnostic whose stack runs through `cb1`, or the comparator calls every such program a false clean.
//
// The link is read from the program tree, never from names in the text: a stack line inside a function declared as `const N = ...`
// (`decl`) belongs to the primitives of the program that take exactly `N` as their callback (`fn: ref N`, `apply: ref N`).
// Only a name declared once is followed, and only a direct registration (a callback handed on through a helper is not).
import { walk } from "./model.mjs";
import { nodesAtLine } from "./print.mjs";

const REGISTERING = new Set(["prim", "own"]);
const CALLBACK_SLOTS = ["fn", "apply"];
const same = (a, b) => a.length === b.length && a.every((key, index) => key === b[index]);

/**
 * @returns {(line: number) => number[]} for a stack line, the lines of the calls that register a function the line is inside of.
 */
export function registrationLines(program, lineTable) {
  const declared = new Map();
  const registered = new Map();
  for (const { node, path } of walk(program)) {
    if (node.k === "decl" && node.e?.k === "fn") declared.set(node.name, (declared.get(node.name) ?? 0) + 1);
    if (!REGISTERING.has(node.k)) continue;
    for (const slot of CALLBACK_SLOTS) if (node[slot]?.k === "ref") registered.set(node[slot].name, [...(registered.get(node[slot].name) ?? []), path]);
  }
  return (line) => {
    const lines = new Set();
    for (const row of nodesAtLine(lineTable, line)) {
      let node = program;
      for (const key of row.path) {
        node = node?.[key];
        if (node?.k !== "decl" || node.e?.k !== "fn" || declared.get(node.name) !== 1) continue;
        for (const path of registered.get(node.name) ?? []) {
          const at = lineTable.find((candidate) => same(candidate.path, path));
          if (at) lines.add(at.line);
        }
      }
    }
    return [...lines];
  };
}
