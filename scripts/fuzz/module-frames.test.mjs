// ADR 0921: a diagnostic whose stack runs through another module of the program (`el.ts`) is judged against that module's findings at that
// module's frames, exactly as App.tsx's findings are judged at App.tsx's frames, and never across files.
import { describe, expect, it } from "vitest";
import { judge } from "./matrix.mjs";
import { mapPositions } from "./chrome-host.mjs";
import { appStack, isProgramModule, withStarts, withheldAtStack } from "./codes.mjs";
import { compiledCallParens, compiledHanded, compiledSpread, sourceCallStarts } from "./served.mjs";
import { classify } from "./classify.mjs";
import { parseModuleFindings } from "./checker.mjs";

// mx-strict-read-untracked-body-use-export-let-direct-none (generator 2, seed 1), as printed, and Chrome's stack for its read (2026-11-09).
const APP = [
  'import { createSignal } from "solid-js";',
  "const [elc] = createSignal(1);",
  "export let elRead = elc;",
  'import { readEl } from "./el";',
  "",
  "export function C1() {",
  "  const [a1, setA1] = createSignal(0);",
  "  void a1; void readEl();;",
  "  return (",
  "    <div>",
  "    </div>",
  "  );",
  "}",
  "",
].join("\n");
const EL = 'import { elRead } from "./App";\nexport const readEl = () => elRead();\n';
const built = { text: APP, files: { "App.tsx": APP, "el.ts": EL } };
const site = { id: "s1", line: 8, aux: [] };
const STACK = [{ file: "el.ts", line: 2, column: 29 }, { file: "App.tsx", line: 8, column: 17 }, { file: "sc-entry.ts" }];
const runtime = (positions = STACK) => ({ diagnostics: [{ code: "STRICT_READ_UNTRACKED", message: "m", positions }], errors: [], witnesses: [] });
const row = (file, line, column, rule = "reactive-dispatch-unresolved", kind = "uncertifiable") => ({ file, rule, kind, id: "SC", line, column, endLine: line });
const verdict = ({ moduleFindings = [], findings = [], positions, files = built.files } = {}) => judge({
  code: "STRICT_READ_UNTRACKED", site, typecheck: { ok: true }, built: { ...built, files }, runtime: runtime(positions),
  checker: { ok: true, findings, ...(moduleFindings.length ? { moduleFindings } : {}) },
});

describe("the matrix judge reads the program's other modules at their own frames (ADR 0921)", () => {
  it("a withholding in el.ts at Chrome's el.ts frame is a recall gap, as at an App.tsx frame (cause 7: was a false clean)", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29)] })).toMatchObject({ outcome: "recall-gap", detail: { declinedBy: "withholding", remote: "reactive-dispatch-unresolved/uncertifiable@el.ts:2" } });
  });
  it("one column off the frame covers nothing: positions, never lines", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 28)] }).outcome).toBe("false-clean");
    expect(verdict({ moduleFindings: [row("el.ts", 2, 1)] }).outcome).toBe("false-clean");
  });
  it("the same position in another file covers nothing: an App.tsx finding at 2:29, a helpers.ts finding at el.ts's frame", () => {
    expect(verdict({ findings: [row("App.tsx", 2, 29)] }).outcome).toBe("false-clean");
    const files = { ...built.files, "helpers.ts": EL };
    expect(verdict({ moduleFindings: [row("helpers.ts", 2, 29)], files }).outcome).toBe("false-clean");
  });
  it("a module the program does not write is not read, whatever its finding", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29)], files: { "App.tsx": APP } }).outcome).toBe("false-clean");
  });
  it("a frame recorded with no position places nothing (the record before the host mapped modules)", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29)], positions: [{ file: "el.ts" }, ...STACK.slice(1)] }).outcome).toBe("false-clean");
  });
  it("a claiming violation in el.ts at the frame agrees, remote; off it, it is not a claim at all", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29, "strict-read-untracked", "violation")] })).toMatchObject({ outcome: "agree", detail: { remote: "strict-read-untracked@el.ts:2" } });
    expect(verdict({ moduleFindings: [row("el.ts", 2, 30, "strict-read-untracked", "violation")] }).outcome).toBe("false-clean");
  });
  it("another rule's finding in el.ts at the frame is no claim", () => {
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29, "missing-owner", "uncertifiable")] }).outcome).toBe("false-clean");
  });
  it("a code placed only in el.ts is judged there, not unplaced, and stays a false clean without a finding at the frame", () => {
    const only = [{ file: "el.ts", line: 2, column: 29 }];
    expect(verdict({ moduleFindings: [row("el.ts", 2, 29)], positions: only }).outcome).toBe("recall-gap");
    expect(verdict({ positions: only }).outcome).toBe("false-clean");
    expect(verdict({ positions: [{ file: "el.ts" }] }).outcome).toBe("unplaced");
  });
});

