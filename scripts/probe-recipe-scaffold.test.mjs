// Pins `scripts/probe-recipe-scaffold.mjs`.
//
// Two of these tests are the reason the script may exist at all. A probe
// recipe that runs to completion without emitting its marker is a *clean
// non-observation*: the mandatory veto passes and the closure certifies on
// the implementation census alone. So a generator that emitted a runnable
// module which observes nothing would convert "nobody wrote the observation"
// into "nothing contradicted the closure", silently, across every candidate
// at once. `emits a module that refuses to run` and `never overwrites` are
// what stand between the tool and that outcome.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "vitest";

import {
  DOMAIN_SCAFFOLD,
  FORMAT,
  MANIFEST_NAME,
  RECIPE_GAP_REASON,
  SCHEMA_VERSION,
  UNFINISHED_MARKER,
  main,
  manifestEntry,
  moduleName,
  moduleSource,
  recipeGaps,
  censusRefusedCandidates,
  unservedDomainReport,
  PROPOSAL_PLAN_FORMAT
} from "./probe-recipe-scaffold.mjs";

const CLAIM = `claim:v1:sha256:${"9e82b17c".repeat(8)}`;
const OTHER_CLAIM = `claim:v1:sha256:${"1".repeat(64)}`;

const gap = (overrides = {}) => ({
  artifactCase: `artifact-case:${"f6c68044".repeat(8)}`,
  domain: "reads",
  export: "createReference",
  reason: RECIPE_GAP_REASON,
  semanticClaimId: CLAIM,
  ...overrides
});

function scratch(material) {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-scaffold-"));
  const plan = join(root, "plan.json");
  writeFileSync(plan, JSON.stringify(material));
  return { root, plan, corpus: join(root, "corpus") };
}

const silent = () => {};

describe("probe-recipe-scaffold selects candidates", () => {
  test("takes only the candidates a recipe would serve", () => {
    const gaps = recipeGaps({
      withheldClosures: [
        gap(),
        gap({ reason: "census refused: unknown accessor", semanticClaimId: OTHER_CLAIM }),
        gap({ reason: "veto did not complete: gate g1", semanticClaimId: OTHER_CLAIM })
      ]
    });
    // A census refusal has no proposed closure to veto, and an incomplete
    // veto already has a recipe whose author a scaffold must not overwrite.
    assert.deepEqual(
      gaps.map(entry => entry.semanticClaimId),
      [CLAIM]
    );
  });

  test("filters by domain when asked", () => {
    const material = {
      withheldClosures: [gap(), gap({ domain: "returns", semanticClaimId: OTHER_CLAIM })]
    };
    assert.deepEqual(
      recipeGaps(material, { domains: ["reads"] }).map(entry => entry.domain),
      ["reads"]
    );
    assert.equal(recipeGaps(material, { domains: [] }).length, 2);
  });

  test("refuses input that carries no withheld closures", () => {
    assert.throws(() => recipeGaps({}), /withheldClosures/);
  });
});

describe("probe-recipe-scaffold emits a corpus Rust can load", () => {
  test("names a module the workspace and the corpus test both accept", () => {
    const name = moduleName({
      specifier: "seroval",
      export: "createReference",
      domain: "reads",
      artifactCase: `artifact-case:${"f6c68044".repeat(8)}`
    });
    // `ecosystem-probe-recipes.test.mjs` enforces this shape, and the private
    // workspace copies each module by file name.
    assert.match(name, /^[a-z0-9][a-z0-9-]*\.mjs$/);
    assert.equal(name, "seroval-create-reference-reads-f6c68044.mjs");
  });

  test("distinguishes two artifact cases of one export", () => {
    const of = digest =>
      moduleName({
        specifier: "seroval",
        export: "createPlugin",
        domain: "returns",
        artifactCase: `artifact-case:${digest}`
      });
    // Production and development are two claims with two ids, and the private
    // workspace creates one file per recipe entry, so they cannot share a
    // module name.
    assert.notEqual(of("aaaaaaaa1111"), of("bbbbbbbb2222"));
  });

  test("binds the module and its manifest entry to one marker", () => {
    const source = moduleSource({
      specifier: "seroval",
      export: "createReference",
      domain: "reads",
      claimId: CLAIM,
      artifactCase: "artifact-case:f6c68044"
    });
    const entry = manifestEntry({
      claimId: CLAIM,
      module: "seroval-create-reference-reads-f6c68044.mjs",
      domain: "reads",
      importKind: "esm"
    });
    // The agreement no test on a hand-authored recipe can check: a marker the
    // module never emits makes the gate unmatchable, and an unmatchable gate
    // passes. Emitting both from one place is the point.
    assert.equal(entry.expectedEvent.marker, DOMAIN_SCAFFOLD.reads.marker);
    assert.ok(source.includes(JSON.stringify(DOMAIN_SCAFFOLD.reads.marker)));
    assert.notEqual(entry.coverageLimitations.length, 0);
  });

  test("obeys ADR 0006: no module hands session or harness to the package", () => {
    const source = moduleSource({
      specifier: "seroval",
      export: "createReference",
      domain: "reads",
      claimId: CLAIM,
      artifactCase: "artifact-case:f6c68044"
    });
    const code = source
      .replaceAll(/\/\*[\s\S]*?\*\//g, " ")
      .replaceAll(/(^|[^:])\/\/.*$/gm, "$1");
    assert.equal((code.match(/\bharness\b(?!\.emit\b)/g) ?? []).length, 1);
    assert.equal(/\bsession\b/.test(code.replaceAll("_session", "")), false);
  });

  test("writes a manifest with exactly the fields Rust reads", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);

    const manifest = JSON.parse(readFileSync(join(corpus, MANIFEST_NAME), "utf8"));
    assert.equal(manifest.format, FORMAT);
    assert.equal(manifest.schemaVersion, SCHEMA_VERSION);
    assert.deepEqual(Object.keys(manifest).sort(), [
      "format",
      "policy",
      "recipes",
      "schemaVersion"
    ]);
    assert.deepEqual(Object.keys(manifest.policy).sort(), [
      "maxEvents",
      "maxMacrotaskTurns",
      "maxMicrotaskTurns",
      "repeatRuns",
      "timeoutMillis"
    ]);
    assert.deepEqual(Object.keys(manifest.recipes[0]).sort(), [
      "claimId",
      "coverageLimitations",
      "dependencySpecifiers",
      "drain",
      "expectedEvent",
      "importKind",
      "module",
      "scenario"
    ]);
    // The claim id is copied, never derived: it is a content digest of the
    // normalized claim, and anything this script computed instead would
    // address nothing.
    assert.equal(manifest.recipes[0].claimId, CLAIM);
  });
});

