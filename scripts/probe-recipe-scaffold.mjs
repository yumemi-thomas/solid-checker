#!/usr/bin/env node

// Emits hand-recipe scaffolds for the closure candidates a certification
// withheld for want of one.
//
// ~~~sh
// bun scripts/probe-recipe-scaffold.mjs \
//   --plan /tmp/run/certification-plan-0.json \
//   --corpus scripts/ecosystem-benchmark/probe-recipes \
//   --domain reads --specifier seroval
// ~~~
//
// Add `--proposal-plan <file>.proposal.json` (the sidecar `contract generate`
// writes) when a domain yields no scaffold: it separates "the generator never
// proposed a closure" from "one was proposed and later withdrawn", which are
// different problems and only one of them is ever a recipe's.
//
// ## What is mechanical, and what is not
//
// A recipe is three things: a claim id, a manifest entry, and an observation.
// The first two are transcription — the id is a content digest an author has
// to copy out of a run's material, the entry has to agree with the module on
// a marker nothing checks — and this emits both. The observation is not, and
// for `reads` it is *provably* not: an unenumerated read is a read of a
// source the export **owns**, which is exactly the half a synthesized veto
// cannot instrument (`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
// § 6). `reviewed_observation` in
// rust/crates/solid-facts-backend/src/contract_certification/synthesized_vetoes.rs
// registers no observation for the domain for that reason, and this script
// does not invent one.
//
// ## Why an unfinished scaffold throws
//
// This is the property that makes emitting anything at all safe. A recipe
// that runs to completion without emitting its marker is a *clean
// non-observation*: `evaluate_runtime_probes` treats it as the mandatory veto
// passing, and the closure certifies on the census alone
// (rust/crates/solid-facts-backend/src/runtime_probes.rs, `CleanNonObservation`).
// So a scaffold that merely called the export would turn "nobody wrote the
// observation yet" into "nothing contradicted the closure" — silently, and on
// every candidate at once.
//
// Each emitted module therefore refuses to run until an author deletes its
// `UNFINISHED` guard. A throw makes the gate incomplete, and an incomplete
// gate withholds the candidate (`WITHHELD_CLOSURE_VETO_INCOMPLETE_PREFIX`)
// exactly as having no recipe at all does — with the throw's own text carried
// into the withheld record to say why. The failure mode of an unfinished
// scaffold is the state it was generated from, never a weaker one.

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const MANIFEST_NAME = "recipes.json";
export const FORMAT = "solid-checker-probe-recipe-corpus";
export const SCHEMA_VERSION = 1;

/// The reason a candidate carries when a recipe is the missing piece.
///
/// Deliberately the only one served. `census refused: …` is a candidate no
/// recipe can help — the implementation census could not decide it, so there
/// is no proposed closure to veto. `veto did not complete: gate …` already
/// has a recipe, and overwriting it with a scaffold would destroy the
/// author's work to fix a runtime failure a scaffold does not address.
export const RECIPE_GAP_REASON = "no recipe in corpus";

/// The default probe policy for a corpus this script creates.
///
/// Matches the checked-in ecosystem corpus. An existing manifest's policy is
/// never rewritten: it is a property of the corpus its author chose, and a
/// scaffold has no standing to widen a timeout or an event budget.
const DEFAULT_POLICY = {
  repeatRuns: 2,
  timeoutMillis: 60000,
  maxMicrotaskTurns: 4,
  maxMacrotaskTurns: 1,
  maxEvents: 64
};