describe("appStack and isProgramModule", () => {
  it("keeps the named file's frames and blanks the others, keeping depth", () => {
    expect(appStack(STACK)).toEqual([{ file: "el.ts", line: -1 }, STACK[1], { file: "sc-entry.ts", line: -1 }]);
    expect(appStack(STACK, "el.ts")).toEqual([STACK[0], { file: "App.tsx", line: -1 }, { file: "sc-entry.ts", line: -1 }]);
    expect(appStack([{ line: 3, column: 1 }], "el.ts")).toEqual([{ file: undefined, line: -1 }]);
  });
  it("a program module is a top-level .ts/.tsx source other than App.tsx and the entries", () => {
    expect(["el.ts", "helpers.tsx"].every(isProgramModule)).toBe(true);
    expect(["App.tsx", "main.tsx", "sc-entry.ts", "x.d.ts", "sub/h.ts", "h.js", undefined].some(isProgramModule)).toBe(false);
  });
});

describe("the Chrome host maps another module's frames through that module's transform (mapPositions)", () => {
  // Each module's map adds its own offset, so a frame mapped through the wrong module's map lands elsewhere.
  const offsets = { "App.tsx": 0, "el.ts": 1 };
  const server = { moduleGraph: { getModuleByUrl: async (url) => ({ transformResult: { map: { mappings: "x", file: url.split("/").pop() } } }) }, transformRequest: async () => null };
  const trace = { TraceMap: class { constructor(raw) { this.file = raw.file; } }, originalPositionFor: (map, { line, column }) => ({ line: line - offsets[map.file], column }) };
  it("gives el.ts's source position; keeps a module it has no text for, and the host's entry, by file only", async () => {
    const record = { diagnostics: [{ code: "X", positions: [{ file: "el.ts", line: 3, column: 29 }, { file: "App.tsx", line: 8, column: 17 }, { file: "sc-entry.ts", line: 2, column: 1 }, { file: "lib.ts", line: 1, column: 1 }] }] };
    const mapped = await mapPositions(server, trace, "a", record, APP, undefined, (file) => (file === "el.ts" ? EL : undefined));
    expect(mapped.diagnostics[0].positions).toEqual([{ file: "el.ts", line: 2, column: 29 }, { file: "App.tsx", line: 8, column: 17 }, { file: "sc-entry.ts" }, { file: "lib.ts" }]);
  });
  it("a frame past the end of its module's line keeps its file and no position", async () => {
    const record = { diagnostics: [{ code: "X", positions: [{ file: "el.ts", line: 3, column: 60 }] }] };
    const mapped = await mapPositions(server, trace, "a", record, APP, undefined, () => EL);
    expect(mapped.diagnostics[0].positions).toEqual([{ file: "el.ts" }]);
  });
});

