// node scripts/fuzz/freeze-judge.mjs > scripts/fuzz/judge-frozen.json: run by hand, once (endstage-report.md 1d, tier 1).
// Rows: every flip witness the checker certified (`before`) while Chrome fired a code some rule claims. Each is a false clean by definition,
// so `judgePattern` must call it one, certified. Tables: the judge's tables in codes.mjs, compared exactly by judge-frozen.test.mjs.
import { readFileSync } from "node:fs";
import { judgePattern } from "./patterns.mjs";
import { besideCode, codeRules, decliningRules, isPlacelessCode } from "./codes.mjs";
import { recordedCodes } from "../flip/judge.mjs";
import { judgeTables } from "./judge-tables.mjs";

const witnesses = JSON.parse(readFileSync(new URL("../flip/witnesses.json", import.meta.url), "utf8"));
const rows = [];
let control = null;
/** An SC9012 row (value-forwarded) starting at `line:column` of App.tsx, ending on `endLine`. */
const withheldAt = (line, column, endLine = line) => ({ id: "SC9012", rule: "reactive-dispatch-unresolved", kind: "uncertifiable", analysisContext: "value-forwarded", primaryLocation: { path: "App.tsx", line, column, endLine } });
for (const [id, w] of Object.entries(witnesses)) {
  if (w.before?.status !== "certified" || (w.before.findings ?? []).length) continue;
  const codes = recordedCodes(w.runtime);
  const verdict = judgePattern({ codes, positions: false }, []);
  if (verdict.outcome !== "false-clean") continue; // no claimed code: nothing for the judge to call
  rows.push({ id, codes, findings: [], expected: { outcome: verdict.outcome, certified: verdict.certified } });
  // Its recall-gap twin: one unrelated uncertifiable, so a cover table that answers too much (`coversCode`) fails the row.
  const rule = ["flush-in-action", "missing-owner"].find((candidate) => codes.every((code) => !(codeRules[code] ?? []).includes(candidate) && !(decliningRules[code] ?? []).includes(candidate) && !(besideCode[code] ?? []).includes(candidate)));
  const findings = [{ id: "SC9999", rule, kind: "uncertifiable", analysisContext: "unrelated-blocker" }];
  const twin = judgePattern({ codes, positions: false }, findings);
  if (twin.outcome !== "false-clean" || twin.certified) throw new Error(`${id}: the twin is ${JSON.stringify(twin)}`);
  rows.push({ id: `${id}#unrelated`, codes, findings, expected: { outcome: twin.outcome, certified: twin.certified } });
  // Its withholding twin (review 4b): Chrome's stack puts each code at 3:5, and one SC9012 starts at 9:5. A withholding covers a code
  // only at a recorded position (4c), never a placeless code's, so the row stays a false clean, not certified.
  const stacks = Object.fromEntries(codes.map((code) => [code, [[{ file: "App.tsx", line: 3, column: 5 }]]]));
  const elsewhere = [withheldAt(9, 5)];
  const withheld = judgePattern({ codes, stacks }, elsewhere);
  if (withheld.outcome !== "false-clean" || withheld.certified) throw new Error(`${id}: the withholding twin is ${JSON.stringify(withheld)}`);
  if (!control && !codes.some(isPlacelessCode)) control = { id, codes };
  rows.push({ id: `${id}#sc9012-elsewhere`, codes, stacks, findings: elsewhere, expected: { outcome: withheld.outcome, certified: withheld.certified } });
}
// Stack twins of the first row with no placeless code (review 4c, R5). The read in a helper (h.ts 2:3) called from App at 9:5:
//   #sc9012-helper-call   the SC9012 at the call's position: covered, `fine` (the lane's recall gap), the one covered control;
//   #sc9012-mount-line    the same call on a line that mounts a component: a false clean;
//   #sc9012-prop-on-tag   the SC9012 at a prop expression (9:20) of a component tag on that line: a false clean;
//   #sc9012-enclosing     one SC9012 from 2:1 to 12:2, enclosing every frame: a false clean (positions, never ranges).
if (control) {
  const { id, codes } = control;
  const deep = (mount) => Object.fromEntries(codes.map((code) => [code, [[{ file: "h.ts", line: 2, column: 3 }, { file: "App.tsx", line: 9, column: 5, ...(mount ? { mount: true } : {}) }]]]));
  for (const [suffix, mount, findings, outcome] of [
    ["#sc9012-helper-call", false, [withheldAt(9, 5)], "fine"],
    ["#sc9012-mount-line", true, [withheldAt(9, 5)], "false-clean"],
    ["#sc9012-prop-on-tag", true, [withheldAt(9, 20)], "false-clean"],
    ["#sc9012-enclosing", false, [withheldAt(2, 1, 12)], "false-clean"],
  ]) {
    const verdict = judgePattern({ codes, stacks: deep(mount) }, findings);
    if (verdict.outcome !== outcome) throw new Error(`${id}${suffix}: ${JSON.stringify(verdict)}`);
    rows.push({ id: `${id}${suffix}`, codes, stacks: deep(mount), findings, expected: { outcome: verdict.outcome, ...(verdict.certified !== undefined ? { certified: verdict.certified } : {}) } });
  }
  // A member call (`providers.map(cb)` at 9:5): V8's frame is at the property (9:15), and stands for the call's start and its callee
  // object's (call-starts.mjs). A row at the object's start covers (`#sc9012-member-call`); one at another column of the line does not.
  const member = Object.fromEntries(codes.map((code) => [code, [[{ file: "h.ts", line: 2, column: 3 }, { file: "App.tsx", line: 9, column: 15, starts: [{ line: 9, column: 15 }, { line: 9, column: 5 }] }]]]));
  for (const [suffix, findings, outcome] of [["#sc9012-member-call", [withheldAt(9, 5)], "fine"], ["#sc9012-member-call-other-column", [withheldAt(9, 9)], "false-clean"]]) {
    const verdict = judgePattern({ codes, stacks: member }, findings);
    if (verdict.outcome !== outcome) throw new Error(`${id}${suffix}: ${JSON.stringify(verdict)}`);
    rows.push({ id: `${id}${suffix}`, codes, stacks: member, findings, expected: { outcome: verdict.outcome, ...(verdict.certified !== undefined ? { certified: verdict.certified } : {}) } });
  }
}
const about = "Frozen judge rows (endstage-report.md 1d, tier 1): each row is a false clean Chrome measured, or a twin of one (review 4b-4c: `#unrelated`, `#sc9012-elsewhere` (a placeless code's too), `#sc9012-mount-line`, `#sc9012-prop-on-tag`, `#sc9012-enclosing`, `#sc9012-member-call-other-column`; `#sc9012-helper-call` and `#sc9012-member-call`, the covered controls); a judge or table change that answers otherwise fails judge-frozen.test.mjs. Changing a row or a table needs a `changes` row { id, was, now, adr, why, program } naming an ADR in docs/decisions.md and a program whose Chrome result shows the new answer right. Generated by freeze-judge.mjs.";
process.stdout.write(`${JSON.stringify({ about, tables: judgeTables(), rows, changes: [] }, null, 1)}\n`);
