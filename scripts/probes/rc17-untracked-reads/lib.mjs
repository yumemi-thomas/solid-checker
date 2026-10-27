// Shared helpers for the three probes. They record which files were resolved,
// capture dev diagnostics on both channels, and read subscriber edges. They do
// not change reactive behaviour.
import { DEV, OBSERVE } from "solid-js";
import { $REFRESH } from "@solidjs/signals";

export const resolved = {
  "solid-js": import.meta.resolve("solid-js"),
  "@solidjs/signals": import.meta.resolve("@solidjs/signals"),
  "@solidjs/web": import.meta.resolve("@solidjs/web"),
};

// Structured channel: every emitDiagnostic call (OBSERVE.diagnostics, dev-shared.js).
const structured = [];
OBSERVE.diagnostics.subscribe((entry) => {
  structured.push(`${entry.severity} ${entry.code}: ${entry.message}`);
});

// Console channel: what a developer sees (reportDiagnostic → console.warn/error).
const consoleLines = [];
for (const level of ["warn", "error"]) {
  console[level] = (...args) => {
    consoleLines.push(`console.${level}: ${args.map(String).join("\n")}`);
  };
}

export function mark() {
  return { s: structured.length, c: consoleLines.length };
}

export function report(label, m) {
  const s = structured.slice(m.s);
  const c = consoleLines.slice(m.c);
  console.log(`  [${label}] diagnostics=${s.length} consoleLines=${c.length}`);
  for (const line of s) console.log(`    diag ${line}`);
  for (const line of c) console.log(`    ${line.split("\n").join("\n    ")}`);
}

// Subscribers of a signal/memo accessor: the nodes linked to its `_subs` list.
// The server build's createSignal returns a plain closure with no node (server.dev.js:640-650).
export function observers(accessor) {
  const node = accessor[$REFRESH];
  if (node === undefined) return ["(no signals node: server-build createSignal)"];
  return DEV.getObservers(node).map((n) => n._name ?? "(unnamed)");
}
