// Assembles M0-results.md from the tables the scripts wrote (results.json, tables.md), the identity and validation summaries, and the method text.
//   node assemble.mjs <tables dir> <out file>
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const S = process.env.S ?? "/Users/thomas/dev/sc-scratch.noindex/mac20b";
const [dir, outFile] = process.argv.slice(2);
const r = JSON.parse(readFileSync(join(dir, "results.json"), "utf8"));
const tables = readFileSync(join(dir, "tables.md"), "utf8");
const method = readFileSync(join(S, "M0-method.md"), "utf8").replace(/^## How to read this \(method, switch, definitions\)\n+/, "");
const optional = (path) => (existsSync(path) ? readFileSync(path, "utf8").trim() : null);
const identity = optional(join(S, "runs/identity-summary.txt")) ?? "(identity summary not recorded)";
const found = optional(join(S, "found-on-the-way.md")) ?? "";
const sha = optional(join(S, "pin/head.sha")) ?? "?";
const finalSha = optional(join(S, "pin/final.sha")) ?? sha;
const n = (value) => value.toLocaleString("en-US");
const pct = (part, whole) => `${((100 * part) / whole).toFixed(1)}%`;
const scenario = (name) => {
  const path = join(S, "scen/scenarios", `${name}.json`);
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
};

const rows = r.baseline.rows;
const floor = r.baseline.floorRows;
const held = rows - floor;
const mech = r.mechanisms;
const freed = (id) => mech[id]?.freed ?? 0;
const projectsFreed = (id) => mech[id]?.projectsFreed ?? 0;
const plan = scenario("M2b+M7v");
const planUncertifiable = (() => {
  let before = 0;
  let after = 0;
  const base = plan?.projects ?? [];
  for (const project of base) {
    after += project.uncertifiable;
    before += JSON.parse(readFileSync(join(S, "runs/census-v2", project.id, "meta.json"), "utf8")).uncertifiable;
  }
  return { before, after };
})();
const floorScenario = scenario("floor");
const ceilingProjects = r.chain.at(-1)?.projects ?? 0;
const m2bAll = scenario("M2b")?.totals.freed ?? 0;
const planAll = plan?.totals.freed ?? 0;
const planProduct = r.chain.at(-1)?.freed ?? 0;
const causeFreed = ["M1", "M1w", "M1u", "M3", "X1131", "M3u", "M4", "M4w", "M5b", "M5", "M5u", "M6"].map((id) => `${id} ${n(freed(id))}`).join(", ");
const together = scenario("M1u+M3u+M4w+M5u+M6");

const out = `# M0 results: the writer-cause census and the what-if (ADR 1114 family 3; ADR 1131 re-scope)

Mac 20b, 2026-11-20. Branch \`claude/writer-cause-census-m0\` (the Rust of the runs is at ${sha.slice(0, 9)} with the what-if gate of ba9057061; later commits are scripts only; the last is named in the reply). Base \`dba9d3cee\` (main with ADR 1114 family 3 slices A–F). **Measured** means a run of the checker on the frozen rc.21 corpus (34 projects, development build, release builds); **ceiling** a mechanism taken to decide everything its definition can (an upper bound on what it frees if built to the design); **estimated** a figure from the design report, or a size (S=1, M=2…), not measured here. Data and scripts: \`/Users/thomas/dev/sc-scratch.noindex/mac20b/\` (runs, scenarios, probes); the scripts are in the branch (\`scripts/writer-census*.mjs\`). Every writer of every project and why: \`M0-writers.md\`.

## 1. Headline

1. **Rows and writers (measured).** ${n(rows)} family-3 rows over the 34 projects at this head (the design report counted 136,756 at slice E; this head carries slice F too). ${n(r.baseline.effectiveWriters)} writer roots, which are ${n(r.unknownGlobal.roots)} writer packages (per app) once a package a linker spells two ways is counted once (${n(r.baseline.byApp.reduce((sum, app) => sum + (app.packages ?? 0), 0))} package-in-project pairs). Every one is non-clear and has a recorded cause: the census explains every writer.
2. **The writer side can move at most ${n(held)} rows, ${pct(held, rows)} (measured).** Leaving out every any-name writer (the what-if, one checker run per project) leaves ${n(floor)} rows (${pct(floor, rows)}): the rows of writers *by name* (\`named-untyped-member\` 16,799, \`named-untyped-call\` 10,503, \`named-project-*\` about 9,500, …) and of sites that do not ask of an any-name writer. The design report's "rows in apps whose every project reaches 0 writers" counted all of an app's rows; the floor shows a third of them stay.
3. **Why a root is a writer (measured, per app, a root once).** The global object handed on by a name: 301 roots (47 have no other cause); \`source\`/\`view\`/\`defaultView\` taken for the window: 238 / 63 / 87 (\`source\` alone 18); a built-in's prototype handed on: 149; code nobody read: \`DynamicLoad\` 138, \`MissingSource\` 115, \`NativeModule\` 33; code generated at run time: \`function-dynamic\` 118, \`function-literal\` 111, \`eval\` 86; a prototype member written under a key nothing decides: 83. **No unknown global makes a writer**: 294 of the ${n(r.unknownGlobal.roots)} writers hold one among their problems (it is the first problem, the one a row's message prints, for 139), and only ${r.unknownGlobal.onlyUnknown} holds nothing else, so M6 (unknown globals by the program's census) frees nothing.
4. **No cause-level mechanism frees a row (measured).** Each alone: ${causeFreed} rows${together ? `; all of them together, the upper bounds included (M1u+M3u+M4w+M5u+M6): ${n(together.totals.freed)} rows, ${together.totals.projectsChanged} projects` : ""}. A project's rows go only when its LAST any-name writer is decided, and after every cause-level mechanism ${r.floorMadeOf ? "151" : "?"} writer packages (per app) stay for causes none decides: unread code (87 \`DynamicLoad\`, 73 \`MissingSource\`, 22 \`NativeModule\`), code generated at run time (78 \`function-dynamic\`, 52 \`eval\`, 24 \`function-value\`), and prototype members written under a key nothing decides (64). The writer a message names (the first by root) is not what holds a project: **V2**, all but one writer left out, keeps every row in ${r.v2.same} of ${r.v2.projects} projects with two or more writers.
5. **Root-level mechanisms (measured).** M7v, reviewed bytes for vite's injected dev client (\`donegeon-client\`, \`helge-dev\`: vite is their only writer): ${n(freed("M7v"))} rows, ${projectsFreed("M7v")} projects, no ruling (the dev host loads the client whatever the program imports, so no M2 mechanism may decide \`vite\`). M2a (a package no value import loads): ${n(freed("M2a"))} rows, and ${n(freed("M2aO"))} with virtual modules and aliases read. M2b (the product realm, needs the ruling): ${n(freed("M2b"))} rows in product files, ${n(m2bAll)} if every row of the freed projects goes, ${projectsFreed("M2b")} projects (kui's eleven, every-deck-of-cards, jar-hell-web); the optimistic M2bO frees the same.
6. **The cumulative ranked plan (measured).** There are two steps: **M2b** (+${n(r.chain[0]?.gain ?? 0)} rows, ${r.chain[0]?.projects ?? 0} projects, needs the ruling), then **M7v** (+${n(r.chain[1]?.gain ?? 0)}, ${r.chain[1]?.newly.length ?? 0} more projects): ${n(planProduct)} rows in product files (${n(planAll)} counting every row of the freed projects; ${pct(planAll, rows)} of ${n(rows)}), ${ceilingProjects} projects. Without the ruling only M7v: ${n(freed("M7v"))} rows (${pct(freed("M7v"), rows)}). Every other mechanism adds nothing, upper bounds included (M1u+M2bO+M3u+M4w+M5u+M6+M7v: ${n(scenario("M1u+M2bO+M3u+M4w+M5u+M6+M7v")?.totals.freed ?? 0)}). Of all ${n(planUncertifiable.before)} uncertifiable rows of the corpus the plan removes ${n(planUncertifiable.before - planUncertifiable.after)} (the family-3 rows and ${n(planUncertifiable.before - planUncertifiable.after - planAll)} rows of other rules that read the writer index); violations: ${scenario("M2b+M7v")?.totals.violationsDelta === 0 ? "unchanged (244)" : "changed"}.
7. **Against the design report.** Its ceiling "about 36,900 of 136,756 (27%), only with the test-and-config ruling, 0 to 1,800 without" becomes **${n(planAll)} of ${n(rows)} (${pct(planAll, rows)}) with the ruling and ${n(freed("M7v"))} (${pct(freed("M7v"), rows)}) without**: the same projects free, fewer rows, because a freed project keeps its floor rows (kui 9,670 of 33,919). Two corrections: **M2b alone** frees kui's eleven projects (the estimate needed the escapes decided too: R5's closure over \`package.json\` dependencies over-counted what the product loads), and **M1, M3, M4, M5 and M6 add nothing** to the plan even as upper bounds (the report: "M1 part of 36.9k, 0 alone"). Its A column (M2a) is 0 and so is the measurement; its per-app writer counts reproduce (app-game 86 against 85 packages; 34 left under everything, as in its B+esc column). Per app: the table "Against the design report".
8. **ADR 1131 alone (measured): ${n(freed("X1131"))} rows.** X1131 decides every \`source\`/\`view\`/\`defaultView\` route in a file of the program, the best case a typed discharge can reach, and decides ${mech.X1131?.perProject.reduce((sum, item) => sum + (item.packages - item.packagesLeft), 0) ?? 0} writer package. The prediction "about 0 app-game rows" holds, and so does the scope: of the route occurrences the census counts, 110 (distinct file × class per app: app-game 57 in 31 files, viviana-ui-web 51 in 17, jandibat-web 2) are in files of the program, where types exist, against 17,336 in installed JavaScript, where none do (app-game's \`library.document.source\` in \`abr-brush\` is one writer of 85; \`abr-brush\` also holds unread code, prototype-key writes and generated code and stays under every mechanism). The coordinator's "139 routes in app-game's own files" is the same reads counted once per spelling of a package that has up to 197 spellings of 85.
9. **Validation (measured).** The offline decision (which roots a set leaves out, so whether a project's rows go) agreed with the checker's runs in ${r.agreement.agree} of ${r.agreement.agree + r.agreement.disagree.length} (scenario, project) pairs. The checker's own class matcher (\`SOLID_CHECKER_WHATIF_WRITER_CAUSES\`) left out the same roots as the offline step for the route classes, the M4 classes and \`unloaded\` in all 629 roots of 34 projects (0 differences). V2 (item 4): ${r.v2.same} of ${r.v2.projects}.
10. **The switch is off by default (measured)**: see section 4.

## 2. What I recommend building first

1. **Do not build M1, M3, M4, M5, M6 or ADR 1131 as a writer-side measure.** They free 0 rows in this corpus, alone or together, upper bounds included, because the last writer of every project holds causes none of them decides (unread code, real generation, prototype-key writes). If one is wanted for another reason, it is not a rows reason. The report's order (M3 with 1131, then M1) would have spent three slices for 0.
2. **Ask for the M2b ruling now: it is a decision, not code, and it is the only lever with mass** (${n(r.chain[0]?.gain ?? 0)} rows in product files, ${n(m2bAll)} counting every row of the freed projects, ${pct(m2bAll, rows)}; all in kui's eleven projects and two small ones). Two things to say with the ask. (a) My M2b decides a writer from a file-name test of the files that import it, a measurement tool; the real mechanism needs the entry set (ADR 1055 part 2, ADR 1079 step 0), which the corpus does not close: ADR 1114 family 3 slice A records that scoping writers by ADR 1120's entry realms moved 1 row of 15,175 for that reason. The 13 projects here have no open load in their product files (the table "M2's fail-closed condition"), which is why a name test is enough for them and may not be for a project with \`virtual:file-routes\`, \`~icons/*\` or unread declaration-side JavaScript: 14 of the 34 projects have such a load, and M2 keeps today's answer there (M2bO, which reads them as closed, frees the same 13 projects: they are not what blocks the rest). (b) A test realm still runs the app's files with vitest and chai loaded: under position 2 the rows of the test files stay (2,810 of the freed projects' rows).
3. **M7v is the only no-ruling item: ${n(freed("M7v"))} rows (${pct(freed("M7v"), rows)}), two projects, two files** (\`vite/dist/client/client.mjs\`, \`env.mjs\`; \`bundledDevClient.mjs\` in newer vite). Small and recurring (digests per vite version, ADR 0974's mechanism); in a production build the client is not loaded and M2a alone frees these two projects. Worth doing only if the 2.3% matters.
4. **Then leave the writer side.** After both steps ${n(rows - planAll)} rows (${pct(rows - planAll, rows)}) remain, and ${n(floor)} (${pct(floor, rows)}) would remain with every any-name writer decided. Those rows move only at the site (the review's P2, P4, P6: prove the receiver or the operand cannot run a written member). The projects closest to free by a targeted premise are in the table "Closest to free": compass-ui (3,295 rows) is held by two packages (posthog-js: a prototype key; shiki: a native module), error-menu-web and hoxxes-briefing's app project by two each, ai-memory-ui by three.
5. **Two scan findings to put in the queue** (found on the way, section 6): a global in an export position is invisible to the module scan (a false-clean candidate with a probe on the base build), and a computed read of a prototype makes its file an any-name writer (a cheap over-approximation that no corpus row depends on).

## 3. Tables

${tables}

## 4. Identity of the switch (off by default)

${identity}

## 5. Method, switch, definitions

${method}

${found.replace(/^## Found on the way/, "## 6. Found on the way")}

## 7. Reproduce

\`\`\`sh
make build-checker-release && make build-what-if            # the default build and the what-if build (ADR 0950)
SOLID_CHECKER_WRITER_CENSUS=1 solid-checker-rust …          # any build: the census on stderr, one JSON line
node scripts/writer-census.mjs --checker <default build> --typefacts bin/solid-typefacts --out DIR --census --keep-report
node scripts/writer-census-report.mjs drops --baseline DIR --mechanisms M2b,M7v --out DROPS      # roots a set decides, one file per project
SOLID_CHECKER_WHATIF_WRITER_DROP=DROPS/<id>.drop <what-if build> …                              # or SOLID_CHECKER_WHATIF_WRITER_CAUSES='escape:source/,escape:view/'
node scripts/writer-census-tables.mjs --baseline DIR --emit-scenarios scenarios.json --stage A   # then B with --assume-floor-zero 1
node scripts/writer-census-scenarios.mjs --baseline DIR --out SCEN --checker <what-if build> --typefacts bin/solid-typefacts --scenarios scenarios.json
node scripts/writer-census-tables.mjs --baseline DIR --scenarios SCEN --out TABLES              # tables.md, writers.md, results.json
\`\`\`
`;
writeFileSync(outFile, out);
console.log(`wrote ${outFile} (${out.length} bytes)`);
