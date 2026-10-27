import { describe, expect, it } from "vitest";
import { classify } from "./classify.mjs";
import { CONDITIONS, deadSiteConditions } from "./deadsite.mjs";
import { generateProgram } from "./generate.mjs";
import { closedGrammar } from "./model.mjs";
import * as m from "./model.mjs";
import { printProgram } from "./print.mjs";

// A ledger by hand: component entry 1 > memo compute entry 2 > closure entry 3 (the site 4 is in it); a default-param site 5 in closure 6.
const ledger = [
  { hit: 1, kind: "entry", node: "component", enclosing: null, inDefault: false },
  { hit: 2, kind: "entry", node: "fn", enclosing: 1, inDefault: false },
  { hit: 3, kind: "entry", node: "fn", enclosing: 2, inDefault: false },
  { hit: 4, kind: "site", node: "write", enclosing: 3, inDefault: false },
  { hit: 5, kind: "site", node: "read", enclosing: 6, inDefault: true },
  { hit: 6, kind: "entry", node: "fn", enclosing: 2, inDefault: false },
];
const site = { id: "s1", hit: 4, line: 8 };
const ok = { closed: { closed: true, reasons: [] }, quiescent: true, neutral: true, disturbed: [] };
const check = (over = {}) => deadSiteConditions({ sites: [site], ledger, hits: { 1: 1, 2: 1 }, ...ok, ...over });
const unmet = (result) => result.unmet.map((u) => u.condition);

describe("dead-site: the soundness conditions as code", () => {
  it("holds when a closure built by a function that ran was never invoked, and every condition is met", () => {
    const result = check();
    expect(result.ok).toBe(true);
    expect(result.dead).toEqual([{ site: "s1", never: 3 }]);
  });

  it("each unmet condition is typed, and any one of them refuses", () => {
    expect(unmet(check({ closed: { closed: false, reasons: ["Show when is not a nonzero literal"] } }))).toEqual(["closed-grammar"]);
    expect(unmet(check({ closed: undefined }))).toEqual(["closed-grammar"]);
    expect(unmet(check({ quiescent: false }))).toEqual(["quiescent"]);
    expect(unmet(check({ quiescent: null }))).toEqual(["quiescent"]);
    expect(unmet(check({ neutral: false }))).toEqual(["neutral"]);
    expect(unmet(check({ disturbed: ["the program threw"] }))).toEqual(["attributed"]);
    for (const over of [{ quiescent: false }, { neutral: false }, { disturbed: ["x"] }]) expect(check(over).ok).toBe(false);
    for (const condition of unmet(check({ closed: undefined, quiescent: null, neutral: false, disturbed: ["x"] }))) expect(CONDITIONS).toContain(condition);
  });

  it("a host that never ran is a harness fault, not a claim", () => {
    const result = check({ hits: {} });
    expect(result.ok).toBe(false);
    expect(unmet(result)).toEqual(["host-ran"]);
  });

  it("a site skipped inside an entered function is a throw, not a dead closure", () => {
    const result = check({ hits: { 1: 1, 2: 1, 3: 1 } });
    expect(unmet(result)).toEqual(["not-a-throw"]);
    expect(check({ hits: { 1: 1, 2: 1, 3: 1, 4: 1 } }).ok).toBe(false);
  });

  it("a site in a parameter default is argument-dependent: not provable", () => {
    expect(unmet(deadSiteConditions({ sites: [{ id: "s2", hit: 5 }], ledger, hits: { 1: 1, 2: 1 }, ...ok }))).toEqual(["not-a-default"]);
  });

  it("without a readout or a ledger row nothing is provable", () => {
    expect(check({ hits: null }).ok).toBe(false);
    expect(deadSiteConditions({ sites: [{ id: "s9", hit: 99 }], ledger, hits: {}, ...ok }).ok).toBe(false);
    expect(deadSiteConditions({ sites: [], ledger, hits: {}, ...ok }).ok).toBe(false);
  });

  it("a nested never-entered chain is dead too: the nearest ancestor that ran decides", () => {
    const nested = [...ledger, { hit: 7, kind: "site", node: "write", enclosing: 6, inDefault: false }];
    expect(deadSiteConditions({ sites: [{ id: "s3", hit: 7 }], ledger: nested, hits: { 1: 1, 2: 1 }, ...ok }).ok).toBe(true);
  });
});