describe("the line-only judges read App.tsx's lines only", () => {
  it("an el.ts frame on line 8 is no runtime report at an App.tsx violation on line 8 (classify.mjs)", () => {
    const sites = [{ id: "s1", line: 8, kind: "write", witness: null }];
    const at = (file) => ({ failure: null, diagnostics: [{ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", message: "m", positions: [{ file, line: 8, column: 1 }] }], errors: [], witnesses: [{ siteId: "s1", reached: true }] });
    const checker = { ok: true, findings: [{ rule: "no-write-in-pure-computation", id: "SC2001", kind: "violation", line: 8, column: 1, endLine: 8 }] };
    const kinds = (file) => classify({ sites, checker, runtime: at(file), plan: { clicks: [] } }).items.map((item) => item.kind);
    expect(kinds("App.tsx")).toContain("agree");
    expect(kinds("el.ts")).not.toContain("agree");
  });
});

describe("parseModuleFindings", () => {
  it("keeps the findings of the program's modules with their file, never App.tsx's, the entry's or a package's", () => {
    const at = (path, line) => ({ rule: "r", id: "SC", kind: "uncertifiable", primaryLocation: { path, line, column: 3, endLine: line } });
    const analysis = { findings: [at("/w/p/App.tsx", 1), at("/w/p/el.ts", 2), at("/w/p/main.tsx", 3), at("/w/p/node_modules/x/index.ts", 4), at("/w/q/el.ts", 5)] };
    expect(parseModuleFindings(analysis, "/w/p")).toEqual([{ file: "el.ts", rule: "r", id: "SC", kind: "uncertifiable", line: 2, column: 3, endLine: 2 }]);
  });
});

// Cause 8 (ADR 0921): mx-no-owner-effect-handler-jsx-array-direct-none as printed, and the dev compiler's output for such JSX (rc.21, 2026-11-09).
const JSX_APP = [
  'import { createSignal } from "solid-js";',
  "",
  "export function C1() {",
  "  return (",
  "    <div>",
  '      <button data-act="b1" onClick={() => {',
  "        <p>{[() => 1]}</p>;",
  "      }}>go</button>",
  "    </div>",
  "  );",
  "}",
  "",
].join("\n");
const SERVED = [
  'import { insert as _$insert } from "/.vite/deps/@solidjs_web.js?v=1";',
  'import { effect as _$effect } from "/.vite/deps/@solidjs_web.js?v=1";',
  'import { memo as _$memo } from "/.vite/deps/@solidjs_web.js?v=1";',
  'import { readShallow as _$readShallow } from "/.vite/deps/@solidjs_web.js?v=1";',
  'import { spread as _$spread } from "/.vite/deps/@solidjs_web.js?v=1";',
  "_$insert(_el$, [() => 1]);",
  "_$insert(_el$, () => {",
  "  return String(Math.random());",
  "}, undefined, undefined, {});",
  "_$effect(() => _$readShallow(String(Math.random())), (_v$, _$p) => {});",
  "_$memo(() => {",
  "  return String(Math.random());",
  "});",
  "_$spread(_el$3, () => x, false);",
  "insert(_el$, y);",
].join("\n");

describe("a frame at a compiled insert, effect or memo stands for the expression it was handed (ADR 0921, cause 8)", () => {
  const at = compiledHanded(SERVED);
  it("gives the handed value's start, through the compiler's arrows and readShallow, on the callee only", () => {
    expect(at(6, 1)).toEqual([{ line: 6, column: 16 }]);
    expect(at(6, 9)).toEqual([{ line: 6, column: 16 }]);
    expect(at(7, 1)).toEqual([{ line: 8, column: 10 }]);
    expect(at(10, 1)).toEqual([{ line: 10, column: 30 }]);
    expect(at(11, 1)).toEqual([{ line: 12, column: 10 }]);
    expect(at(6, 16)).toEqual([]);
  });
  it("nothing for a spread, a call not imported from @solidjs/web, or a position off every callee", () => {
    expect(at(14, 1)).toEqual([]);
    expect(at(15, 1)).toEqual([]);
    expect(at(1, 1)).toEqual([]);
  });
  const frame = { file: "App.tsx", line: 7, column: 9, compiled: [{ line: 7, column: 13 }] };
  const claimU = { rule: "missing-owner", kind: "uncertifiable", line: 7, column: 13 };
  it("the top frame's compiled start covers a claiming finding there; never a withholding, never a deeper frame's", () => {
    expect(withheldAtStack(claimU, withStarts([frame], null))).toBe(true);
    expect(withheldAtStack({ ...claimU, rule: "reactive-dispatch-unresolved" }, withStarts([frame], null))).toBe(false);
    expect(withheldAtStack(claimU, withStarts([{ file: "App.tsx", line: 1, column: 1 }, frame], null), () => false)).toBe(false);
    expect(withheldAtStack({ ...claimU, column: 14 }, withStarts([frame], null))).toBe(false);
  });
  const jsxVerdict = (positions, findings) => judge({
    code: "NO_OWNER_EFFECT", site: { id: "s1", line: 7, aux: [] }, typecheck: { ok: true }, built: { text: JSX_APP, files: { "App.tsx": JSX_APP } },
    runtime: { diagnostics: [{ code: "NO_OWNER_EFFECT", message: "m", positions }], errors: [], witnesses: [] }, checker: { ok: true, findings },
  });
  const u = (line, column, rule = "missing-owner") => ({ file: "App.tsx", rule, kind: "uncertifiable", id: "SC4001", line, column, endLine: line });
  const stack = [frame, { file: "App.tsx", line: 7, column: 26 }];
  it("SC4001's U on the container's expression is a recall gap at the compiled insert's frame (was a false clean)", () => {
    expect(jsxVerdict(stack, [u(7, 13)])).toMatchObject({ outcome: "recall-gap", detail: { declinedBy: "claiming" } });
  });
  it("without the compiled start, one column off it, or as a withholding, it stays a false clean", () => {
    expect(jsxVerdict([{ ...frame, compiled: undefined }, stack[1]], [u(7, 13)]).outcome).toBe("false-clean");
    expect(jsxVerdict(stack, [u(7, 12)]).outcome).toBe("false-clean");
    expect(jsxVerdict(stack, [u(7, 13, "reactive-dispatch-unresolved")]).outcome).toBe("false-clean");
  });
  it("the host records a compiled start only where the source map has a segment at the value's own start", async () => {
    const code = "_$insert(_el$, [() => 1]);";
    const served = `import { insert as _$insert } from "/.vite/deps/@solidjs_web.js";\n${code}`;
    const server = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { map: { mappings: "x" }, code: served } }) } };
    // Generated line 2: the call maps to 7:9, the value (column 16) to 7:13 when the map has a segment there.
    const trace = (segments) => ({
      TraceMap: class {},
      originalPositionFor: (_, { line, column }) => (line === 2 ? (column >= 15 && segments.includes(15) ? { line: 7, column: 12 } : { line: 7, column: 8 }) : { line, column }),
      traceSegment: (_, line, column) => (line === 1 ? [segments.filter((at) => at <= column).at(-1)] : null),
    });
    const record = { diagnostics: [{ code: "NO_OWNER_EFFECT", positions: [{ file: "App.tsx", line: 2, column: 1 }] }] };
    expect((await mapPositions(server, trace([0, 15]), "a", record, JSX_APP)).diagnostics[0].positions).toEqual([{ file: "App.tsx", line: 7, column: 9, compiled: [{ line: 7, column: 13 }] }]);
    expect((await mapPositions(server, trace([0]), "a", record, JSX_APP)).diagnostics[0].positions).toEqual([{ file: "App.tsx", line: 7, column: 9 }]);
  });
});