/// Per-domain scaffolding, for the domains whose contradiction has a settled
/// spelling in this repository — and for no others.
///
/// There is deliberately no fallback arm, mirroring `reviewed_observation` on
/// the Rust side and for the same reason. A domain added to
/// `ClaimDomain::PROPOSABLE` would otherwise inherit whichever marker sat
/// closest to hand, and a manifest entry whose marker names a contradiction
/// nobody defined is a gate that cannot fire: the recipe would pass by
/// construction. An unscaffolded domain is reported and skipped.
///
/// `marker` binds the module and its manifest entry together. That agreement
/// is the one thing a hand author can get wrong with no test catching it —
/// the gate simply never matches, and the veto passes — so it is emitted from
/// one place into both.
export const DOMAIN_SCAFFOLD = {
  reads: {
    marker: "read-operation",
    contradiction:
      "the export observed the current value of a reactive source this package owns",
    // The exactness statement each recipe has to carry. `reads` is the domain
    // where it does the most work: the recipe is only as good as its author's
    // knowledge of what the closure owns.
    limitation:
      "Exact for this package only: the observation must cover every reactive-shaped source the closure owns, which no synthesized veto can know",
    obligations: [
      "identify every reactive-shaped source this *closure* owns -- a proxy, a getter, an accessor it built. A read reached through a caller-supplied value is the caller's (ADR 0034) and is not this domain's business.",
      "make each of them observable from the recipe, and assert your own observation fired, so a package edit that removes the source fails the recipe instead of silently passing the gate.",
      "emit the marker only when a sample actually observed one of them."
    ]
  },
  returns: {
    marker: "return-outside-identity",
    contradiction: "a call returned something other than what the census claims it returns",
    limitation:
      "Finite samples can contradict the claimed return identity but cannot establish it; the authenticated implementation census remains the proof",
    obligations: [
      "read the census's `output` claim for this export and sample the shapes that could falsify it.",
      "cross the primitive/object boundary where the implementation admits it -- a plain `Map`-backed registry accepts primitives a `WeakMap` would reject."
    ]
  },
  creates: {
    marker: "create-operation",
    contradiction: "the call created an owned resource the closure claims it does not create",
    limitation:
      "Own-property additions to globalThis during the call window are not an exact observation of a create operation",
    obligations: [
      "decide what an owned creation would be observable as for this package, and observe that rather than the globalThis proxy the synthesized veto uses."
    ]
  }
};

const IDENTIFIER_SAFE = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

/// A single lower-case path component Rust and the corpus test both accept.
///
/// `^[a-z0-9][a-z0-9-]*\.mjs$` is what `ecosystem-probe-recipes.test.mjs`
/// enforces and what the private workspace copies by file name, so a slug
/// that cannot satisfy it is a refusal here rather than a corpus that fails
/// its own gate later.
export function slug(value) {
  return String(value)
    // Before lower-casing, so `createReference` reads as `create-reference`
    // rather than as one word: these names go in front of a human choosing
    // which scaffold to finish.
    .replaceAll(/([a-z0-9])([A-Z])/g, "$1-$2")
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .replace(/-+$/, "");
}

/// The marker a module must still carry to be an unfinished scaffold.
///
/// Exported so the corpus test can refuse one that reached a commit: a
/// scaffold is a working state, and a checked-in corpus holding one spends a
/// worker launch per gate to arrive at the withholding it started from.
export const UNFINISHED_MARKER = "const UNFINISHED = true;";

/// The module file name for one withheld candidate.
///
/// The artifact case is in the name because a candidate is per case, not per
/// export: `seroval`'s production and development `createPlugin` are two
/// claims with two ids, and the private workspace creates one file per recipe
/// entry, so two entries cannot share a module.
export function moduleName({ specifier, export: exported, domain, artifactCase }) {
  const caseDigest = String(artifactCase ?? "").match(/[0-9a-f]{8,}/)?.[0] ?? "";
  const parts = [slug(specifier), slug(exported), slug(domain), caseDigest.slice(0, 8)].filter(
    part => part.length > 0
  );
  const name = `${parts.join("-")}.mjs`;
  if (!/^[a-z0-9][a-z0-9-]*\.mjs$/.test(name)) {
    throw new Error(`cannot derive a corpus module name from ${specifier}.${exported} (${domain})`);
  }
  return name;
}

function commentBlock(lines, indent = "") {
  return lines.map(line => (line.length ? `${indent}// ${line}` : `${indent}//`)).join("\n");
}