describe("dead-site in the comparator: off by default", () => {
  const sites = [{ ...site, kind: "write", witness: { selector: "[data-probe=\"w1\"]", text: "17" } }];
  const violation = { rule: "no-write-in-pure-computation", id: "SC2001", kind: "violation", line: 8, endLine: 8 };
  const runtime = (over = {}) => ({ failure: null, diagnostics: [], errors: [], witnesses: [{ siteId: "s1", reached: null, actual: null }], hits: { 1: 1, 2: 1 }, quiescent: true, ...over });
  const run = (flags, extra = {}) => classify({ sites, checker: { ok: true, findings: [violation] }, runtime: runtime(), plan: { clicks: [] }, flags,
    evidence: { ledger, closed: { closed: true, reasons: [] }, neutral: true, ...extra } });

  it("never-ran is inconclusive without the flag, whatever the evidence", () => {
    const result = run({});
    expect(result.verdict).toBe("inconclusive");
    expect(result.items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
    expect(run(undefined).verdict).toBe("inconclusive");
  });

  it("with the flag, a provable dead site is a dead-site candidate false positive", () => {
    const result = run({ deadSite: true });
    expect(result.verdict).toBe("candidate-false-positive");
    expect(result.items[0]).toMatchObject({ subkind: "dead-site", siteIds: ["s1"] });
  });

  it("with the flag but an unmet condition, it stays inconclusive and says which", () => {
    const result = run({ deadSite: true }, { neutral: false });
    expect(result.verdict).toBe("inconclusive");
    expect(result.items[0]).toMatchObject({ why: "never-ran", unmet: ["neutral"] });
    expect(classify({ sites, checker: { ok: true, findings: [violation] }, runtime: runtime({ quiescent: false }), plan: { clicks: [] }, flags: { deadSite: true }, evidence: { ledger, closed: { closed: true }, neutral: true } }).items[0].unmet).toEqual(["quiescent"]);
  });

  it("a disturbance or a diagnostic at the finding wins over a dead site", () => {
    const disturbed = classify({ sites, checker: { ok: true, findings: [violation] }, runtime: runtime({ errors: [{ kind: "handler", message: "boom" }] }), plan: { clicks: [] }, flags: { deadSite: true }, evidence: { ledger, closed: { closed: true }, neutral: true } });
    expect(disturbed.verdict).toBe("inconclusive");
    expect(disturbed.items[0].why).toBe("disturbed");
  });
});

describe("closedGrammar", () => {
  it("accepts what the generator makes (Show when={1}, For each={[1]}, forEach/map over [1], the helper table)", () => {
    let closed = 0;
    for (let seed = 1; seed <= 300; seed += 1) if (closedGrammar(generateProgram(seed)).closed) closed += 1;
    expect(closed).toBeGreaterThan(280);
  });

  it("refuses a conditional, a loop outside the table, recursion, an unknown call, an unknown helper form", () => {
    const base = () => generateProgram(4);
    const withView = (item) => { const p = base(); p.components[0].view.push(item); return p; };
    expect(closedGrammar(withView(m.show(m.read("r1"), [m.probe("r1")]))).reasons.join()).toMatch(/Show when/);
    expect(closedGrammar(withView(m.show(m.num(0), [m.probe("r1")]))).closed).toBe(false);
    expect(closedGrammar(withView(m.show(m.num(1), [], [m.probe("r1")]))).reasons.join()).toMatch(/fallback/);
    const unknownCall = base(); unknownCall.components[0].body.push(m.expr(m.call(m.ref("mystery"))));
    expect(closedGrammar(unknownCall).reasons.join()).toMatch(/undeclared mystery/);
    const recursive = base(); recursive.components[0].body.push(m.decl("loop", m.fn([m.expr(m.call(m.ref("loop")))])));
    expect(closedGrammar(recursive).reasons.join()).toMatch(/loop calls itself/);
    const odd = base(); odd.helpers = [m.helper("h9", "unheard")];
    expect(closedGrammar(odd).reasons.join()).toMatch(/helper form unheard/);
    const awaits = base(); awaits.components[0].body.push(m.expr(m.fn([m.awaitStmt(), m.awaitStmt()], { async: true })));
    expect(closedGrammar(awaits).reasons.join()).toMatch(/more than one await/);
  });

  it("every program it accepts still prints to a twin", () => {
    for (let seed = 1; seed <= 50; seed += 1) {
      const program = generateProgram(seed);
      if (closedGrammar(program).closed) expect(printProgram(program).ledger.length).toBeGreaterThan(0);
    }
  });
});
