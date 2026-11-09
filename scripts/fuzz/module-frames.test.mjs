// ADR 0921: a diagnostic whose stack runs through another module of the program (`el.ts`) is judged against that module's findings at that
// module's frames, exactly as App.tsx's findings are judged at App.tsx's frames, and never across files.
import { describe, expect, it } from "vitest";
import { judge } from "./matrix.mjs";
import { mapPositions } from "./chrome-host.mjs";
import { appStack, isProgramModule, withStarts, withheldAtStack } from "./codes.mjs";
import { compiledHanded } from "./served.mjs";
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