// Cause 10 (ADR 0930): mx-no-owner-effect-handler-jsx-spread-direct-none as printed, and the dev compiler's output for its spread (rc.21,
// 2026-11-10): the call maps to nothing, its operand to 9:18, and V8's next frame (the element's IIFE) to the element's end, 9:29.
const SPREAD_APP = [
  'import { createContext, createSignal } from "solid-js";',
  "",
  'const jsxRest1 = { class: "x" };',
  "",
  "export function C1() {",
  "  return (",
  "    <div>",
  '      <button data-act="b1" onClick={() => {',
  "        <div {...jsxRest1} />;",
  "      }}>go</button>",
  "    </div>",
  "  );",
  "}",
  "",
].join("\n");
const SPREAD_SERVED = [
  'import { spread as _$spread } from "/.vite/deps/@solidjs_web.js?v=1";',
  'import { insert as _$insert } from "/.vite/deps/@solidjs_web.js?v=1";',
  '_$spread(_el$3, jsxRest1, false, undefined, "div", "cases/a/App.tsx:9:14");',
  '_$spread(_el$4, [a, b], false, undefined, "div", "other.tsx:3:18");',
  "_$spread(_el$5, x, false);",
  '_$insert(_el$, y, undefined, undefined, "App.tsx:2:2");',
  'spread(_el$, y, false, undefined, "div", "App.tsx:2:2");',
].join("\n");