/// The scaffold module's source.
///
/// `harness` is named exactly once outside an `emit` call — the parameter
/// itself — because ADR 0006's one unenforceable obligation is that a recipe
/// never hand `session` or `harness` to the package, and the corpus test
/// counts the mentions.
export function moduleSource({ specifier, export: exported, domain, claimId, artifactCase }) {
  const scaffold = DOMAIN_SCAFFOLD[domain];
  const accessor = IDENTIFIER_SAFE.test(exported)
    ? `subjectModule.${exported}`
    : `subjectModule[${JSON.stringify(exported)}]`;
  return `${commentBlock([
    `SCAFFOLD -- not a veto yet. Generated by scripts/probe-recipe-scaffold.mjs.`,
    ``,
    `Addresses the \`${domain}: []\` claim of \`${exported}\`.`,
    ``,
    `  claim         ${claimId}`,
    `  artifact case ${artifactCase}`,
    ``,
    `The contradiction to emit: ${scaffold.contradiction}.`,
    ``,
    `This module throws until the \`UNFINISHED\` guard below is deleted, and`,
    `that is deliberate. A recipe that runs to completion without emitting its`,
    `marker is a clean non-observation: the mandatory veto passes and the`,
    `closure certifies on the implementation census alone. An unfinished`,
    `scaffold must therefore never complete -- a throw makes the gate`,
    `incomplete, which withholds the candidate exactly as having no recipe`,
    `does.`,
    ``,
    `To finish it:`,
    ...scaffold.obligations.flatMap((obligation, index) =>
      `  ${index + 1}. ${obligation}`.match(/.{1,72}(\s|$)/g).map((line, part) =>
        part === 0 ? line.trimEnd() : `     ${line.trim()}`
      )
    ),
    `  ${scaffold.obligations.length + 1}. replace the placeholder samples with arguments that exercise`,
    `     the export.`,
    `  ${scaffold.obligations.length + 2}. delete \`UNFINISHED\` and its guard, and rewrite this header to`,
    `     state what the recipe observes and what it cannot.`,
    ``,
    `Never hand \`session\` or \`harness\` to the package under test (ADR 0006).`
  ])}
import * as subjectModule from ${JSON.stringify(specifier)};

const subject = ${accessor};

// TODO: the argument tuples this export should be called with. One empty
// tuple is a placeholder, not a sample set.
const samples = [[]];

// Delete this, and the guard below, once the observation above is written.
${UNFINISHED_MARKER}

export async function runProbeSession(_session, harness) {
  if (UNFINISHED) {
    throw new Error(
      "probe recipe scaffold for ${claimId} is unfinished: " +
        "write the observation and delete its UNFINISHED guard"
    );
  }
  if (typeof subject !== "function") {
    throw new Error("the export is not callable in this realm");
  }
  for (const args of samples) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    subject(...args);
    // TODO: emit only when this sample contradicted the closure.
    // harness.emit({ marker: ${JSON.stringify(scaffold.marker)}, kind: "call", phase: "enter" });
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
}
`;
}

/// The manifest entry, carrying the same marker the module was emitted with.
export function manifestEntry({ claimId, module, domain, importKind }) {
  const scaffold = DOMAIN_SCAFFOLD[domain];
  return {
    claimId,
    module,
    importKind,
    dependencySpecifiers: [],
    scenario: "operation",
    expectedEvent: { marker: scaffold.marker, class: "call" },
    drain: [{ kind: "microtasks", maxTurns: 1 }],
    coverageLimitations: [
      "SCAFFOLD: this recipe throws until its observation is written, so its gate is incomplete and its candidate stays withheld",
      scaffold.limitation
    ]
  };
}

/// Every candidate in `material` a recipe would serve.
///
/// Accepts a certification plan (`--plan-contract-certification` output) or a
/// certification audit: both carry `withheldClosures` arrays of the same
/// `WithheldClosure` shape, and neither carries anything else this needs.
export function recipeGaps(material, { domains } = {}) {
  const withheld = material?.withheldClosures;
  if (!Array.isArray(withheld)) {
    throw new Error(
      "the input carries no withheldClosures array; pass a certification plan or audit"
    );
  }
  return withheld
    .filter(entry => entry && typeof entry === "object")
    .filter(entry => entry.reason === RECIPE_GAP_REASON)
    .filter(entry => !domains?.length || domains.includes(entry.domain))
    .filter(entry => typeof entry.semanticClaimId === "string" && typeof entry.export === "string");
}

export const PROPOSAL_PLAN_FORMAT = "solid-checker-contract-proposal-plan";

/// Which side of the proposal/certification line a domain was lost on.
///
/// A candidate reaches a gate only if the *generator* proposed a closure for
/// it. `contract generate`'s `.proposal.json` sidecar separates the two
/// outcomes by name: `closureCandidates` is what it proposed, and
/// `unresolvedClaims` is every (export, domain) it could not decide. Reading
/// it turns "no candidate" from a dead end into a stage.
///
/// Measured on `@solid-primitives/memo@2.0.0-next.2`, where the sidecar is
/// the whole answer: 70 unresolved claims, seven exports by ten domains, no
/// closure candidate at all, because the package's three imports are
/// `unaccepted-external-dependency`
/// (`2026-09-10-reads-veto-observation-design.md` § 24).
export function proposalStageReport(domain, plan) {
  if (plan?.format !== PROPOSAL_PLAN_FORMAT) {
    return [
      "    Pass --proposal-plan <file>.proposal.json to learn whether the generator",
      "    proposed a closure for it at all."
    ];
  }
  const candidates = (plan.closureCandidates ?? []).filter(
    candidate => candidate?.subject?.path?.domain === domain
  );
  if (candidates.length) {
    return [
      `    The generator proposed ${candidates.length} ${domain} closure candidate(s), so the`,
      "    domain was lost after proposal -- a closure hazard or a withdrawal, not the",
      "    generator."
    ];
  }
  const unresolved = (plan.unresolvedClaims ?? []).filter(
    claim => claim?.subject?.path?.domain === domain
  );
  if (!unresolved.length) {
    return [`    The generator's plan names no ${domain} claim at all.`];
  }
  const exports = [...new Set(unresolved.map(claim => claim.subject.export))].sort();
  return [
    `    The generator proposed no ${domain} closure: it is unresolved for ${exports.length}`,
    `    export(s) (${exports.slice(0, 6).join(", ")}${exports.length > 6 ? ", …" : ""}).`,
    "    No recipe applies before the generator can decide the domain; read the",
    "    .refusals.json sidecar's declinedClosures and certificationInputs for why."
  ];
}

