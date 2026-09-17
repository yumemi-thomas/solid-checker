import { scrollRoot } from "probe-browser-source-only";

// A module request outside the served exact URL map. The origin is allowed by
// `script-src`, so the request reaches the launcher's interception, which fails
// it and refuses the launch: the page would have loaded bytes this transaction
// never authenticated. (A `fetch` never gets that far — `connect-src 'none'` is
// browser-enforced and rejects it before any request exists.)
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  scrollRoot();
  await import("https://solid-checker.invalid/modules/not-served.mjs").catch(() => {});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