describe("a frame at a compiled spread stands for the spread its loc names (ADR 0930, cause 10)", () => {
  const at = compiledSpread(SPREAD_SERVED, ["App.tsx", "cases/a/App.tsx"]);
  it("gives the call's loc, on the callee only, for this module's spread", () => {
    expect(at(3, 1)).toEqual([{ line: 9, column: 14 }]);
    expect(at(3, 8)).toEqual([{ line: 9, column: 14 }]);
    expect(at(3, 10)).toEqual([]);
  });
  it("nothing for another module's loc, a spread with no loc, another call, or one not imported from @solidjs/web", () => {
    for (const line of [4, 5, 6, 7]) expect(at(line, 1)).toEqual([]);
  });
  const verdict = (positions, findings) => judge({
    code: "NO_OWNER_EFFECT", site: { id: "s1", line: 9, aux: [] }, typecheck: { ok: true }, built: { text: SPREAD_APP, files: { "App.tsx": SPREAD_APP } },
    runtime: { diagnostics: [{ code: "NO_OWNER_EFFECT", message: "m", positions }], errors: [], witnesses: [] }, checker: { ok: true, findings },
  });
  const finding = (kind, line, column, rule = "missing-owner") => ({ file: "App.tsx", rule, kind, id: "SC4001", line, column, endLine: line });
  const spreadFrame = { file: "App.tsx", line: 9, column: 14, compiled: [{ line: 9, column: 14 }], unmapped: true };
  const iife = { file: "App.tsx", line: 9, column: 29 };
  it("SC4001 at the spread agrees at the spread's frame (was misplaced); its U is a recall gap (was a false clean)", () => {
    expect(verdict([iife], [finding("violation", 9, 14)]).outcome).toBe("misplaced");
    expect(verdict([spreadFrame, iife], [finding("violation", 9, 14)]).outcome).toBe("agree");
    expect(verdict([iife], [finding("uncertifiable", 9, 14)]).outcome).toBe("false-clean");
    expect(verdict([spreadFrame, iife], [finding("uncertifiable", 9, 14)])).toMatchObject({ outcome: "recall-gap", detail: { declinedBy: "claiming" } });
  });
  it("the frame has no position of its own: no withholding there, no other column, nothing below the top", () => {
    expect(verdict([spreadFrame, iife], [finding("uncertifiable", 9, 14, "reactive-dispatch-unresolved")]).outcome).toBe("false-clean");
    expect(verdict([spreadFrame, iife], [finding("uncertifiable", 9, 18)]).outcome).toBe("false-clean");
    expect(withheldAtStack(finding("violation", 9, 14), withStarts([{ file: "App.tsx", line: 1, column: 1 }, spreadFrame], null), () => false)).toBe(false);
    expect(withheldAtStack(finding("uncertifiable", 9, 14, "reactive-dispatch-unresolved"), [spreadFrame])).toBe(false);
  });
  it("the host keeps a spread call's frame, which the source map drops, at its loc on the clean text", async () => {
    const served = SPREAD_SERVED.split("\n").slice(0, 3).join("\n");
    const server = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { map: { mappings: "x" }, code: served } }) } };
    const trace = { TraceMap: class {}, originalPositionFor: (_, { line }) => (line === 3 ? { line: null, column: null } : { line: 9, column: 28 }), traceSegment: () => null };
    const record = { diagnostics: [{ code: "NO_OWNER_EFFECT", positions: [{ file: "App.tsx", line: 3, column: 1 }, { file: "App.tsx", line: 4, column: 1 }] }] };
    expect((await mapPositions(server, trace, "a", record, SPREAD_APP)).diagnostics[0].positions).toEqual([spreadFrame, iife]);
    // On the instrumented twin, its loc goes to the clean text as every other position does.
    const shifted = (await mapPositions(server, trace, "a", record, SPREAD_APP, ({ line, column }) => ({ line, column: column - 2 }))).diagnostics[0].positions;
    expect(shifted[0]).toEqual({ ...spreadFrame, column: 12, compiled: [{ line: 9, column: 12 }] });
  });
});