describe("probe-recipe-scaffold cannot certify by omission", () => {
  test("emits a module that refuses to run until its observation is written", async () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);
    const [module] = readdirSync(corpus).filter(name => name.endsWith(".mjs"));
    const source = readFileSync(join(corpus, module), "utf8");

    assert.ok(source.includes(UNFINISHED_MARKER));
    // Executed rather than pattern-matched: what matters is that the session
    // *throws*, because a session that returns having emitted no marker is a
    // clean non-observation and the closure certifies on the census alone.
    // The import is stubbed out so this tests the guard and not the registry.
    const body = source.replace(/^import .*$/m, "const subjectModule = {};");
    const runner = new Function(
      "harness",
      `${body.replace("export async function runProbeSession", "return async function runProbeSession")}`
    );
    const emitted = [];
    await assert.rejects(
      () => runner()(undefined, { emit: event => emitted.push(event) }),
      /unfinished/
    );
    assert.deepEqual(emitted, []);
  });

  test("never overwrites a module or re-addresses a claim", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);
    const [module] = readdirSync(corpus).filter(name => name.endsWith(".mjs"));

    const finished = "// a finished recipe\nexport async function runProbeSession() {}\n";
    writeFileSync(join(corpus, module), finished);
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);

    assert.equal(readFileSync(join(corpus, module), "utf8"), finished);
    assert.equal(
      JSON.parse(readFileSync(join(corpus, MANIFEST_NAME), "utf8")).recipes.length,
      1
    );
  });

  test("emits nothing for a domain whose contradiction is not settled here", () => {
    const { plan, corpus } = scratch({
      withheldClosures: [gap({ domain: "cleanups", export: "onTeardown" })]
    });
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);
    // Mirrors `reviewed_observation`'s deliberate absence of a fallback arm: a
    // domain that inherited a neighbour's marker would be gated by a veto
    // watching for a contradiction nobody defined, and would pass.
    assert.throws(() => readdirSync(corpus), /ENOENT/);
  });

  test("leaves an existing corpus's policy alone", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    mkdirSync(corpus, { recursive: true });
    const policy = {
      repeatRuns: 3,
      timeoutMillis: 1000,
      maxMicrotaskTurns: 1,
      maxMacrotaskTurns: 1,
      maxEvents: 8
    };
    writeFileSync(
      join(corpus, MANIFEST_NAME),
      JSON.stringify({ format: FORMAT, schemaVersion: SCHEMA_VERSION, policy, recipes: [] })
    );
    main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent);
    assert.deepEqual(
      JSON.parse(readFileSync(join(corpus, MANIFEST_NAME), "utf8")).policy,
      policy
    );
  });

  test("refuses a manifest that is not a recipe corpus", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    mkdirSync(corpus, { recursive: true });
    writeFileSync(join(corpus, MANIFEST_NAME), JSON.stringify({ format: "something-else" }));
    assert.throws(
      () => main(["--plan", plan, "--corpus", corpus, "--specifier", "seroval"], silent),
      /refusing to rewrite/
    );
  });

  test("requires the bare specifier when the input does not name the package", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    assert.throws(() => main(["--plan", plan, "--corpus", corpus], silent), /--specifier/);
  });

  test("takes the specifier from a graph-lane record when it has one", () => {
    const { plan, corpus } = scratch({
      withheldClosures: [gap({ node: { package: "seroval", version: "1.5.6" } })]
    });
    main(["--plan", plan, "--corpus", corpus], silent);
    const [module] = readdirSync(corpus).filter(name => name.endsWith(".mjs"));
    assert.match(readFileSync(join(corpus, module), "utf8"), /from "seroval"/);
  });

  test("says why an asked-for domain got nothing", () => {
    // "Nothing to do" has two causes an author cannot tell apart from
    // silence, and on `seroval@1.5.6` both occur at once: `creates` is
    // withheld on a census refusal no recipe addresses, and `reads` has no
    // candidate at all because the bundle names `Object.defineProperty` and
    // the closure hazard withdraws the domain before planning.
    const material = {
      withheldClosures: [
        gap({ domain: "creates", export: "resolvePlugins", reason: "census refused: uncensused invoking form" })
      ]
    };
    assert.deepEqual(unservedDomainReport(material, ["creates"]).length, 2);
    assert.match(unservedDomainReport(material, ["creates"])[1], /census refused/);

    const reads = unservedDomainReport(material, ["reads"]).join(" ");
    assert.match(reads, /no candidate at all/);
    assert.match(reads, /closure hazard withdraws a domain before/);
    // Nothing is claimed when no domain was asked for: the report answers a
    // question about a named domain and inventing one would be noise.
    assert.deepEqual(unservedDomainReport(material, []), []);
  });

  test("separates a domain the generator never proposed from one withdrawn later", () => {
    // The distinction `@solid-primitives/memo@2.0.0-next.2` forced. Its
    // certification is clean -- certified, no refusal, nothing withheld --
    // and its contract states nothing at all, because all three of its
    // imports are unaccepted external dependencies. Nothing downstream of
    // the generator can be the problem there, and a recipe least of all.
    const claim = (exported, domain) => ({ subject: { export: exported, path: { kind: "call", domain } } });
    const material = { withheldClosures: [] };
    const generatorNever = {
      format: PROPOSAL_PLAN_FORMAT,
      closureCandidates: [],
      unresolvedClaims: [claim("createLatest", "reads"), claim("createReducer", "reads")]
    };
    const proposedThenLost = {
      format: PROPOSAL_PLAN_FORMAT,
      closureCandidates: [claim("createLatest", "reads")],
      unresolvedClaims: []
    };

    const never = unservedDomainReport(material, ["reads"], generatorNever).join(" ");
    assert.match(never, /generator proposed no reads closure/);
    assert.match(never, /unresolved for 2/);

    const lost = unservedDomainReport(material, ["reads"], proposedThenLost).join(" ");
    assert.match(lost, /proposed 1 reads closure candidate/);
    assert.match(lost, /lost after proposal/);

    // Without the sidecar the tool says what would answer the question rather
    // than guessing between the two.
    assert.match(unservedDomainReport(material, ["reads"]).join(" "), /--proposal-plan/);
  });

  test("--dry-run writes nothing", () => {
    const { plan, corpus } = scratch({ withheldClosures: [gap()] });
    const lines = [];
    main(
      ["--plan", plan, "--corpus", corpus, "--specifier", "seroval", "--dry-run"],
      line => lines.push(line)
    );
    assert.equal(
      lines.some(line => line.startsWith("would emit")),
      true
    );
    assert.throws(() => readdirSync(corpus));
  });
});