/// Why an asked-for domain got nothing, when the answer is in the material.
///
/// "Nothing to do" has two very different causes and an author cannot tell
/// them apart from silence. Either the domain is withheld for a reason a
/// recipe does not address — a census refusal has no proposed closure to veto
/// — or the domain has **no candidate at all**, which is what a closure
/// hazard produces: `runtime-accessor-installation` withdraws `reads` from
/// every export in a file before planning, so the domain never reaches a
/// gate and nothing about it appears here.
///
/// Measured on `seroval@1.5.6`, where the second case is the whole story: the
/// package's certified contract closes `creates` and `returns` and never
/// states `reads`, because the bundle names `Object.defineProperty` (see
/// `2026-09-10-reads-veto-observation-design.md` § 23).
export function unservedDomainReport(material, domains, plan) {
  if (!domains?.length) return [];
  const withheld = material.withheldClosures.filter(entry => entry && typeof entry === "object");
  return domains.flatMap(domain => {
    const mentions = withheld.filter(entry => entry.domain === domain);
    if (!mentions.length) {
      return [
        `  ${domain}: no candidate at all in this material. Either the domain is already`,
        `    closed, or none was planned -- a closure hazard withdraws a domain before`,
        `    planning, and no recipe can create a candidate.`,
        ...proposalStageReport(domain, plan)
      ];
    }
    const reasons = [...new Set(mentions.map(entry => entry.reason))];
    return [
      `  ${domain}: withheld, but for ${reasons.length === 1 ? "a reason" : "reasons"} no recipe addresses:`,
      ...reasons.map(reason => `    ${reason.length > 120 ? `${reason.slice(0, 117)}...` : reason}`)
    ];
  });
}

function readManifest(path) {
  if (!existsSync(path)) {
    return { format: FORMAT, schemaVersion: SCHEMA_VERSION, policy: DEFAULT_POLICY, recipes: [] };
  }
  const manifest = JSON.parse(readFileSync(path, "utf8"));
  if (manifest.format !== FORMAT || manifest.schemaVersion !== SCHEMA_VERSION) {
    throw new Error(
      `${path} is not a ${FORMAT} schemaVersion ${SCHEMA_VERSION} manifest; refusing to rewrite it`
    );
  }
  if (!Array.isArray(manifest.recipes)) throw new Error(`${path} has no recipes array`);
  return manifest;
}