// Causes 12-13 (ADR 0930): mx-strict-read-untracked-body-guard-passed-direct-helper's line 11 and its compiled line 22 (rc.21, 2026-11-10).
// V8 puts `children(…)()`'s frame at its second `(` (generated 22:29); the segment before it (22:28) maps to the source `(`, 11:143, one
// past the callee; the call's start (generated 19:8) maps exactly to 11:104, the checker's row. In direct-none the drift lands on the `)`.
describe("a frame at the `(` of a call whose callee is no identifier stands for that call's start (ADR 0930, causes 12-13)", () => {
  const served = ["void children(() => _$createComponent(K, {", "\tr: () => a1()", '}, "K", "App.tsx:2:22"))();', "(() => { return 1; })();", "f(x)(y);", "a.b(1);", "g?.()(2);"].join("\n");
  const at = compiledCallParens(served);
  it("gives the start of the call whose argument list opens there, for no identifier callee", () => {
    expect(at(3, 25)).toEqual([{ line: 1, column: 6 }]);
    expect(at(3, 24)).toEqual([]);
    expect(at(4, 22)).toEqual([{ line: 4, column: 1 }]);
    expect(at(5, 5)).toEqual([{ line: 5, column: 1 }]);
    expect(at(5, 2)).toEqual([]);
    expect(at(6, 4)).toEqual([{ line: 6, column: 1 }]);
  });
  it("knows a source call by its start, never an identifier callee's or a JSX element's", () => {
    const has = sourceCallStarts("void children(() => <K r={() => a1()} />)();\nf(1);\nconst e = <div />;");
    expect(has({ line: 1, column: 6 })).toBe(true);
    expect(has({ line: 1, column: 15 })).toBe(false);
    expect(has({ line: 2, column: 1 })).toBe(false);
    expect(has({ line: 3, column: 11 })).toBe(false);
  });
  it("the host records the call's start only with an exact segment there and a source call there", async () => {
    const source = "  void children(() => <K r={() => a1()} />)();";
    const server = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { map: { mappings: "x" }, code: served } }) } };
    const trace = (segments, start = 8) => ({
      TraceMap: class {},
      originalPositionFor: (_, { line, column }) => (line === 1 && column === 5 ? { line: 1, column: start - 1 } : { line: 1, column: 44 }),
      traceSegment: (_, line, column) => (line === 0 ? [segments.filter((s) => s <= column).at(-1)] : [column]),
    });
    const record = { diagnostics: [{ code: "STRICT_READ_UNTRACKED", positions: [{ file: "App.tsx", line: 3, column: 25 }] }] };
    const mapped = async (t) => (await mapPositions(server, t, "a", record, source)).diagnostics[0].positions;
    expect(await mapped(trace([5]))).toEqual([{ file: "App.tsx", line: 1, column: 45, compiled: [{ line: 1, column: 8 }] }]);
    expect(await mapped(trace([4]))).toEqual([{ file: "App.tsx", line: 1, column: 45 }]);
    expect(await mapped(trace([5], 9))).toEqual([{ file: "App.tsx", line: 1, column: 45 }]);
  });
  const APP11 = ["", "", "", "", "", "", "", "", "", "", "    const K = (p: { r?: () => number; on?: boolean }) => { if (p.on) void p.r!(); return null; }; void children(() => <K on r={() => a1()} />)();;", ""].join("\n");
  const verdict = (positions) => judge({
    code: "STRICT_READ_UNTRACKED", site: { id: "s1", line: 11, aux: [] }, typecheck: { ok: true }, built: { text: APP11, files: { "App.tsx": APP11 } },
    runtime: { diagnostics: [{ code: "STRICT_READ_UNTRACKED", message: "m", positions }], errors: [], witnesses: [] },
    checker: { ok: true, findings: [{ file: "App.tsx", rule: "strict-read-untracked", kind: "violation", id: "SC1001", line: 11, column: 104, endLine: 11 }] },
  });
  it("the checker's row at the call's start agrees at that frame (was misplaced one column past the callee)", () => {
    expect(verdict([{ file: "App.tsx", line: 11, column: 143 }]).outcome).toBe("misplaced");
    expect(verdict([{ file: "App.tsx", line: 11, column: 143, compiled: [{ line: 11, column: 104 }] }]).outcome).toBe("agree");
  });
});