describe("census-refused candidates", () => {
  // The two-pass workflow's whole payload. A candidate withheld as
  // `no recipe in corpus` is weakened out of the plan before its demands are
  // discharged, so its census never runs and a refusal underneath stays
  // masked; a *throwing* scaffold keeps it in the plan and unmasks it. What
  // the second pass over that audit has to do is say which candidates nobody
  // should finish -- measured at 24 of 45 on one real artifact case.
  const audit = reasons => ({
    withheldClosures: reasons.map(([export_, domain, reason]) => ({
      artifactCase: "artifact-case:x",
      export: export_,
      domain,
      semanticClaimId: CLAIM,
      reason
    }))
  });

  test("names the candidates no recipe serves, and only those", () => {
    const material = audit([
      ["arrayEquals", "reads", "census refused: a reads closure candidate must enumerate no operation, but the proposal names 1"],
      ["compare", "reads", "census refused: reads-census premise required: the coercion form"],
      ["clamp", "reads", "veto did not complete: gate 7"],
      ["trueFn", "reads", RECIPE_GAP_REASON]
    ]);
    assert.deepEqual(
      censusRefusedCandidates(material).map(entry => entry.export),
      ["arrayEquals", "compare"]
    );
    // The control: a gap is not a refusal, and neither is an incomplete gate.
    // Confusing either for one would tell an author to abandon a candidate
    // that is exactly the one worth finishing.
    assert.deepEqual(
      recipeGaps(material).map(entry => entry.export),
      ["trueFn"]
    );
  });

  test("filters by domain and tolerates material with no withheld array", () => {
    const material = audit([
      ["a", "reads", "census refused: x"],
      ["b", "creates", "census refused: y"]
    ]);
    assert.deepEqual(
      censusRefusedCandidates(material, { domains: ["creates"] }).map(entry => entry.export),
      ["b"]
    );
    assert.deepEqual(censusRefusedCandidates({}), []);
    assert.deepEqual(censusRefusedCandidates(null), []);
  });
});
