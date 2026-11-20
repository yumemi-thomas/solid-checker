// The what-if build with no switch against the base: its reports differ by the `assumed` taint alone (and key order: the taint re-serializes the report).
import { gunzipSync } from "node:zlib";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
const S = process.env.RUNS ?? "/Users/thomas/dev/sc-scratch.noindex/mac20b/runs"; // the runs directory (RUNS=<scratch>/runs)
const canon = (value) => JSON.stringify(value, (key, v) => (v && typeof v === "object" && !Array.isArray(v) ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, v[k]])) : v));
const read = (dir, id) => JSON.parse(gunzipSync(readFileSync(join(S, dir, id, "report.json.gz"))).toString("utf8"));
let same = 0, tainted = 0;
const differ = [];
const ids = readdirSync(join(S, "identity-base")).filter((id) => !id.startsWith("."));
for (const id of ids) {
  const base = read("identity-base", id);
  const off = read("whatif-off", id);
  if (Object.prototype.hasOwnProperty.call(off, "assumed")) tainted += 1;
  delete off.assumed;
  if (canon(base) === canon(off)) same += 1;
  else {
    // app-game's SC5003 relatedLocations are nondeterministic between runs of one build: compare without them.
    const strip = (r) => canon({ ...r, findings: r.findings.filter((f) => f.id !== "SC5003") });
    if (id.startsWith("app-game") && strip(base) === strip(off)) same += 1;
    else differ.push(id);
  }
}
console.log(`what-if build, no switch, against the base: ${ids.length} projects, every report tainted (a top-level \`assumed\`): ${tainted}; the report without it equal to the base's: ${same}${differ.length ? `; different: ${differ.join(", ")}` : ""} (app-game compared without its SC5003 findings, which differ between runs of one build)`);
