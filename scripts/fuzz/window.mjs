// A finding that says the runtime does not warn (`STRICT_READ_UNWINDOWED`, rust/dialects/solid-v2/rules/src/lib.rs; ADRs 0443, 0530) is the
// checker's claim about the runtime, so the judge may not take its word: silence excuses the finding only once silence is measured.
// `@solidjs/signals` gates the warning on a private label (`read`, `if (strictRead)`), so the one view of it from outside is a read. The
// control run reads a signal of its own in every counter (`__h`), one signal per counter, named for it: rc.17 warns on that read exactly
// when a strict-read window is open on that stack. Same line numbers as the twin; the program's own run is untouched.

import { PRELUDE_MARK } from "./hits.mjs";

/** The name of counter `id`'s control signal, which the warning prints. */
const SIGNAL_PREFIX = "__scWindow:";
const SIGNAL_NAME = /"__scWindow:(\d+)"/;
const COUNTER_HEAD = "function __h(id: number): void {";

/** The instrumented twin with the control reads. The first line keeps its number: the declarations share it, and an `import` hoists. */
export function windowProbeText(runText) {
  const cut = runText.indexOf(PRELUDE_MARK);
  if (cut < 0 || !runText.includes(COUNTER_HEAD)) throw new Error("window probe: the text has no counter prelude");
  const ids = [...runText.slice(0, cut).matchAll(/__h\((\d+)\)/g)].map((match) => Number(match[1]));
  const size = Math.max(0, ...ids) + 1;
  const head = `import { createSignal as __scSignal } from "solid-js"; const __scWindow = Array.from({ length: ${size} }, (_, id) => __scSignal(0, { name: \`${SIGNAL_PREFIX}\${id}\` })[0]); `;
  return head + runText.replace(COUNTER_HEAD, `${COUNTER_HEAD}\n  __scWindow[id]();`);
}

/** The counters a strict-read window was open at, from the control run's warnings. */
export function openWindows(record) {
  const open = new Set();
  for (const diagnostic of record?.diagnostics ?? []) {
    if (diagnostic.code !== "STRICT_READ_UNTRACKED") continue;
    const match = SIGNAL_NAME.exec(String(diagnostic.message ?? ""));
    if (match) open.add(Number(match[1]));
  }
  return open;
}

/**
 * Whether the control run measured `finding`'s silence: a counter on its lines ran, and none of those that ran had a window open. No
 * record, a failed or halted run, or lines that never ran measure nothing: the finding stays judged, a false positive if the runtime is silent.
 */
export function silenceMeasured({ finding, built, record }) {
  if (!record || record.failure || record.errors?.length || !record.hits) return false;
  const lines = (site) => site.line >= finding.line && site.line <= (finding.endLine ?? finding.line);
  const counters = [...(built.sites ?? []), ...(built.glue ?? [])].filter((site) => site.hit !== undefined && site.hit !== null && lines(site));
  const ran = counters.filter((site) => Number(record.hits[site.hit] ?? 0) > 0);
  if (!ran.length) return false;
  const open = openWindows(record);
  return ran.every((site) => !open.has(site.hit));
}