/// Plans the emission without performing it, so `--dry-run` and the write
/// path answer the same question.
///
/// A claim already addressed by the manifest is skipped rather than
/// rewritten, and so is a module that exists on disk under the derived name.
/// Both are the same rule: this never overwrites a recipe, because the only
/// recipe worth having is one somebody finished.
export function planEmission({ gaps, manifest, existingModules, specifier, importKind }) {
  const addressed = new Set(manifest.recipes.map(recipe => recipe.claimId));
  const present = new Set(existingModules);
  const emit = [];
  const skipped = [];
  for (const gap of gaps) {
    const scaffold = DOMAIN_SCAFFOLD[gap.domain];
    if (!scaffold) {
      skipped.push({
        gap,
        why: `no scaffold is defined for the ${gap.domain} domain; its contradiction has not been settled here`
      });
      continue;
    }
    if (addressed.has(gap.semanticClaimId)) {
      skipped.push({ gap, why: "the manifest already addresses this claim" });
      continue;
    }
    const module = moduleName({
      specifier,
      export: gap.export,
      domain: gap.domain,
      artifactCase: gap.artifactCase
    });
    if (present.has(module)) {
      skipped.push({ gap, why: `${module} already exists and is not overwritten` });
      continue;
    }
    present.add(module);
    emit.push({
      gap,
      module,
      source: moduleSource({
        specifier,
        export: gap.export,
        domain: gap.domain,
        claimId: gap.semanticClaimId,
        artifactCase: gap.artifactCase ?? "(unnamed)"
      }),
      entry: manifestEntry({
        claimId: gap.semanticClaimId,
        module,
        domain: gap.domain,
        importKind
      })
    });
  }
  return { emit, skipped };
}

function parseArguments(argv) {
  const options = { domains: [], importKind: "esm", dryRun: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const next = () => {
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error(`${argument} needs a value`);
      }
      index += 1;
      return value;
    };
    switch (argument) {
      case "--plan":
      case "--audit":
        options.input = next();
        break;
      case "--corpus":
        options.corpus = next();
        break;
      case "--proposal-plan":
        options.proposalPlan = next();
        break;
      case "--domain":
        options.domains.push(next());
        break;
      case "--specifier":
        options.specifier = next();
        break;
      case "--import-kind":
        options.importKind = next();
        break;
      case "--dry-run":
        options.dryRun = true;
        break;
      default:
        throw new Error(`unknown argument ${argument}`);
    }
  }
  if (!options.input) throw new Error("--plan (or --audit) is required");
  if (!options.corpus) throw new Error("--corpus is required");
  if (!["esm", "require"].includes(options.importKind)) {
    throw new Error("--import-kind must be esm or require");
  }
  return options;
}

export function main(argv = process.argv.slice(2), log = console.log) {
  const options = parseArguments(argv);
  const material = JSON.parse(readFileSync(options.input, "utf8"));
  const gaps = recipeGaps(material, { domains: options.domains });
  if (!gaps.length) {
    log(`no candidate in ${basename(options.input)} is withheld for want of a recipe`);
    const plan = options.proposalPlan
      ? JSON.parse(readFileSync(options.proposalPlan, "utf8"))
      : undefined;
    for (const line of unservedDomainReport(material, options.domains, plan)) log(line);
    return 0;
  }
  // The bare specifier every emitted module imports. Graph lanes name the
  // package in the withheld record; a single-package run does not, and
  // guessing it from a path would produce a module that resolves to nothing.
  const specifier =
    options.specifier ?? gaps.map(gap => gap.node?.package).find(name => typeof name === "string");
  if (!specifier) {
    throw new Error(
      "the input does not name the package, so --specifier is required: it is the bare specifier every emitted module imports"
    );
  }

  const corpus = resolve(options.corpus);
  const manifestPath = join(corpus, MANIFEST_NAME);
  const manifest = readManifest(manifestPath);
  const existingModules = existsSync(corpus)
    ? readdirSync(corpus).filter(name => name.endsWith(".mjs"))
    : [];
  const { emit, skipped } = planEmission({
    gaps,
    manifest,
    existingModules,
    specifier,
    importKind: options.importKind
  });

  for (const { gap, why } of skipped) {
    log(`skipped  ${gap.domain} ${gap.export}: ${why}`);
  }
  if (!emit.length) {
    log("nothing to emit");
    return 0;
  }
  if (options.dryRun) {
    for (const { gap, module } of emit) {
      log(`would emit  ${module}  (${gap.domain} ${gap.export} ${gap.semanticClaimId})`);
    }
    return 0;
  }

  mkdirSync(corpus, { recursive: true });
  for (const { module, source } of emit) {
    writeFileSync(join(corpus, module), source);
  }
  manifest.recipes.push(...emit.map(planned => planned.entry));
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  for (const { gap, module } of emit) {
    log(`emitted  ${module}  (${gap.domain} ${gap.export})`);
  }
  log(
    `${emit.length} scaffold(s) in ${corpus}. Each throws until its observation is written, ` +
      "so every one of these candidates stays withheld until somebody finishes it."
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main();
  } catch (error) {
    console.error(`probe-recipe-scaffold: ${error.message}`);
    process.exitCode = 1;
  }
}
