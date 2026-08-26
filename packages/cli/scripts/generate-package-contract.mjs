import { createHash, randomUUID } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { runNative } from "../bin/launcher.mjs";
import { expandContract, normalizeContract } from "./contract-document.mjs";
import {
  collectReviewItems,
  isUnknownClaim,
  previousContractPath,
  probeReportPath,
  renderReviewPlan,
  renderReviewPlanDocument,
  reviewPlanJsonPath,
  reviewPlanPath,
  reviewStatePath,
  verifyReportPath
} from "./contract-review-plan.mjs";
import {
  createModuleResolver,
  noteFor,
  openDynamicImportReachability,
  runtimeModuleClosure
} from "./runtime-module-closure.mjs";

/// Who produced a review plan, so a transfer can refuse to carry a review
/// across a generator whose enumeration or summarization changed underneath it.
let generatorIdentityCache;
function generatorIdentity() {
  if (generatorIdentityCache) return generatorIdentityCache;
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8")
  );
  generatorIdentityCache = `${manifest.name}@${manifest.version}`;
  return generatorIdentityCache;
}

export const packageContractHelp = `Usage:
  solid-checker contract check [OPTIONS]
  solid-checker contract generate [OPTIONS]
  solid-checker contract probe <CONTRACT> [OPTIONS]
  solid-checker contract verify <CONTRACT> [OPTIONS]
  solid-checker contract review <CONTRACT> [OPTIONS]
  solid-checker contract review <CONTRACT> --transfer-from <CONTRACT>

contract check reports every imported Solid package whose contract is missing,
unverified, or stale, and prints the command that resolves each one. It exits 1
when any package needs action.

Options:
  --project <PATH>       TypeScript project (default: tsconfig.json)
  --format <text|json>   Output format (default: text)
  --contract <FILE>      Contract override to classify (repeatable)

contract generate writes a package's solid-reactivity.json, a review checklist
(<contract>.review.md), and the machine-readable review plan contract review
resolves (<contract>.review.json). Generation never promotes inferred claims:
the plan still has to be reviewed. Regenerating over a contract that already has
a review state keeps the previous contract, plan and state as
<contract>.previous.json and its siblings, and prints the transfer command that
carries that review forward.

Options:
  --package-root <DIR>   Package root (default: current directory)
  --output <FILE>        Contract output path
  --entrypoint <SUBPATH> Generate one exact subpath (repeatable)
  --conditions <LIST>    Resolve conditional exports, e.g. browser,import
  --contract <FILE>      Dependency contract (repeatable)
  -h, --help             Show this help

contract generate --missing generates a project-owned contract for every
package the contract report reports as missing. It never regenerates a
contract that already exists. It takes only:

  --project <PATH>       TypeScript project (default: tsconfig.json)
  --format <text|json>   Output format (default: text)

contract probe executes a generated contract's drivable claims against the
installed package and writes <contract>.probe.json. It runs the package's code
in a child process -- run it where you would run that package's own test suite.
It confirms claims that already exist and never writes a new one. Probing comes
between generate and review; run solid-checker contract probe --help for its
options.

contract verify promotes a probed contract to verified evidence mechanically:
no human decision, reproducible by anyone with the same installed artifact.
Every positive claim that is neither statically proven nor probed is converted
to the {"status": "unknown"} sentinel first -- losing information visibly, into
<contract>.verify.json, rather than guessing. It refuses on a failed probe, an
incompleteness finding, a closure note, a probe report that is not the report
for these exact bytes, or a review already under way. Run solid-checker
contract verify --help for its options. A verified contract is not a reviewed
one; the reviewed tier resolves what the machine could not.

contract review lists a generated contract's review plan, records one decision
per item in <contract>.review-state.json, and promotes the contract to reviewed
evidence once every item is decided. With no options it is a gate: it exits 1
while any item is open or stale, or any unknown claim remains.

  --resolve <ID=DECISION>  Record one decision (repeatable). Decisions are
                           confirm (the generated claim is correct as
                           generated; never for an unknown claim), absent
                           (certify the negative -- the only way to delete an
                           unknown claim field or to certify that an export
                           invokes no caller-supplied callback), and
                           resolved-by-edit (the contract was hand-edited to
                           carry the audited value; accepted only once the
                           contract no longer raises the item)
  --answers <FILE>         A JSON {id: decision} map of the same decisions
  --note <TEXT>            Note recorded with a single --resolve
  --transfer-from <FILE>   Carry a previous review's resolutions onto this
                           regenerated contract, so an upgrade costs only the
                           diff. Usually <contract>.previous.json, which the
                           regeneration wrote. A resolution transfers only for
                           an entrypoint whose runtime module closure is
                           byte-identical to the one the previous review was
                           recorded against, and only when the item still says
                           exactly what the reviewer answered; every other item
                           stays open. Run it before recording any decision
                           against the new contract.
  --promote reviewed       Apply the absent deletions and set the contract's
                           evidence to reviewed. Refused while any item is
                           open or stale, or any unknown claim is undecided.
                           trusted and attested mean an out-of-band trust
                           decision and a verifier-produced release identity
                           and are never written here; verified is a
                           mechanical check with no decision in it, so it has
                           its own command, solid-checker contract verify.
`;

/// A refusal the generator *decided*, as distinct from an error it merely hit.
///
/// The per-entrypoint catch below turns a refusal into a listed
/// refused-entrypoint entry and keeps generating the other entrypoints, which
/// is only sound when the thrown thing is a fail-closed decision this file (or
/// the native checker's own contract emitter) made deliberately. An
/// unclassified `Error` -- a generator bug, an ENOENT, a malformed JSON
/// document, a panicked or handshake-mismatched native process -- proves
/// nothing about the entrypoint, and recording it as "refused and omitted"
/// would ship a 1-of-20-entrypoint contract with exit 0 where the run used to
/// fail loudly. Those propagate.
export class GenerationRefusal extends Error {
  constructor(message) {
    super(message);
    this.name = "GenerationRefusal";
    // A marker as well as a class: a refusal raised inside a recursively
    // generated dependency contract crosses back through this module's own
    // frames, and an identity check that survives duplicate module instances
    // is cheaper to reason about than one that does not.
    this.generationRefusal = true;
  }
}

function refuse(message) {
  return new GenerationRefusal(message);
}

export function isGenerationRefusal(error) {
  return error instanceof GenerationRefusal || error?.generationRefusal === true;
}

function parseArguments(arguments_) {
  const options = {
    packageRoot: process.cwd(),
    output: "",
    entrypoints: [],
    contracts: [],
    conditions: []
  };
  for (let index = 0; index < arguments_.length; index++) {
    const argument = arguments_[index];
    const separator = argument.indexOf("=");
    const key = separator === -1 ? argument : argument.slice(0, separator);
    const inline = separator === -1 ? undefined : argument.slice(separator + 1);
    const value = inline ?? arguments_[++index];
    if (!argument.startsWith("--") || value === undefined) {
      throw new Error(
        "usage: solid-checker contract generate [--package-root DIR] [--output FILE] " +
          "[--entrypoint SUBPATH] [--conditions LIST] [--contract FILE]"
      );
    }
    switch (key) {
      case "--package-root":
        options.packageRoot = value;
        break;
      case "--output":
        options.output = value;
        break;
      case "--entrypoint":
        options.entrypoints.push(value);
        break;
      case "--contract":
        options.contracts.push(value);
        break;
      case "--conditions":
        options.conditions.push(
          ...value
            .split(",")
            .map(condition => condition.trim())
            .filter(Boolean)
        );
        break;
      default:
        throw new Error(`unknown contract generation argument ${key}`);
    }
  }
  return options;
}

function runtimeLeaf(target) {
  if (typeof target !== "string" || target.endsWith(".d.ts")) return false;
  return [".js", ".jsx", ".mjs", ".ts", ".tsx", ".mts"].includes(extname(target));
}

function collectRuntimeLeaves(target, conditions = []) {
  if (typeof target === "string") {
    return runtimeLeaf(target) ? [{ target, conditions }] : [];
  }
  if (Array.isArray(target)) {
    return target.flatMap(value => collectRuntimeLeaves(value, conditions));
  }
  if (!target || typeof target !== "object") return [];
  return Object.entries(target).flatMap(([condition, value]) => {
    if (condition === "types" || condition === "require") return [];
    return collectRuntimeLeaves(
      value,
      condition === "default" ? conditions : [...conditions, condition]
    );
  });
}

function stringTargets(target) {
  if (typeof target === "string") return [target];
  if (Array.isArray(target)) return target.flatMap(stringTargets);
  if (!target || typeof target !== "object") return [];
  return Object.values(target).flatMap(stringTargets);
}

function resolveRuntimeLeaf(target, active, conditions = []) {
  if (typeof target === "string") {
    return runtimeLeaf(target) ? [{ target, conditions }] : [];
  }
  if (Array.isArray(target)) {
    for (const value of target) {
      const resolved = resolveRuntimeLeaf(value, active, conditions);
      if (resolved.length) return resolved;
    }
    return [];
  }
  if (!target || typeof target !== "object") return [];
  for (const [condition, value] of Object.entries(target)) {
    if (condition === "types" || condition === "require") continue;
    if (condition === "default" || active.has(condition)) {
      const resolved = resolveRuntimeLeaf(
        value,
        active,
        condition === "default" ? conditions : [...conditions, condition]
      );
      if (resolved.length) return resolved;
    }
  }
  return [];
}

function walkFiles(directory, root = directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (
      entry.name === "node_modules" ||
      entry.name === ".git" ||
      entry.name.startsWith(".solid-checker-contract-")
    ) {
      continue;
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walkFiles(path, root, files);
    else if (entry.isFile()) files.push(`./${relative(root, path).replaceAll(sep, "/")}`);
  }
  return files;
}

/// The runtime modules an entry file pulls in, and why that set may be short.
///
/// The walk has one remaining consumer of its *file list*: the TypeScript
/// project `analyzeTarget` seeds. It stays the seeder because a published ESM
/// barrel's `.js` specifiers resolve to adjacent `.d.ts` files when only the
/// entrypoint is seeded, so letting TypeScript discover the graph from the entry
/// alone would make the analysis read declarations where it now reads runtime
/// bytes -- see the pinned rationale in `analyzeTarget`.
///
/// What the walk no longer decides is the *record*. The unpinned-module count
/// and the per-entrypoint hash set a review transfers against now come from the
/// analyzing program's own module inventory (`attestedClosure`), and the walk's
/// problems are reconciled against that inventory rather than quoted blind. The
/// walk is therefore the seed and the attestation is both the record and the
/// seed's verifier: a module one side names and the other does not is a named,
/// fail-closed note, never a silent reconciliation.
function closureOf(packageRoot, resolver, entryFile, excludedFiles = new Set()) {
  return runtimeModuleClosure({ packageRoot, entryFile, excludedFiles, resolver });
}

/// The realpath of a path that exists, or the path itself.
///
/// Neither process's spelling of a file is predictable from the other's. This
/// one names whatever the caller handed it; the analyzing program names the
/// cleaned path it holds the file under, which is a realpath only where
/// resolution walked a symlink under `node_modules`. A package generated inside
/// a symlinked temporary directory -- `/var/folders/...` on macOS, and every
/// directory an ecosystem probe generates in -- is reached by both spellings at
/// once.
///
/// So every path is normalized through here before it is compared, and the
/// record is written back in this process's spelling (`packageScope`). Neither
/// side derives its answer from the other's, and a path that names no file (a
/// `bundled:` library path) normalizes to itself and falls outside the package
/// on its own.
///
/// **`realpathSync.native`, not `realpathSync`, and the difference is a
/// verdict.** The JavaScript implementation resolves symlinks and leaves the
/// case alone, so on a case-insensitive filesystem -- APFS, HFS+, NTFS -- two
/// spellings of one file normalized to two different keys: the record named a
/// path that does not exist on a case-sensitive filesystem, and the seed sweep
/// reported the same file as seeded-but-never-opened. `realpathSync.native`
/// goes through the platform's `realpath(3)`, which returns the name the
/// filesystem actually holds, so one file is one key on every platform and the
/// verdict for a package no longer depends on which machine generated it.
const realpathCache = new Map();
function realpathOrSelf(path) {
  const cached = realpathCache.get(path);
  if (cached !== undefined) return cached;
  let resolved;
  try {
    resolved = realpathSync.native(path);
  } catch {
    try {
      resolved = realpathSync(path);
    } catch {
      resolved = path;
    }
  }
  realpathCache.set(path, resolved);
  return resolved;
}

/// Where a module the analyzing program opened sits, relative to the package
/// this contract describes. Four answers, and only one of them is a silence.
///
/// - **`local`** carries the path *this* process spells the file with, which is
///   what the record names and hashes. Two spellings are accepted for the same
///   reason the native side accepts two (`local` in `write_module_inventory`):
///   TypeScript takes a realpath only where resolution walked a symlink under
///   `node_modules`, so a directory symlink *inside* the package -- `src ->
///   ../shared` -- is held under the spelled path while its realpath leaves
///   `realRoot` entirely. Canonicalizing first and filtering second dropped
///   that module from the record with no note at all, which is the exact defect
///   an attested record exists to make impossible. This filter must never be
///   narrower than the native one.
/// - **`dependency`** is an installed package's own bytes, reached through a
///   `node_modules` directory in either spelling. Excluded from the record
///   deliberately: they are not this package's bytes, no republish of it
///   changes them, and hashing them would bind the record to the *install
///   layout* (hoisted or nested) and to a dependency's version, so two
///   generations over byte-identical package bytes would refuse to transfer a
///   review. What the analysis read from a dependency is described by that
///   package's own contract and closure record (`dependencyContracts`). The
///   residue -- a dependency with no contract of its own -- is a named
///   approximation in docs/precision-backlog.md, not a claim this record makes.
/// - **`library`** is the producer's own bundled lib (`bundled:/libs/...`): not
///   an absolute path, so not a file any record could hash.
/// - **`foreign`** is everything else: a file the analysis read that this
///   record cannot claim. It is *noted*, not dropped -- a record that excludes
///   bytes the summaries were derived from has to say so.
function packageScope({ packageRoot, realRoot, spelled, real }) {
  const throughNodeModules = path => path.split(sep).includes("node_modules");
  if (!isAbsolute(spelled)) return { kind: "library" };
  // The canonical form first, so the record names the file the filesystem
  // holds: on a case-insensitive filesystem the analyzing program can hand back
  // a spelling that exists nowhere on a case-sensitive one, and a record is
  // transferred between machines. The spelled path is the fallback, for the file
  // whose realpath left the package root.
  const local = isWithinDirectory(realRoot, real)
    ? join(packageRoot, relative(realRoot, real))
    : isWithinDirectory(packageRoot, spelled)
      ? spelled
      : undefined;
  if (local !== undefined) {
    return throughNodeModules(relative(packageRoot, local))
      ? { kind: "dependency" }
      : { kind: "local", path: local };
  }
  return throughNodeModules(spelled) || throughNodeModules(real)
    ? { kind: "dependency" }
    : { kind: "foreign" };
}

function isWithinDirectory(root, candidate) {
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

/// The analyzing program's module inventory, or why there is none.
///
/// Fail-closed by construction: every shape that is not a complete, current
/// inventory answers `unavailable` with the reason, and no caller may treat an
/// `unavailable` as licence to fall back to the walk's own record. `complete`
/// is the producer's `ModuleGraph::is_complete` -- a scoped answer that covered
/// less than it asked for -- and it is checked here rather than at each use so
/// there is one place the check can be read.
///
/// **Two of these branches are defence against a future producer, not a tier
/// with a population, and the distinction is worth stating where a reader will
/// hit it.** Against the pinned producer neither can occur:
///
/// - `complete: false` is structurally unreachable. `unknownImportPaths` is
///   non-empty only for a *requested* path the program does not hold, and
///   `write_module_inventory` builds the request from the program's own
///   inventory answer with both sides cleaned, so the request is always a
///   subset of the holdings.
/// - an absent or unparsable inventory is unreachable through this call, because
///   a run that cannot write one exits non-zero and aborts the whole generation
///   before any contract or plan is written.
///
/// The code stays, and stays tested (`STUB_INVENTORY_ABSENT` /
/// `STUB_INVENTORY_INCOMPLETE` in scripts/contract-generation.test.mjs) as the
/// pin on the *contract* those shapes must honor if a producer ever answers
/// that way. What must not be claimed is that a user has seen the sentence
/// below, or that "unattested" is a tier this generator currently produces.
function readModuleInventory(path) {
  let document;
  try {
    document = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    return {
      unavailable: `the analyzing program wrote no module inventory (${String(
        error?.message ?? error
      )})`
    };
  }
  if (document?.schemaVersion !== 1) {
    return {
      unavailable: `the module inventory declares schema version ${JSON.stringify(
        document?.schemaVersion
      )} rather than 1`
    };
  }
  if (!Array.isArray(document.modules) || !Array.isArray(document.imports)) {
    return { unavailable: "the module inventory names no module list" };
  }
  if (document.complete !== true) {
    return {
      unavailable:
        "the analyzing program reported its resolved module graph incomplete: it holds fewer " +
        `files than the inventory asked about (${(document.unknownImportPaths ?? []).length} unanswered)`
    };
  }
  return document;
}

/// The attested inventories this generation collected, one per analyzed target.
///
/// Keyed by `(target, excludedTargets)` rather than by the analysis key, because
/// that is the key the closure record is rebuilt under: `generationClosures`
/// mirrors the analysis by excluding an entrypoint's sibling targets, while the
/// export-map conditions that complete the analysis key select *dependency
/// contracts* and not TypeScript's own resolution, so two condition sets over
/// one target analyze the same file set. `record` fails closed on the day that
/// stops being true instead of letting one condition's inventory stand in for
/// another's.
function moduleInventories() {
  const byTarget = new Map();
  const key = (target, excludedTargets) => JSON.stringify([target, [...excludedTargets].sort()]);
  // Modules *and* import facts: the modules decide the record, the import facts
  // decide which of the walk's problems survive reconciliation, and a check
  // that compared only the first would let two analyses that resolved the same
  // files differently share one attestation. Nothing in the generated tsconfig
  // is condition-dependent today, so this is expected to hold; the point is
  // that it is checked rather than assumed.
  const identity = inventory =>
    inventory?.unavailable
      ? `unavailable:${inventory.unavailable}`
      : JSON.stringify([
          (inventory?.modules ?? []).map(module => module.path),
          (inventory?.imports ?? []).map(fact => [
            fact.path,
            fact.startByte,
            fact.text,
            fact.resolution,
            fact.resolvedPath ?? ""
          ])
        ]);
  return {
    record(target, excludedTargets, inventory) {
      const slot = key(target, excludedTargets);
      const existing = byTarget.get(slot);
      if (existing === undefined) {
        byTarget.set(slot, inventory ?? { unavailable: "the target was not analyzed" });
        return;
      }
      if (identity(existing) !== identity(inventory)) {
        byTarget.set(slot, {
          unavailable:
            "two analyses of this target reported different module inventories, so neither " +
            "attests which bytes the summaries were derived from"
        });
      }
    },
    for(target, excludedTargets) {
      return (
        byTarget.get(key(target, excludedTargets)) ?? {
          unavailable: "this generation recorded no module inventory for the target"
        }
      );
    }
  };
}

/// One analyzed target's closure record, attested against the program that
/// produced its summaries.
///
/// Three answers, and the third is the point.
///
/// - **The record** is the inventory, scoped to the package by `packageScope`:
///   the files the analyzing program opened that are this package's own bytes.
///   Declaration files are kept. They are bytes the analysis read and the
///   summaries depend on them exactly as much as on a runtime module -- the walk
///   classified a `.d.ts` resolution as `external` because *it* could not read
///   runtime behavior out of one, which is a fact about the walk and not about
///   the analysis. Library files and dependency bytes are excluded, and
///   `packageScope` says why each exclusion is not a silence.
/// - **The walk's problems are reconciled, not quoted.** A specifier the walk
///   could not resolve and the compiler resolved nothing for either is not an
///   omission from the record -- the analysis read no file for it -- so no
///   *record* note is kept. One the compiler *did* resolve is kept and restated
///   with the attested path, which is strictly more than the walk could say.
/// - **What the runtime may still load is a different claim.** The record is
///   attested complete for what the analysis read; what stays unproven is what
///   the runtime loads, and no module graph can prove that. It rides
///   `runtimeNotes` so it still blocks promotion while no longer blocking a
///   transfer between two generations whose attested records are identical. Two
///   shapes reach it, and they make the same claim: a non-literal `import()`,
///   and a specifier the compiler resolved nothing for that names existing
///   runtime modules inside this package a runtime *can* select -- an
///   unselected conditional `imports` branch. The second is why "the compiler
///   resolved nothing" is not on its own a licence to say nothing: `#internal`
///   with a `browser`/`node` pair resolves to no file under `bundler`
///   resolution and to `node.mjs` under Node, and a side-effect import of it is
///   exactly where a package patches globals or calls into `solid-js/web`.
///   `runtimeTargets` (`createModuleResolver`) is the fact that separates it
///   from `./styles.css` and `./gone.js`, which name no runtime module at all
///   and which therefore no runtime loads either.
///
/// And the fail-closed halves: an absent or incomplete inventory leaves the
/// record unattested and noted -- never silently replaced by the walk's own
/// claim -- and a module either side names and the other does not is its own
/// note, in both directions. Those two inventory shapes are **defensive**: see
/// `readModuleInventory` for why no current producer can reach them.
function attestedClosure({
  packageRoot,
  realRoot,
  target,
  walked,
  inventory,
  openDynamicAttribution
}) {
  const notes = [];
  const runtimeNotes = [];
  const runtimeObligations = [];
  // Two spellings, deliberately. `relativeToRoot` names a path the *producer*
  // answered with, which is canonical, so it is relative to the canonical root
  // -- and it is what a note about a file outside the package has to use. The
  // `local` spelling of a module the record does carry goes through
  // `packageRelative` instead, so a note and the record name one file one way.
  const relativeToRoot = file => relative(realRoot, file).replaceAll(sep, "/");
  const packageRelative = file => relative(packageRoot, file).replaceAll(sep, "/");
  if (inventory?.unavailable) {
    notes.push(
      `${target}: closure not attested: ${inventory.unavailable}. The record below is this ` +
        "generator's own syntax walk, which cannot say whether the analyzing program read the " +
        "same files"
    );
    return { files: walked.files, notes, runtimeNotes, runtimeObligations };
  }
  // Every path from either side goes through `realpathOrSelf` before it is
  // compared: see there for why neither process's spelling can be derived from
  // the other's, and why the comparison is case-exact on every platform.
  //
  // One scoped view, computed once and used by the record and by all three
  // sweeps below. Two views was itself a defect: the record filtered on the
  // canonical path while the sweeps compared against the unfiltered inventory,
  // so a module the analysis read that the record excluded was invisible to
  // both the record and every note.
  const attested = inventory.modules.map(module => {
    const real = realpathOrSelf(module.path);
    return {
      ...module,
      path: real,
      scope: packageScope({ packageRoot, realRoot, spelled: module.path, real })
    };
  });
  const local = attested.filter(module => module.scope.kind === "local");
  const included = new Set(attested.map(module => module.path));
  const seeded = new Set(walked.files.map(realpathOrSelf));
  const importsByFile = new Map();
  for (const fact of inventory.imports) {
    const importer = realpathOrSelf(fact.path);
    const facts = importsByFile.get(importer) ?? [];
    facts.push({
      ...fact,
      ...(fact.resolvedPath ? { resolvedPath: realpathOrSelf(fact.resolvedPath) } : {})
    });
    importsByFile.set(importer, facts);
  }
  // A module already named by a restated specifier note is not also reported as
  // an unseeded module: one cause, one note.
  const restated = new Set();
  for (const problem of walked.problems) {
    const importer = realpathOrSelf(problem.file);
    if (!included.has(importer)) {
      // The program never opened the module this problem was read from, so
      // nothing attests what it imports -- and this branch has to come first,
      // because the sentences below all begin by claiming the record is
      // attested. The walk's own sentence stands instead, on `notes`.
      notes.push(noteFor(problem));
      continue;
    }
    if (problem.kind === "dynamic-import") {
      const note =
        `${problem.spelled}: the module record is attested -- it names every file the analyzing ` +
        `program opened under this package -- and complete except for what ${problem.reason} ` +
        "may load at runtime, which no module graph can enumerate";
      if (openDynamicAttribution) {
        runtimeObligations.push({
          note,
          exports: openDynamicAttribution.affectedExports
        });
      } else {
        runtimeNotes.push(note);
      }
      continue;
    }
    if (problem.kind !== "specifier" || !problem.specifier) {
      // A specifier this walk could not read at all -- an unterminated comment
      // or string, a non-literal `from`, a module whose bytes it could not
      // open. The compiler's import list for the same file settles it: a
      // resolution the walk missed is a file in the inventory, so it is
      // reported below as a module the walk did not seed rather than here as a
      // sentence about what the walk could not read.
      continue;
    }
    const resolved = (importsByFile.get(importer) ?? []).filter(
      fact => fact.text === problem.specifier && fact.resolution !== "unresolved" && fact.resolvedPath
    );
    if (!resolved.length) {
      // The compiler resolved nothing for the specifier either, so the analysis
      // read no file for it and the *record* is complete. That is a claim about
      // the record and says nothing about the runtime -- and where the walk
      // found runtime modules inside this package that a runtime can still
      // select for this specifier, the runtime loads package bytes neither side
      // read. Same claim as a non-literal `import()`, same channel: it blocks
      // promotion and not a transfer between two identical records.
      //
      // An empty `runtimeTargets` is the ordinary answer and is not a shortcut:
      // it means nothing on disk answers this specifier, so no runtime resolves
      // it to a module either -- `./styles.css`, `./gone.js`, an `imports` map
      // entry that matches nothing, a specifier that escapes the package (whose
      // boundary the dependency contract owns, exactly as a bare specifier's
      // is).
      const reachable = (problem.runtimeTargets ?? []).map(packageRelative).sort();
      if (reachable.length) {
        runtimeNotes.push(
          `${problem.spelled}: the module record is attested -- it names every file the analyzing ` +
            `program opened under this package -- and complete except for what ${problem.specifier} ` +
            `may load at runtime: the analyzing program resolved nothing for it (${problem.reason}), ` +
            `while ${reachable.join(", ")} exist on disk and a runtime selecting one of them reads ` +
            "package bytes this analysis did not, which no module graph can enumerate"
        );
      }
      continue;
    }
    for (const fact of resolved) {
      restated.add(fact.resolvedPath);
      notes.push(
        `${noteFor(problem)}; the analyzing program resolved it to ` +
          `${relativeToRoot(fact.resolvedPath)} (${fact.resolution}${
            fact.extension ? `, ${fact.extension}` : ""
          }), so the analysis read a module this walk did not seed`
      );
    }
  }
  for (const module of local) {
    // A declaration file is not a seeding gap. TypeScript preferring an
    // adjacent `.d.ts` over the `.js` beside it is why the walk seeds runtime
    // files at all, and the identity split that creates is the analyzer's own
    // incompleteness finding, not this record's -- see
    // docs/precision-backlog.md. Reporting it here would double-report it.
    if (module.declarationFile) continue;
    if (seeded.has(module.path) || restated.has(module.path)) continue;
    notes.push(
      `${packageRelative(module.scope.path)}: the analyzing program opened this module and the closure ` +
        "walk did not seed it, so the analysis read package bytes the walk did not enumerate"
    );
  }
  for (const file of walked.files) {
    const real = realpathOrSelf(file);
    if (included.has(real)) continue;
    // The same scoped view the record uses: a seeded path that is not this
    // package's own bytes is not a seeding gap in this package's record. The
    // note names it the way the record would have -- package-relative, in this
    // process's spelling -- rather than through a realpath that a directory
    // symlink can push outside the package.
    const scope = packageScope({ packageRoot, realRoot, spelled: file, real });
    if (scope.kind !== "local") continue;
    notes.push(
      `${packageRelative(scope.path)}: the closure walk seeded this ` +
        "module as an analysis root and the analyzing program did not open it, so the " +
        "record cannot say the summaries were derived from it"
    );
  }
  // The third direction, and the one that had no note at all: a module the
  // analysis read that the record's own scope excludes. A dependency's bytes
  // and the producer's bundled libs are named elsewhere (`packageScope`), and a
  // declaration file outside this package is a dependency's typing whose
  // identity the analyzer's own declaration-sibling finding owns. Everything
  // else is a file the summaries were derived from that no hash here pins --
  // which is precisely what a record may not leave unsaid.
  for (const module of attested) {
    if (module.scope.kind !== "foreign" || module.declarationFile) continue;
    if (restated.has(module.path)) continue;
    notes.push(
      `${relativeToRoot(module.path)}: the analyzing program opened this module and it is not ` +
        "inside this package, so the record excludes bytes the summaries were derived from"
    );
  }
  return {
    files: local.map(module => module.scope.path),
    notes,
    runtimeNotes,
    runtimeObligations
  };
}

function patternCapture(pattern, candidate) {
  const star = pattern.indexOf("*");
  if (star === -1) return pattern === candidate ? "" : undefined;
  if (pattern.indexOf("*", star + 1) !== -1) {
    throw refuse(`package export pattern may contain only one wildcard: ${pattern}`);
  }
  const prefix = pattern.slice(0, star);
  const suffix = pattern.slice(star + 1);
  if (
    !candidate.startsWith(prefix) ||
    !candidate.endsWith(suffix) ||
    candidate.length < prefix.length + suffix.length
  ) {
    return undefined;
  }
  return candidate.slice(prefix.length, candidate.length - suffix.length);
}

function substituteStar(pattern, capture) {
  const star = pattern.indexOf("*");
  if (star === -1 || pattern.indexOf("*", star + 1) !== -1) {
    throw refuse(`package export pattern must contain one wildcard: ${pattern}`);
  }
  return `${pattern.slice(0, star)}${capture}${pattern.slice(star + 1)}`;
}

function packageTargetPath(packageRoot, target) {
  if (!target.startsWith("./")) {
    throw refuse(`package export target must be relative: ${target}`);
  }
  const path = resolve(packageRoot, target);
  if (path !== packageRoot && !path.startsWith(`${packageRoot}${sep}`)) {
    throw refuse(`package export target escapes the package root: ${target}`);
  }
  return path;
}

function existingPackageTarget(packageRoot, target) {
  const path = packageTargetPath(packageRoot, target);
  return existsSync(path) && statSync(path).isFile();
}

function legacyTarget(value) {
  if (typeof value !== "string" || value.length === 0) return undefined;
  if (isAbsolute(value)) {
    throw refuse(`legacy package runtime target must be relative: ${value}`);
  }
  return value.startsWith("./") ? value : `./${value}`;
}

/// Which legacy manifest field a root contract was resolved from, and whether
/// another field names a *different* runtime artifact.
///
/// `module` is the bundler's ESM entry; `main` is what Node's own resolver
/// loads. When they name different files the generated contract describes only
/// the one that was analyzable, and a consumer resolving the other gets a
/// summary that was never proven for it. Schema v1 has no condition that
/// distinguishes the two -- the `import`/`require` pair describes a resolver
/// choice, not these fields -- so this is surfaced for review rather than
/// encoded as a variant, and the entrypoint is not refused: `main` is usually
/// just the CJS transpile of the same source, which the generator cannot read
/// either way.
function legacyRootProvenance(manifest) {
  if (manifest.exports) return undefined;
  const moduleTarget = legacyTarget(manifest.module);
  const mainTarget = legacyTarget(manifest.main);
  if (!moduleTarget) return undefined;
  if (!mainTarget || mainTarget === moduleTarget) {
    return {
      field: "module",
      target: moduleTarget,
      text: `.: resolved from the legacy "module" field (${moduleTarget})`
    };
  }
  return {
    field: "module",
    target: moduleTarget,
    divergentMain: mainTarget,
    text:
      `.: resolved from the legacy "module" field (${moduleTarget}), but "main" names a different ` +
      `runtime artifact (${mainTarget}); confirm both builds have the same reactive behavior before ` +
      `promoting this contract, or publish an exports map that separates them`
  };
}

function legacyPackageExports(packageRoot, manifest) {
  const moduleTarget = legacyTarget(manifest.module);
  if (moduleTarget) {
    if ([".cjs", ".cts"].includes(extname(moduleTarget))) {
      throw refuse(". has only a CJS runtime target; CJS contract generation is unsupported");
    }
    if (!runtimeLeaf(moduleTarget) || !existingPackageTarget(packageRoot, moduleTarget)) {
      throw refuse(
        `${manifest.name} has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ${moduleTarget}`
      );
    }
    return { ".": moduleTarget };
  }

  const mainTarget = legacyTarget(manifest.main);
  if (mainTarget) {
    const extension = extname(mainTarget);
    const esmMain = [".mjs", ".mts"].includes(extension) ||
      (manifest.type === "module" && runtimeLeaf(mainTarget));
    if (!esmMain) {
      throw refuse(". has only a CJS runtime target; CJS contract generation is unsupported");
    }
    if (!existingPackageTarget(packageRoot, mainTarget)) {
      throw refuse(
        `${manifest.name} has no supported ESM runtime entrypoints; legacy main target does not exist: ${mainTarget}`
      );
    }
    return { ".": mainTarget };
  }

  const fallbackCandidates = ["./index.mjs", "./index.mts"];
  if (manifest.type === "module") fallbackCandidates.push("./index.js");
  const fallback = fallbackCandidates.find(target => existingPackageTarget(packageRoot, target));
  return fallback ? { ".": fallback } : undefined;
}

function concreteEntrypoints(packageRoot, exports_, selectedConditions) {
  const map =
    typeof exports_ === "object" &&
    !Array.isArray(exports_) &&
    exports_ !== null &&
    Object.keys(exports_).some(key => key.startsWith("."))
      ? exports_
      : { ".": exports_ };
  const packageFiles = Object.keys(map).some(key => key.includes("*"))
    ? walkFiles(packageRoot)
    : [];
  const concrete = new Map();
  const add = (entrypoint, leaf) => {
    const item = concrete.get(entrypoint) ?? [];
    if (
      !item.some(
        existing =>
          existing.target === leaf.target &&
          JSON.stringify(existing.conditions) === JSON.stringify(leaf.conditions)
      )
    ) {
      item.push(leaf);
    }
    concrete.set(entrypoint, item);
  };

  for (const [entrypoint, target] of Object.entries(map)) {
    // `package.json#exports` is an ordered map resolved first-match-wins, and
    // both collectors walk it depth-first in declaration order. So a leaf's
    // position in this array IS its resolution precedence: index 0 is the
    // branch Node would pick when several match. Recording it here is what
    // lets overlapping branches with genuinely different semantics be
    // represented instead of refused.
    const leaves = (selectedConditions.length
      ? resolveRuntimeLeaf(target, new Set(selectedConditions))
      : collectRuntimeLeaves(target)
    ).map((leaf, index) => ({ ...leaf, precedence: index }));
    if (
      leaves.length === 0 &&
      stringTargets(target).some(target =>
        [".cjs", ".cts"].includes(extname(target))
      )
    ) {
      throw refuse(
        `${entrypoint} has only a CJS runtime target; CJS contract generation is unsupported`
      );
    }
    if (!entrypoint.includes("*")) {
      // A source checkout can legitimately advertise build-condition targets
      // that do not exist until packaging. Analyze every currently materialized
      // runtime variant; a package with no materialized variant still fails
      // below instead of silently receiving an empty contract.
      for (const leaf of leaves) {
        if (existingPackageTarget(packageRoot, leaf.target)) add(entrypoint, leaf);
      }
      continue;
    }
    for (const leaf of leaves) {
      if (!leaf.target.includes("*")) {
        throw refuse(
          `package export pattern ${entrypoint} has non-pattern target ${leaf.target}`
        );
      }
      for (const file of packageFiles) {
        const capture = patternCapture(leaf.target, file);
        if (capture === undefined) continue;
        add(substituteStar(entrypoint, capture), {
          target: file,
          conditions: leaf.conditions,
          precedence: leaf.precedence
        });
      }
    }
  }
  return concrete;
}

function packageLocalTarget(packageRoot, target) {
  const path = packageTargetPath(packageRoot, target);
  if (!existsSync(path) || !statSync(path).isFile()) {
    throw refuse(`package export target does not exist: ${target}`);
  }
  return path;
}

function defaultOutput(packageRoot, packageName) {
  if (resolve(process.cwd()) === packageRoot) {
    return join(packageRoot, "solid-reactivity.json");
  }
  return join(
    process.cwd(),
    ".solid-checker",
    "contracts",
    ...packageName.split("/"),
    "solid-reactivity.json"
  );
}

function sha256Artifact(path) {
  return `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
}

/// Binds the emitted contract to the exact runtime bytes it describes, in the
/// only shape schema v1 has for saying so.
///
/// `artifacts.implementation` is one `{ path, hash }` pair whose path the
/// consumer resolves *inside the contract file's own directory* and whose
/// sha256 it verifies on every load (see `validate_contract_artifacts` in
/// rust/crates/solid-facts-backend/src/diagnostics.rs, and the "Trust
/// boundary" section of docs/package-contracts.md). Two facts follow from
/// that, and neither is negotiable:
///
/// - One pair. A package whose emitted entrypoints resolve to several runtime
///   artifacts cannot be bound by it; hashing one of them would claim byte
///   identity for a contract whose other entrypoints describe files nothing
///   pins. That case stays unbound and says so on the review plan.
/// - Inside the contract's directory. The project-owned output form
///   (`.solid-checker/contracts/<package>/solid-reactivity.json`) is outside
///   the package by construction, so its artifact path could only be spelled
///   with `..` -- which the consumer rejects outright. Unbound, and said.
///
/// The declaration artifact is deliberately not emitted: this generator
/// analyzes runtime targets and never resolves the `types` condition, so it
/// has no declaration file whose bytes it could honestly claim to have read.
///
/// One more thing the pair cannot say, and the review plan therefore must. The
/// bound hash covers the *entry* artifact only, while the analysis behind the
/// summaries consumes every module the analyzing program opened under this
/// package. A barrel entry -- `export { x } from "./internal.mjs"` -- therefore
/// gets a contract whose semantics come from files no hash pins: replace
/// `internal.mjs` and the entry bytes, and the hash with them, are unchanged.
/// The hash is still real evidence about the entry file, so it keeps being
/// emitted; the unpinned remainder is counted on the review plan instead of
/// being left to look like full byte binding.
///
/// That count is read off the closure record rather than re-walked, which is why
/// this runs after `generationClosures`. The two used to be independent walks of
/// the same entrypoint, so a hole in one was a hole in the other with nothing
/// forcing them to agree; now there is one attested answer and this reads it.
/// With exactly one target the record is the same for every entrypoint -- one
/// target means no sibling to exclude -- so any entrypoint's record answers it,
/// and the body checks that rather than trusting it: a disagreement is a bug in
/// this generation and a bug may not surface as a smaller unpinned remainder.
export function contractArtifacts(output, packageRoot, targetsByEntrypoint, entrypoints, closures) {
  const targets = [
    ...new Set(
      [...targetsByEntrypoint]
        .filter(([entrypoint]) => entrypoints[entrypoint])
        .flatMap(([, entrypointTargets]) => [...entrypointTargets])
    )
  ].sort();
  if (targets.length === 0) return { artifacts: {}, notes: [] };
  if (targets.length > 1) {
    return {
      artifacts: {},
      notes: [
        `contract is not byte-bound: ${targets.length} runtime artifacts back this contract (${targets.join(", ")}) and schema v1 records one implementation artifact; check each target against the exact package release by hand`
      ]
    };
  }
  const [target] = targets;
  const file = packageLocalTarget(packageRoot, target);
  const directory = dirname(output);
  const relativePath = relative(directory, file).replaceAll(sep, "/");
  if (
    !relativePath ||
    isAbsolute(relativePath) ||
    relativePath === ".." ||
    relativePath.startsWith("../")
  ) {
    return {
      artifacts: {},
      notes: [
        `contract is not byte-bound: ${target} is outside the contract's own directory (${directory}), and schema-v1 artifact paths must resolve inside it; check the artifact against the exact package release by hand`
      ]
    };
  }
  // Every record here describes this one target, so any of them answers the
  // count -- and that is checked rather than assumed. A record with no module at
  // all is the `closure not recorded` catch path: it counts nothing, and reading
  // it as "one module, nothing pulled in" would suppress this note over a
  // generation that failed to derive a closure. A generation with no record at
  // all (every entrypoint refused before a closure was derived) likewise says
  // nothing extra here -- the per-entrypoint notes already carry why.
  const records = Object.values(closures.entrypoints ?? {}).filter(
    entry => Array.isArray(entry?.modules) && entry.modules.length > 0
  );
  const counts = [...new Set(records.map(entry => entry.modules.length))].sort(
    (left, right) => left - right
  );
  const artifacts = { implementation: { path: relativePath, hash: sha256Artifact(file) } };
  if (counts.length > 1) {
    // One target means no sibling to exclude, so two records over it cannot
    // legitimately differ. If they do, the generation contradicted itself and
    // the smaller count must not be the one a reviewer is handed.
    return {
      artifacts,
      notes: [
        `contract is byte-bound to its entry artifact only: the closure records for ${target} name different module counts (${counts.join(", ")}) although one target has no sibling to exclude, so the unpinned remainder is not established; check every module the analysis read against the exact package release by hand`
      ]
    };
  }
  const pulled = (counts[0] ?? 1) - 1;
  return {
    artifacts,
    notes:
      pulled > 0
        ? [
            `contract is byte-bound to its entry artifact only: ${target} pulls in ${pulled} further module(s) the analysis read, whose bytes the summaries depend on and schema v1 cannot pin; check those against the exact package release by hand`
          ]
        : []
  };
}

/// The closure notes, restated on the review plan's artifact-binding section.
///
/// A `notes` entry inside `generation.entrypoints` already blocks a transfer,
/// but a reviewer reading the checklist would never see it. A specifier the
/// walk could not resolve and the analyzing program did resolve means the
/// contract is bound to *less* than the entry artifact -- the hash covers bytes
/// whose dependencies nobody enumerated -- which is exactly what this section is
/// for.
///
/// `runtimeNotes` ride the same section for the same reason, and are the same
/// checklist item to a reviewer: the difference between the two kinds is which
/// gate they block (see `closureDifference` and `collectBlockers`), not whether
/// a human has to look.
function closureEnumerationNotes(closures) {
  return Object.entries(closures.entrypoints ?? {})
    .flatMap(([entrypoint, record]) =>
      [...(record.notes ?? []), ...(record.runtimeNotes ?? [])].map(
        note => `${entrypoint} ${note}`
      )
    )
    .sort();
}

/// What the emitted summaries were derived from, per entrypoint: every module
/// the analyzing program opened under this package, hashed.
///
/// This is the review plan's record, not the contract's: `artifacts` can carry
/// one implementation pair inside the contract's own directory, while an
/// entrypoint's real closure is several files and a project-owned output sits
/// outside the package entirely, so these paths may be spelled with `..`. It
/// answers the question the checklist cannot -- *which bytes was this reviewed
/// against* -- and nothing loads it as evidence.
///
/// The record is an **attestation**, not a reconstruction: the module list is the
/// analyzing program's own (`--emit-module-inventory`), and this generator's
/// syntax walk is reconciled against it rather than quoted. See
/// `attestedClosure` for the three answers that produces and the two fail-closed
/// halves. One consequence worth stating where a reader will hit it: the record
/// names realpaths, resolved back into this process's spelling, and it names the
/// declaration files the analysis read -- so a record written before attestation
/// landed does not compare equal to one written after, and a review recorded
/// against the older record does not transfer. That break is one-time and
/// documented in docs/package-contracts.md.
function generationClosures(
  output,
  packageRoot,
  resolver,
  targetsByEntrypoint,
  entrypoints,
  legacyRoot,
  inventories,
  targetExportNames
) {
  const directory = dirname(output);
  const realRoot = realpathOrSelf(packageRoot);
  const closures = {};
  for (const [entrypoint, targets] of [...targetsByEntrypoint].sort(([left], [right]) =>
    left.localeCompare(right)
  )) {
    if (!entrypoints[entrypoint]) continue;
    const sorted = [...targets].sort();
    const modules = [];
    const notes = [];
    const runtimeNotes = [];
    const runtimeObligations = [];
    for (const target of sorted) {
      // Mirrors the analysis exactly: a sibling conditional target of the same
      // entrypoint is excluded there, so it is not part of what this target's
      // summaries were derived from -- and it is the other half of the key the
      // attested inventory for this target was recorded under.
      const excluded = new Set();
      const excludedTargets = sorted.filter(sibling => sibling !== target);
      let files;
      try {
        for (const sibling of excludedTargets) {
          excluded.add(packageLocalTarget(packageRoot, sibling));
        }
        const entryFile = packageLocalTarget(packageRoot, target);
        const walked = closureOf(packageRoot, resolver, entryFile, excluded);
        const dynamicProblems = walked.problems.filter(problem => problem.kind === "dynamic-import");
        const analyzedExportNames = targetExportNames.get(target);
        const openDynamicCandidate =
          dynamicProblems.length > 0 &&
          Array.isArray(analyzedExportNames) &&
          dynamicProblems.every(problem => realpathOrSelf(problem.file) === realpathOrSelf(entryFile))
            ? openDynamicImportReachability(
                readFileSync(entryFile, "utf8"),
                analyzedExportNames
              )
            : undefined;
        const openDynamicAttribution = Array.isArray(openDynamicCandidate?.affectedExports)
          ? openDynamicCandidate
          : undefined;
        const reconciled = attestedClosure({
          packageRoot,
          realRoot,
          target,
          walked,
          inventory: inventories.for(target, excludedTargets),
          openDynamicAttribution
        });
        files = reconciled.files;
        // Each note already names the exact module the specifier was read from,
        // which is more precise than the target that reached it.
        for (const note of reconciled.notes) notes.push(note);
        for (const note of reconciled.runtimeNotes) runtimeNotes.push(note);
        for (const obligation of reconciled.runtimeObligations) {
          runtimeObligations.push({ target, ...obligation });
        }
      } catch (error) {
        notes.push(`${target}: closure not recorded (${String(error?.message ?? error)})`);
        continue;
      }
      for (const file of files) {
        const path = relative(directory, file).replaceAll(sep, "/");
        if (modules.some(module => module.path === path)) continue;
        try {
          modules.push({ path, hash: sha256Artifact(file) });
        } catch (error) {
          // An unreadable module is left out rather than recorded with a hash
          // of nothing; the note is what keeps the omission visible.
          notes.push(`${path}: bytes unavailable at generation (${String(error?.message ?? error)})`);
        }
      }
    }
    closures[entrypoint] = {
      targets: sorted,
      modules: modules.sort((left, right) => left.path.localeCompare(right.path)),
      ...(notes.length ? { notes: [...new Set(notes)].sort() } : {}),
      // A separate field, not a second class of `notes`, because the two answer
      // different questions. A `notes` entry says the record does not establish
      // which bytes the summaries came from, so nothing transfers against it. A
      // `runtimeNotes` entry says the record *is* established and something
      // outside any module graph may still load a module the analysis never
      // read -- so two generations with identical records may transfer, and
      // promotion is still refused.
      ...(runtimeNotes.length ? { runtimeNotes: [...new Set(runtimeNotes)].sort() } : {}),
      ...(runtimeObligations.length
        ? {
            runtimeObligations: [...new Map(
              runtimeObligations.map(obligation => [JSON.stringify(obligation), obligation])
            ).values()].sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)))
          }
        : {})
    };
  }
  return {
    generator: generatorIdentity(),
    entrypoints: closures,
    // Which legacy manifest field the root came from, as data rather than as
    // the sentence the checklist renders: a republish that starts diverging
    // `main` from `module` must not inherit the previous review's confirmation
    // of a manifest that did not.
    ...(legacyRoot
      ? {
          legacyRoot: {
            field: legacyRoot.field,
            target: legacyRoot.target,
            ...(legacyRoot.divergentMain ? { divergentMain: legacyRoot.divergentMain } : {})
          }
        }
      : {})
  };
}

/// Keeps a review alive across the regeneration that invalidates it.
///
/// The documented upgrade sequence is *regenerate, then transfer*, and until
/// this existed it could not be run: regenerating in place overwrote the
/// contract and the review plan, and `--transfer-from` needs both of the old
/// ones to read a reviewed conclusion off them. So a generation that is about
/// to overwrite a contract a review state sits beside moves the whole triple --
/// contract, plan, state -- to `<name>.previous.json` and its siblings first,
/// and the new generation starts with no review state of its own. Any older
/// `.previous` set is replaced: two regenerations without a transfer in between
/// mean the middle contract was never reviewed, and keeping its state would let
/// a transfer read a review of bytes nobody promoted.
///
/// A contract with no review state beside it is not snapshotted: there is
/// nothing to carry, and a stale `.previous` pair would be a transfer source
/// that looks reviewed and is not.
///
/// A *machine-verified* contract has no review state and is snapshotted anyway.
/// Its conclusion is not transferable -- `--transfer-from` refuses a verified
/// source outright, because a verification is an observation of the bytes that
/// were installed when it ran and is reproduced rather than carried -- but the
/// `<contract>.verify.json` and `<contract>.probe.json` sidecars *are* the audit
/// trail for exactly those bytes, and overwriting the contract while leaving
/// them in place beside a fresh `inferred` draft would leave two files claiming
/// to describe a document that no longer exists. So they travel with the bytes
/// they describe. Both are self-invalidating anyway -- each records the contract
/// hash it was written for -- but a sidecar that is merely stale beside the
/// wrong file is a worse artifact than one filed with its own contract.
/// Whether the `.verify.json` beside a contract records a *promotion*.
///
/// `contract verify` writes the sidecar on its refusal path too, with
/// `outcome: "refused"` and no `evidence` block. That record is not a
/// verification and must not be read as one: a refused draft has no conclusion
/// to preserve, so it neither triggers a snapshot nor makes the message below
/// say a machine-verified contract was kept.
function recordsVerification(path) {
  if (!existsSync(path)) return false;
  try {
    const report = JSON.parse(readFileSync(path, "utf8"));
    return report.outcome !== "refused" && Boolean(report.evidence);
  } catch {
    return false;
  }
}

function snapshotPreviousReview(output) {
  const state = reviewStatePath(output);
  const verified = verifyReportPath(output);
  if (!existsSync(output) || !(existsSync(state) || recordsVerification(verified))) return undefined;
  const previous = previousContractPath(output);
  const moves = [
    [output, previous],
    [reviewPlanJsonPath(output), reviewPlanJsonPath(previous)],
    [reviewPlanPath(output), reviewPlanPath(previous)],
    [state, reviewStatePath(previous)],
    [verified, verifyReportPath(previous)],
    [probeReportPath(output), probeReportPath(previous)]
  ];
  for (const [from, to] of moves) {
    rmSync(to, { force: true });
    if (existsSync(from)) renameSync(from, to);
  }
  return { contract: previous, verified: recordsVerification(verifyReportPath(previous)) };
}

function dependencyContracts(packageRoot, manifest) {
  const dependencies = new Set([
    ...Object.keys(manifest.dependencies ?? {}),
    ...Object.keys(manifest.optionalDependencies ?? {}),
    ...Object.keys(manifest.peerDependencies ?? {})
  ]);
  const contracts = [];
  for (const dependency of [...dependencies].sort()) {
    let directory = packageRoot;
    while (true) {
      const candidate = join(
        directory,
        "node_modules",
        ...dependency.split("/"),
        "solid-reactivity.json"
      );
      if (existsSync(candidate)) {
        contracts.push(candidate);
        break;
      }
      const parent = dirname(directory);
      if (parent === directory) break;
      directory = parent;
    }
  }
  return contracts;
}

function semanticSummary(summary) {
  if (Array.isArray(summary)) return summary.map(semanticSummary);
  if (!summary || typeof summary !== "object") return summary;
  return Object.fromEntries(
    Object.entries(summary)
      .filter(([key]) => key !== "evidence")
      .map(([key, value]) => [key, semanticSummary(value)])
  );
}

function semanticSummaryKey(summary) {
  return JSON.stringify(semanticSummary(summary));
}

function sameSummary(left, right) {
  return semanticSummaryKey(left) === semanticSummaryKey(right);
}

const exclusiveConditionGroups = [
  new Set(["browser", "node", "deno", "worker"]),
  new Set(["development", "production"]),
  new Set(["csr", "string-ssr", "streaming-ssr"])
];

// Positive condition lists can represent disjoint branches such as browser
// versus node. They cannot represent export-map ordering: a development path
// and its default/import fallback overlap unless the contract also records a
// negative predicate. Refuse that semantic split instead of manufacturing an
// ambiguous variant which every consumer must leave uncertifiable.
function conditionBranchesOverlap(left, right) {
  if (left.includes("default") || right.includes("default")) return true;
  for (const group of exclusiveConditionGroups) {
    const leftMember = left.find(condition => group.has(condition));
    const rightMember = right.find(condition => group.has(condition));
    if (leftMember && rightMember && leftMember !== rightMember) return false;
  }
  return true;
}

function conditionsContain(container, contained) {
  return contained.every(condition => container.includes(condition));
}

// When a more-specific export-map path proves the same behavior as a broader
// path, retaining both would make runtime selection match two variants and
// therefore become uncertifiable. Keep the broader semantic representative;
// disjoint equal branches remain separate because neither condition set
// contains the other.
function removeRedundantConditionalSummaries(summaries) {
  const kept = [];
  for (const candidate of [...summaries].sort(
    (left, right) =>
      left.conditions.length - right.conditions.length ||
      JSON.stringify(left.conditions).localeCompare(JSON.stringify(right.conditions))
  )) {
    if (
      kept.some(
        representative =>
          sameSummary(representative.summary, candidate.summary) &&
          conditionsContain(candidate.conditions, representative.conditions)
      )
    ) {
      continue;
    }
    kept.push(candidate);
  }
  return kept;
}

function mergeUnique(left = [], right = [], compare) {
  const values = new Map();
  for (const value of [...left, ...right]) values.set(JSON.stringify(value), value);
  return [...values.values()].sort(compare);
}

function mergeClaimRows(left, right, compare) {
  if (isUnknownClaim(left) || isUnknownClaim(right)) return { status: "unknown" };
  return mergeUnique(left, right, compare);
}

/// Whether merged `callbacks` rows claim two executions for one parameter.
///
/// The per-target sentinel in `contract_export_function` (solid-reactive-ir)
/// cannot see this: it runs once per analyzed target, and the union above then
/// puts both targets' rows in one list. Schema v1 has one execution axis per
/// parameter and the runtime has one behavior *per selected target*, so a base
/// carrying `parameter: 0` as both `deferred` and `inline` states two mutually
/// exclusive things and a consumer picking either is guessing. The exact
/// per-branch claims survive in `variants` beside the base, which is what the
/// export map states outright — the same trade `returns` and `asyncBehavior`
/// already make one function down.
///
/// `mergeUnique`'s comparator breaks ties on `execution` precisely because two
/// executions per parameter were expected here; that tiebreaker now only orders
/// rows on the way to this check.
///
/// Rows agreeing on `execution` and differing elsewhere (argument descriptors,
/// owner) are not contradictory — they are extra facts about one schedule.
/// One-sided *presence* is a wider question this deliberately does not answer:
/// a parameter with a row in one branch and none in the other is a positive
/// against a certified negative, the same hole `claimDomainsDiverge` closed for
/// `returns`, and closing it for callbacks needs its own measurement. Recorded
/// in docs/precision-backlog.md.
///
/// Returns "" when there is no contradiction, and otherwise the shape of it.
export function callbackRowsContradict(rows) {
  if (isUnknownClaim(rows) || !Array.isArray(rows)) return "";
  const executions = new Map();
  for (const row of rows) {
    const seen = executions.get(row.parameter);
    if (seen !== undefined && seen !== row.execution) {
      return `the branches prove different executions for parameter ${row.parameter}`;
    }
    executions.set(row.parameter, row.execution);
  }
  return "";
}

/// Whether two branches disagree about a single-valued claim domain.
///
/// One-sided presence is a disagreement, and missing that was a real hole. When
/// one branch *proved* a `returns` (or an `asyncBehavior`) and the other proved
/// none, `left.returns ?? right.returns` handed the proving branch's claim to
/// the environment-unaware base -- a certified positive claim that is simply
/// false in the other environment. The both-present case was already handled;
/// this is the same fact with an absence on one side, which schema v1 has the
/// same exact spelling for.
///
/// An absence is not "nothing to merge": in a *proven* summary it is itself a
/// certified negative, so a base built from one branch's positive and the
/// other's negative can only be the sentinel.
///
/// Returns "" when the branches agree, and otherwise the shape of the
/// disagreement, which the review plan quotes to the reviewer.
function claimDomainsDiverge(left, right, same) {
  if (left === undefined && right === undefined) return "";
  if (isUnknownClaim(left) || isUnknownClaim(right)) return "";
  if (left === undefined || right === undefined) {
    return "one branch proves it and another proves none";
  }
  return same(left, right) ? "" : "the branches prove different values";
}

function mergeSummaries(left, right, onDiverge = () => {}) {
  if (sameSummary(left, right)) return left;
  if (left.kind !== right.kind) {
    // A callable contract is only safe when every selected runtime target is
    // callable. `value` is the conservative cross-condition surface.
    onDiverge("kind", "the branches prove different export kinds");
    return { kind: "value" };
  }
  // Two branches that each *prove* a different return, or a different async
  // behavior, have no single cross-condition answer. That is a claim the
  // environment-unaware base cannot make -- not a reason to refuse the whole
  // entrypoint and publish nothing about its other exports. Schema v1 has an
  // exact spelling for it, and this function already uses that spelling one
  // line down when either side is unknown and one branch up when the *kinds*
  // diverge (`value` is that branch's conservative surface). An
  // environment-aware consumer still gets the exact behavior: the divergent
  // branches are emitted as `variants` beside this base.
  //
  // This matters because it is the shape a real package has. solid-js 1.9.14's
  // `Show` returns its `props` argument in the server build and a memo accessor
  // in the client build; refusing on that discarded the other 147 exports of
  // its `.` entrypoint.
  const returnsDiverge = claimDomainsDiverge(left.returns, right.returns, sameSummary);
  const asyncBehaviorDiverges = claimDomainsDiverge(
    left.asyncBehavior,
    right.asyncBehavior,
    (a, b) => a === b
  );
  if (returnsDiverge) onDiverge("returns", returnsDiverge);
  if (asyncBehaviorDiverges) onDiverge("asyncBehavior", asyncBehaviorDiverges);
  const merged = { kind: left.kind };
  const evidence = left.evidence ?? right.evidence;
  if (evidence) merged.evidence = evidence;
  const returns =
    returnsDiverge || isUnknownClaim(left.returns) || isUnknownClaim(right.returns)
      ? { status: "unknown" }
      : left.returns ?? right.returns;
  if (returns) merged.returns = returns;
  const united = mergeClaimRows(
    left.callbacks,
    right.callbacks,
    (a, b) => a.parameter - b.parameter || a.execution.localeCompare(b.execution)
  );
  const callbacksDiverge = callbackRowsContradict(united);
  if (callbacksDiverge) onDiverge("callbacks", callbacksDiverge);
  const callbacks = callbacksDiverge ? { status: "unknown" } : united;
  if (isUnknownClaim(callbacks) || callbacks.length) merged.callbacks = callbacks;
  const ownerRequirements = mergeClaimRows(
    left.ownerRequirements,
    right.ownerRequirements,
    (a, b) => a.operation.localeCompare(b.operation)
  );
  if (isUnknownClaim(ownerRequirements) || ownerRequirements.length) {
    merged.ownerRequirements = ownerRequirements;
  }
  const asyncBehavior =
    asyncBehaviorDiverges ||
    isUnknownClaim(left.asyncBehavior) ||
    isUnknownClaim(right.asyncBehavior)
      ? { status: "unknown" }
      : left.asyncBehavior ?? right.asyncBehavior;
  if (asyncBehavior) merged.asyncBehavior = asyncBehavior;
  const reactiveReads = mergeClaimRows(
    left.reactiveReads,
    right.reactiveReads,
    (a, b) =>
      a.kind.localeCompare(b.kind) ||
      (a.parameter ?? -1) - (b.parameter ?? -1) ||
      (a.label ?? "").localeCompare(b.label ?? "")
  );
  if (isUnknownClaim(reactiveReads) || reactiveReads.length) merged.reactiveReads = reactiveReads;
  const variants = mergeUnique(
    left.variants,
    right.variants,
    (a, b) =>
      JSON.stringify(a.conditions).localeCompare(JSON.stringify(b.conditions)) ||
      JSON.stringify(a.summary).localeCompare(JSON.stringify(b.summary)),
  );
  if (variants.length) merged.variants = variants;
  return merged;
}

function inferredClaimEvidence() {
  return { kind: "inferred" };
}

function annotateReturnEvidence(returned) {
  if (!returned || isUnknownClaim(returned)) return returned;
  return {
    ...returned,
    evidence: returned.evidence ?? inferredClaimEvidence(),
    ...(returned.elements
      ? { elements: returned.elements.map(element => annotateReturnEvidence(element)) }
      : {}),
    ...(returned.properties
      ? {
          properties: Object.fromEntries(
            Object.entries(returned.properties).map(([name, value]) => [
              name,
              annotateReturnEvidence(value)
            ])
          )
        }
      : {})
  };
}

function annotateClaimEvidence(summary) {
  return {
    ...summary,
    evidence: summary.evidence ?? inferredClaimEvidence(),
    ...(Array.isArray(summary.reactiveReads)
      ? {
          reactiveReads: summary.reactiveReads.map(read => ({
            ...read,
            evidence: read.evidence ?? inferredClaimEvidence()
          }))
        }
      : {}),
    ...(Array.isArray(summary.callbacks)
      ? {
          callbacks: summary.callbacks.map(callback => ({
            ...callback,
            ...(callback.arguments
              ? {
                  arguments: callback.arguments.map(argument =>
                    argument ? annotateReturnEvidence(argument) : null
                  )
                }
              : {}),
            evidence: callback.evidence ?? inferredClaimEvidence()
          }))
        }
      : {}),
    ...(Array.isArray(summary.ownerRequirements)
      ? {
          ownerRequirements: summary.ownerRequirements.map(requirement => ({
            ...requirement,
            evidence: requirement.evidence ?? inferredClaimEvidence()
          }))
        }
      : {}),
    ...(summary.returns && !isUnknownClaim(summary.returns)
      ? { returns: annotateReturnEvidence(summary.returns) }
      : {}),
    ...(summary.variants
      ? {
          variants: summary.variants.map(variant => ({
            ...variant,
            summary: annotateClaimEvidence(variant.summary)
          }))
        }
      : {})
  };
}

// The native checker's own fail-closed contract-emission refusals all carry
// this exact prefix (`emit package contract: ...`, in
// rust/crates/solid-facts-backend/src/main.rs): an entry file that is not part
// of the project, an export with no semantic summary, an export-all it cannot
// statically expand, a dependency contract with no matching entrypoint, an
// entry file with no runtime ESM exports. Those are decisions, and the
// entrypoint that provoked one is legitimately refused.
//
// Everything else a non-zero exit can mean is not: a panic (exit 101), a
// producer handshake mismatch (exit 3), an unreadable project, a malformed
// dependency contract the loader rejected. Contract emission itself never
// exits non-zero merely because the analysis found violations -- the emitting
// run passes no `--certify`, and `snapshot_emission` only ever returns a
// non-zero code under it -- so a status here is an error, not a verdict.
const nativeRefusalPattern = /emit package contract:/;

function runChecked(args, options = {}) {
  const result = runNative("solid-checker", args, {
    ...options,
    encoding: "utf8",
    stdio: "pipe"
  });
  // A spawn failure is the launcher's problem, never an entrypoint's.
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const output = [result.stderr, result.stdout].filter(Boolean).join("\n");
    // Read the machine-readable boundary marker off the raw output, then drop
    // it from the message a human ever sees.
    const module = unresolvedDependencyModule(output);
    const message =
      withoutMarkers(output).trim() || `native solid-checker exited ${result.status}`;
    const error = nativeRefusalPattern.test(message) ? refuse(message) : new Error(message);
    if (module) error.unresolvedDependencyModule = module;
    throw error;
  }
  // The attribution notes ride a *successful* run's stderr: they explain the
  // unknown claims the run just wrote into the contract.
  result.attributions = unknownClaimAttributions(result.stderr);
  return result;
}

function dependencyPackageName(module) {
  if (typeof module !== "string" || module.startsWith(".") || module.startsWith("/")) {
    return undefined;
  }
  const parts = module.split("/");
  return module.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}

// The native checker names the package boundary it could not cross on one
// stable line of its own (`refuse_unresolved_dependency_module` in
// rust/crates/solid-facts-backend/src/main.rs). That line, not the human
// sentence beside it, is the interface between the two processes: it is what
// makes demand-driven dependency generation survive a reworded diagnostic.
// Recovering the specifier from prose instead means a reworded message stops
// the recursion silently -- the entrypoint is merely "refused", and a refusal
// exits 0.
const unresolvedDependencyMarkerPrefix = "solid-checker:unresolved-dependency-module=";

// Prose forms an older engine emitted. A native binary can lag this script --
// the launcher takes an overridden binary, and a published CLI runs whichever
// engine its platform package carries -- so these stay as a fallback rather
// than being replaced. They are strictly secondary: the marker wins whenever
// both are present.
//
// Each alternative names a real interface: the export-all refusal sentence in
// `rust/crates/solid-facts-backend/src/main.rs`, and the two
// `StaticDefectKind` variants that carry a `module` field
// (`PackageContractExportMissing`, `PackageContractEnvironmentDependent`, in
// `rust/crates/solid-reactive-ir/src/lib.rs`). A variant that no engine ever
// emitted does not belong here: it reads as evidence of a channel that exists,
// and the next reader has no way to tell the live alternatives from the
// invented one.
const unresolvedDependencyProse = [
  /cannot statically expand external export-all\s+"([^"]+)"/,
  /(?:PackageContractExportMissing|PackageContractEnvironmentDependent)\s*\{[^}]*\bmodule:\s*"([^"]+)"/
];

export function unresolvedDependencyModule(message) {
  const lines = String(message).split(/\r?\n/);
  for (const line of lines) {
    if (!line.startsWith(unresolvedDependencyMarkerPrefix)) continue;
    const module = line.slice(unresolvedDependencyMarkerPrefix.length).trim();
    if (module) return module;
  }
  for (const pattern of unresolvedDependencyProse) {
    const match = String(message).match(pattern);
    if (match) return match[1];
  }
  return undefined;
}

// Why one unknown claim is unknown, and which exports it was written onto.
//
// Schema v1's `unknownClaim` is `additionalProperties: false`, so the contract
// itself cannot carry the reason -- RFC 0002 rejected adding a field for
// exactly that: a loader that predates it hard-fails on the whole document
// rather than ignoring one property. The review plan is this project's own
// sidecar and has no such constraint, so the native emitter names each decision
// on one stable stderr line (`UNKNOWN_CLAIM_ATTRIBUTION_MARKER` in
// rust/crates/solid-facts-backend/src/main.rs) and the matching
// `unknown-sentinel` item records it under `because`.
//
// The line is JSON, not prose, for the same reason the dependency marker is a
// marker: attribution gained a rung (`enclosing-chain`, `reachability`) while
// this was being written, and a prose parser would have silently stopped
// recognising the new shape.
const unknownClaimAttributionPrefix = "solid-checker:unknown-claim-attribution=";

export function unknownClaimAttributions(text) {
  const notes = [];
  for (const line of String(text ?? "").split(/\r?\n/)) {
    if (!line.startsWith(unknownClaimAttributionPrefix)) continue;
    try {
      const note = JSON.parse(line.slice(unknownClaimAttributionPrefix.length));
      // A note naming no export is kept: it is the emitter reporting that the
      // attribution ladder resolved an obligation to *nothing*, which marks no
      // claim and so attaches to no review item. `narrowedAttributionNotes`
      // turns those into review-plan notes of their own; `attach` below simply
      // finds no export to hang them on. A note with no `exports` array at all
      // is still a half-note and still dropped.
      if (Array.isArray(note?.exports)) notes.push(note);
    } catch {
      // A malformed line is a marker this script does not understand -- a newer
      // engine, most likely. Ignoring it loses an explanation; failing the
      // generation over it would lose the contract.
    }
  }
  return notes;
}

// The markers are addressed to this script, not to a reviewer. Refusal reasons
// are quoted verbatim into the review plan (one line each), so strip both
// marker kinds there and leave the human sentence as the reason.
function withoutMarkers(text) {
  return text
    .split(/\r?\n/)
    .filter(
      line =>
        !line.startsWith(unresolvedDependencyMarkerPrefix) &&
        !line.startsWith(unknownClaimAttributionPrefix)
    )
    .join("\n");
}

function installedDependencyRoot(packageRoot, packageName) {
  let directory = packageRoot;
  while (true) {
    const candidate = join(directory, "node_modules", ...packageName.split("/"));
    const manifest = join(candidate, "package.json");
    if (existsSync(manifest) && statSync(manifest).isFile()) return candidate;
    const parent = dirname(directory);
    if (parent === directory) return undefined;
    directory = parent;
  }
}

async function ensureGeneratedDependencyContract({
  module,
  packageRoot,
  conditions,
  generationContext
}) {
  const packageName = dependencyPackageName(module);
  if (!packageName) return undefined;
  const dependencyRoot = installedDependencyRoot(packageRoot, packageName);
  if (!dependencyRoot || dependencyRoot === packageRoot) return undefined;
  const key = JSON.stringify([dependencyRoot, [...new Set(conditions)].sort()]);
  if (generationContext.contractCache.has(key)) {
    return generationContext.contractCache.get(key);
  }
  if (generationContext.active.has(key)) {
    throw refuse(
      `dependency contract cycle while generating ${packageName}; pass a reviewed contract with --contract`
    );
  }
  const output = join(
    generationContext.cacheDirectory,
    `${generationContext.contractCache.size}-${randomUUID()}.json`
  );
  await generatePackageContractInternal(
    [
      "--package-root",
      dependencyRoot,
      "--output",
      output,
      ...(conditions.length ? ["--conditions", [...new Set(conditions)].sort().join(",")] : []),
      ...generationContext.explicitContracts.flatMap(contract => ["--contract", contract])
    ],
    { ...generationContext, quiet: true, ownsCacheDirectory: false }
  );
  generationContext.contractCache.set(key, output);
  generationContext.generatedContracts.add(resolve(output));
  return output;
}

async function analyzeTarget({
  packageRoot,
  packageName,
  packageVersion,
  target,
  conditions = [],
  selectedConditions = [],
  contracts,
  temporaryDirectory,
  identifier,
  excludedTargets,
  resolver,
  generationContext
}) {
  const entryFile = packageLocalTarget(packageRoot, target);
  const excludedFiles = new Set(
    excludedTargets.map(target => packageLocalTarget(packageRoot, target))
  );
  const runtimeClosure = closureOf(packageRoot, resolver, entryFile, excludedFiles);
  const implementationFiles = runtimeClosure.files;
  const project = join(temporaryDirectory, `${identifier}-tsconfig.json`);
  const output = join(temporaryDirectory, `${identifier}.json`);
  const inventoryPath = join(temporaryDirectory, `${identifier}-inventory.json`);
  const probePlanPath = join(temporaryDirectory, `${identifier}-probe-plan.json`);
  const runtimeResolutionPath = join(temporaryDirectory, `${identifier}-runtime-resolutions.json`);
  writeFileSync(
    project,
    `${JSON.stringify(
      {
        compilerOptions: {
          allowJs: true,
          checkJs: true,
          jsx: "preserve",
          module: "ESNext",
          moduleResolution: "Bundler",
          skipLibCheck: true,
          target: "ES2022"
        },
        // Runtime files are explicit roots because a published ESM barrel's
        // `.js` specifiers often resolve to adjacent `.d.ts` files when only
        // the entrypoint is seeded. The native emitter filters unresolved
        // behavior by this entrypoint's exact runtime identities, so private
        // siblings cannot poison its public contract.
        files: implementationFiles
      },
      null,
      2
    )}\n`
  );
  writeFileSync(
    runtimeResolutionPath,
    `${JSON.stringify(
      {
        schemaVersion: 1,
        resolutions: runtimeClosure.resolutions
      },
      null,
      2
    )}\n`
  );
  try {
    const args = [
      "--project",
      project,
      "--emit-contract",
      output,
      "--emit-probe-plan",
      probePlanPath,
      // The same run's own answer to "which files did you open". The walk above
      // seeded `files`; this is what the program did with that seed, and it is
      // what the closure record is built from. Asked for only here, on a
      // generation run: it is a read of an already-built program, but it is
      // still two round trips and an ordinary analysis has no consumer for it.
      //
      // Passed unconditionally, and an engine that does not know the flag exits
      // non-zero on an unknown argument -- so a CLI newer than its native engine
      // fails the whole generation loudly instead of writing a contract whose
      // record is this process's own walk. That is the intended direction: the
      // record is fail-closed on a missing attestation, and a version skew is a
      // missing attestation it would be worse to paper over than to report. The
      // sentence does not match `nativeRefusalPattern`, so it is an error rather
      // than a per-entrypoint refusal, which would exit 0.
      "--emit-module-inventory",
      inventoryPath,
      // Exact importer/specifier/runtime-target triples from the same closure
      // walk that seeded `files` above. This is the missing declaration/runtime
      // identity seam: TypeScript may bind `./impl.js` through `impl.d.ts`, but
      // the published ESM graph still loads the exact `impl.js` target recorded
      // here. The native side accepts only bindings and runtime exports it can
      // join by compiler symbol; a missing or ambiguous join changes nothing.
      "--runtime-module-resolutions",
      runtimeResolutionPath,
      "--package-name",
      packageName,
      "--package-version",
      packageVersion,
      "--contract-entry-file",
      entryFile,
      "--contract-package-root",
      packageRoot
    ];
    // The export-map conditions that selected THIS runtime target are a fact
    // about the code being analyzed, not a guess: a `browser` branch's
    // implementation runs in a browser. Passing them through lets dependency
    // contracts with environment-dependent variants resolve to the variant
    // that actually applies here. Without them the consumer analysis has no
    // selected condition at all, so an environment-dependent dependency
    // contract can only fail closed -- which is correct given no selection,
    // but the selection was knowable all along. `--runtime-conditions` is the
    // documented channel for exactly this (see `selected_conditions`, whose
    // free-form set "carries export-map conditions such as `import`").
    // `default` is already excluded upstream in `collectRuntimeLeaves`.
    // `--conditions browser,import` is the caller stating which environment
    // this generation is for, so it seeds the runtime selection too. Without
    // that, a package whose own export map declares no host condition (a bare
    // `./*` with only `solid`/`default` branches, which is the common shape)
    // could never resolve a host-dependent dependency contract even when the
    // caller had already named the host.
    // `import` is unconditionally true of this analysis: only ESM runtime
    // leaves are ever analyzed (see `runtimeLeaf`), never a `require` branch.
    // It has to be stated explicitly rather than left implicit, because
    // `matches_entrypoint_conditions` treats an EMPTY selection specially --
    // it accepts an entrypoint whose conditions are all resolver conditions
    // (`default`/`import`/`require`) -- and switches to a plain intersection
    // once anything is selected. Passing `{solid}` alone therefore made
    // ordinary `import`-only dependency entrypoints stop matching, which
    // regressed three Solid 1.x packages that had generated cleanly before.
    // Naming `import` keeps the selection a superset of what the empty case
    // already allowed, so this can only add resolution, never remove it.
    const runtimeConditions = [...new Set([...conditions, ...selectedConditions, "import"])].sort();
    args.push("--runtime-conditions", runtimeConditions.join(","));
    // A contract this run generated from the dependency's own sources had its
    // `kind` decided by this exact rule, so the emitter may carry it across
    // the package boundary; a discovered or user-supplied one is trusted for
    // `kind` only when its own evidence records a review. `--contract` versus
    // `--generated-contract` is the only channel that distinguishes them.
    const contractFlag = path =>
      generationContext.generatedContracts.has(path) ? "--generated-contract" : "--contract";
    for (const contract of contracts) {
      const path = resolve(contract);
      args.push(contractFlag(path), path);
    }
    // A dependency obligation names the exact package boundary the analyzer
    // could not cross. Generate only that installed artifact, then retry with
    // its contract. Newly revealed obligations repeat this loop; a cache keeps
    // shared dependencies at one generation per condition set.
    const attemptedDependencies = new Set();
    let attributions = [];
    while (true) {
      try {
        attributions = runChecked(args, { cwd: packageRoot }).attributions;
        break;
      } catch (error) {
        if (error.message.includes("has no runtime ESM exports")) {
          return { exports: {}, attributions: [] };
        }
        // `runChecked` already read the marker off the raw native output; the
        // message it carries has had the marker stripped for the reviewer, so
        // re-parsing it can only reach the prose fallback.
        const module = error.unresolvedDependencyModule ?? unresolvedDependencyModule(error.message);
        const packageName = dependencyPackageName(module);
        if (!packageName || attemptedDependencies.has(packageName)) throw error;
        attemptedDependencies.add(packageName);
        const contract = await ensureGeneratedDependencyContract({
          module,
          packageRoot,
          conditions: runtimeConditions,
          generationContext
        });
        if (!contract) throw error;
        contracts.push(contract);
        const generated = resolve(contract);
        args.push(contractFlag(generated), generated);
      }
    }
    return {
      exports: expandContract(JSON.parse(readFileSync(output, "utf8"))).entrypoints["."].exports,
      // Read from the run that just succeeded, and read here rather than left to
      // the caller: the file lives in this generation's temporary directory and
      // is removed with the project below.
      inventory: readModuleInventory(inventoryPath),
      probePlan: JSON.parse(readFileSync(probePlanPath, "utf8")),
      // Relative to the package root so the plan describes the published
      // package rather than the temporary directory this run analyzed it in.
      attributions: attributions.map(note => ({
        ...note,
        path: packageLocalPath(packageRoot, note.path)
      }))
    };
  } finally {
    rmSync(project, { force: true });
    rmSync(inventoryPath, { force: true });
    rmSync(probePlanPath, { force: true });
    rmSync(runtimeResolutionPath, { force: true });
  }
}

function packageLocalPath(packageRoot, path) {
  if (typeof path !== "string") return path;
  const local = relative(packageRoot, path).replaceAll(sep, "/");
  return local && !local.startsWith("..") ? local : path;
}

/// Records, on each `unknown-sentinel` item, why that exact claim is unknown.
///
/// The plan's items are the contract's questions; the notes are the emitter's
/// reasons. Matching them on (entrypoint, export, claim domain) is what makes
/// the answer checkable: a reviewer reading "reactiveReads is unknown" now also
/// reads which obligation forced it, where it is, and how emission decided the
/// claim belonged to *this* export -- including when it decided by marking
/// every export because nothing identified the obligation's function.
///
/// Notes with no matching item are dropped rather than collected elsewhere. An
/// item can disappear between emission and the plan (normalization collapses
/// variants, an alias unification merges two exports), and a floating reason
/// with nothing to attach to reads as a claim the document does not contain.
/// The narrowing decisions, as review-plan notes.
///
/// An obligation the ladder resolved to no export marks no claim, so there is
/// no `unknown-sentinel` item to explain -- and the resulting contract is
/// indistinguishable from one where the analyzer never saw the obligation. The
/// difference matters: the second is silence, the first is a *decision* that
/// no export of this entrypoint can reach a proof obligation the analyzer did
/// see. That decision is exactly what a reviewer has to check, and it rides
/// the plan the same way an artifact-binding gap does -- a note nothing in the
/// contract's bytes can confirm or deny.
export function narrowedAttributionNotes(notes) {
  return notes
    .filter(note => Array.isArray(note.exports) && note.exports.length === 0)
    .map(note => {
      const where =
        note.path === undefined ? "" : ` at ${note.path}:${note.startByte}-${note.endByte}`;
      const entrypoint = note.entrypoint ? `${note.entrypoint}: ` : "";
      return (
        `${entrypoint}the ${note.obligation} obligation${where}` +
        `${note.analysisContext ? ` (${note.analysisContext})` : ""} was attributed to no ` +
        `export by \`${note.mechanism}\`, so no claim was marked unknown for it; check that ` +
        `no export of this entrypoint can reach it`
      );
    })
    .sort();
}

function attachUnknownClaimAttributions(items, notes) {
  if (!notes.length) return items;
  const byTarget = new Map();
  for (const note of notes) {
    for (const exportName of note.exports ?? []) {
      for (const domain of note.domains ?? []) {
        const key = JSON.stringify([note.entrypoint, exportName, domain]);
        const attributions = byTarget.get(key) ?? [];
        const attribution = {
          obligation: note.obligation,
          mechanism: note.mechanism,
          ...(note.analysisContext ? { analysisContext: note.analysisContext } : {}),
          ...(note.path === undefined
            ? {}
            : { location: `${note.path}:${note.startByte}-${note.endByte}` })
        };
        // One obligation reported once per (export, domain) even when several
        // analyses of the same target ran: a reviewer counts reasons, not runs.
        if (!attributions.some(existing => sameAttribution(existing, attribution))) {
          attributions.push(attribution);
        }
        byTarget.set(key, attributions);
      }
    }
  }
  return items.map(item => {
    if (item.kind !== "unknown-sentinel") return item;
    const attributions = byTarget.get(
      JSON.stringify([item.target.entrypoint, item.target.export, item.target.field])
    );
    if (!attributions?.length) return item;
    return {
      ...item,
      because: {
        attributions: [...attributions].sort((left, right) =>
          JSON.stringify(left).localeCompare(JSON.stringify(right))
        )
      }
    };
  });
}

function sameAttribution(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

/// Records, on each `unknown-sentinel` item a *merge* produced, which branches
/// disagreed and about what.
///
/// The other sentinel emitter is `mergeSummaries`, and it used to be silent: a
/// reviewer read "returns is unknown" with nothing saying that the browser
/// branch proved an accessor and the node branch proved none, which is exactly
/// the fact that decides the answer. The generator knows it at the moment it
/// writes the sentinel, so it says it.
///
/// Only top-level domains are recorded, because that is where a merge writes:
/// `mergeSummaries` builds the environment-unaware base and leaves the exact
/// per-branch summaries in `variants`.
export function attachMergeDivergences(items, divergences) {
  if (!divergences.length) return items;
  const byTarget = new Map();
  for (const divergence of divergences) {
    if (divergence.domain === "kind") continue; // no sentinel: `value` is the merged surface
    const key = JSON.stringify([divergence.entrypoint, divergence.export, divergence.domain]);
    const notes = byTarget.get(key) ?? [];
    const note = {
      mechanism: divergence.mechanism,
      branches: [...divergence.branches].sort(),
      detail:
        `the ${divergence.domain} claim diverges across ${divergence.branches.length} branch(es) ` +
        `(${[...divergence.branches].sort().join(", ")}): ${divergence.shape}, so the ` +
        "environment-unaware base can only be the unknown sentinel; the exact per-branch claims " +
        "are in variants"
    };
    if (!notes.some(existing => sameAttribution(existing, note))) notes.push(note);
    byTarget.set(key, notes);
  }
  return items.map(item => {
    if (item.kind !== "unknown-sentinel") return item;
    const notes = byTarget.get(
      JSON.stringify([item.target.entrypoint, item.target.export, item.target.field])
    );
    if (!notes?.length) return item;
    return {
      ...item,
      because: {
        ...item.because,
        divergences: [...notes].sort((left, right) =>
          JSON.stringify(left).localeCompare(JSON.stringify(right))
        )
      }
    };
  });
}

async function generatePackageContractInternal(arguments_, context) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    process.stdout.write(packageContractHelp);
    return;
  }
  const options = parseArguments(arguments_);
  const packageRoot = resolve(options.packageRoot);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (!manifest.name || !manifest.version) {
    throw refuse(
      `${manifestPath} must declare name and version for package contract generation`
    );
  }
  const legacyProvenance = legacyRootProvenance(manifest);
  // One resolver for the whole generation: the `imports` map and the condition
  // selection are properties of this package and this run, and a walker that
  // guessed either would put one branch's bytes behind another branch's claims.
  const moduleResolver = createModuleResolver({
    packageRoot,
    manifest,
    conditions: options.conditions
  });
  const packageExports = manifest.exports ?? legacyPackageExports(packageRoot, manifest);
  if (!packageExports) {
    throw refuse(`${manifest.name} has no supported ESM runtime entrypoints`);
  }
  const discovered = concreteEntrypoints(
    packageRoot,
    packageExports,
    options.conditions
  );
  // `--conditions` collapses each entrypoint to the single leaf that selection
  // resolves, which erases whether the export map was conditional at all. The
  // unselected discovery answers that question, and only that question: an
  // entrypoint with one unconditional branch needs no condition record, while
  // one with real branches must carry the environment its summary was proven
  // for. This resolves paths only -- it runs no analysis.
  const unconditionalDiscovery = options.conditions.length
    ? concreteEntrypoints(packageRoot, packageExports, [])
    : discovered;
  const conditionalEntrypoints = new Set(
    [...unconditionalDiscovery]
      .filter(
        ([, variants]) =>
          variants.length > 1 || variants.some(variant => variant.conditions.length > 0)
      )
      .map(([entrypoint]) => entrypoint)
  );
  const selected = options.entrypoints.length
    ? new Map(
        options.entrypoints.map(entrypoint => {
          const variants = discovered.get(entrypoint);
          if (!variants) throw refuse(`package has no runtime entrypoint ${entrypoint}`);
          return [entrypoint, variants];
        })
      )
    : discovered;
  if (selected.size === 0) {
    throw refuse(`${manifest.name} has no supported ESM runtime entrypoints`);
  }

  const output = resolve(options.output || defaultOutput(packageRoot, manifest.name));
  const generationContext = context ?? {
    active: new Set(),
    contractCache: new Map(),
    // The contracts THIS run generated, by resolved path. Provenance the
    // documents cannot carry: they are `inferred` like any generated contract,
    // and the native emitter needs to tell them from a contract merely found
    // at `node_modules/<dep>/solid-reactivity.json` before it will carry an
    // export `kind` across the boundary unproved. See `--generated-contract`.
    generatedContracts: new Set(),
    cacheDirectory: mkdtempSync(join(tmpdir(), "solid-checker-dependency-contracts-")),
    explicitContracts: options.contracts.map(contract => resolve(contract)),
    quiet: false,
    ownsCacheDirectory: true
  };
  const generationKey = JSON.stringify([
    packageRoot,
    [...new Set(options.conditions)].sort()
  ]);
  if (generationContext.active.has(generationKey)) {
    throw refuse(
      `dependency contract cycle while generating ${manifest.name}; pass a reviewed contract with --contract`
    );
  }
  generationContext.active.add(generationKey);
  const contracts = [
    ...new Set([
      ...options.contracts.map(contract => resolve(contract)),
      ...dependencyContracts(packageRoot, manifest)
    ])
  ];
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "solid-checker-contract-"));
  const entrypoints = {};
  const probePlans = {};
  // Declared alongside `entrypoints` because both outlive the analysis
  // block: the review plan and the stdout summary are written after it.
  const refusedEntrypoints = [];
  const refusedExports = [];
  // Same reason: the artifact-binding notes are decided once the emitted
  // entrypoint set is final, and the stdout summary recomputes the review
  // plan after the analysis block has been left.
  let artifactNotes = [];
  // Same reason again: the stdout summary and the returned result report the
  // plan the emission block built.
  let reviewItems = [];
  // The per-entrypoint closure record, needed by the artifact-binding notes
  // before the contract is written and by the review plan after it.
  let closures = { generator: generatorIdentity(), entrypoints: {} };
  // Why each unknown claim in the emitted contract is unknown, collected off
  // the native emitter's stderr as the analyses run and attached to the review
  // plan's `unknown-sentinel` items once the plan exists.
  const attributionNotes = [];
  // Why each *merge*-produced unknown claim is unknown. `mergeSummaries` is the
  // other emitter of the sentinel and it used to say nothing: a reviewer saw
  // "returns is unknown" with no hint that two conditional branches proved
  // different things, or that one proved a return and the other proved none.
  // The generator knows both; these notes carry it onto the plan item.
  const mergeDivergences = [];
  // Where the previous reviewed triple was moved to, when this generation
  // overwrote one. Reported on stdout so the documented upgrade sequence is
  // executable rather than aspirational.
  let snapshot;
  const targetsByEntrypoint = new Map();
  // Conditional exports frequently point several public entrypoints at the
  // same runtime target. Retain that target analysis for this generation
  // instead of rebuilding TypeScript, Reactive IR, and dependency contracts
  // once per public alias.
  const targetAnalyses = new Map();
  // Public names are a property of the target module itself. Conditions may
  // change dependency summaries, but two analyses of one target disagreeing
  // about its export identities makes open-load attribution ambiguous.
  const targetExportNames = new Map();
  // What each analyzed target's own program reported it opened. The closure
  // record is built from this, not from the walk that seeded the program.
  const inventories = moduleInventories();
  try {
    let ordinal = 0;
    for (const [entrypoint, variants] of [...selected].sort(([left], [right]) =>
      left.localeCompare(right)
    )) {
      try {
        const exports = {};
        const conditionalSummaries = new Map();
        // The condition lists already folded into `exports[name]`, so a
        // divergence note can name both sides of the merge that produced it.
        const mergedBranches = new Map();
        const conditions = new Set();
        const targets = new Set();
        let singleProbePlan;
        // Only a genuinely branching entrypoint carries conditions. For one
        // unconditional target there is no environment to record, and claiming
        // one would mark a summary that holds everywhere as conditional.
        const branching = conditionalEntrypoints.has(entrypoint);
        // `--conditions` is an *assertion* about the resolving environment, not
        // an observation of the export map: it suppresses every branch the
        // selection does not take, so the resulting summary is only valid where
        // that selection holds. Schema v1 cannot say "and not development", so
        // the selection itself is what the entrypoint has to carry -- recording
        // `default` here instead would mark the contract unconditional and let
        // a development consumer apply a production-only summary.
        if (branching) {
          for (const condition of options.conditions) conditions.add(condition);
        }
        for (const variant of variants) {
          // With no selection, a fallback branch carries no condition name, but
          // the entrypoint it resolves really is reachable from every
          // environment. Recording nothing for it made the entrypoint look
          // conditional on the named branches alone, so a consumer selecting
          // any other environment was gated out of an entrypoint that actually
          // resolves there. `default` is the spelling the variant list below
          // synthesizes for the same branch.
          if (branching && variant.conditions.length === 0 && options.conditions.length === 0) {
            conditions.add("default");
          }
          variant.conditions.forEach(condition => conditions.add(condition));
          targets.add(variant.target);
        }
        targetsByEntrypoint.set(entrypoint, targets);
        for (const variant of variants) {
          const target = variant.target;
          const excludedTargets = [...targets]
            .filter(candidate => candidate !== target)
            .sort();
          // Conditions are part of the key because the analysis result now
          // depends on them: two entrypoints sharing one runtime target under
          // different conditions can legitimately resolve different dependency
          // contract variants, and reusing the first analysis for the second
          // would silently attribute one environment's summary to the other.
          const analysisConditions = [...variant.conditions].sort();
          const analysisKey = JSON.stringify([target, excludedTargets, analysisConditions]);
          let observed = targetAnalyses.get(analysisKey);
          if (!observed) {
            observed = await analyzeTarget({
              packageRoot,
              packageName: manifest.name,
              packageVersion: manifest.version,
              target,
              conditions: analysisConditions,
              selectedConditions: options.conditions,
              contracts,
              temporaryDirectory,
              identifier: `${ordinal++}-${randomUUID()}`,
              excludedTargets,
              resolver: moduleResolver,
              generationContext
            });
            targetAnalyses.set(analysisKey, observed);
          }
          if (variants.length === 1) singleProbePlan = observed.probePlan?.exports ?? {};
          // Recorded on every variant, not only on a fresh analysis, so a target
          // reached under two condition sets is checked for inventory agreement
          // rather than silently taking the first one's answer.
          inventories.record(target, excludedTargets, observed.inventory);
          const names = Object.keys(observed.exports).sort();
          const recordedNames = targetExportNames.get(target);
          if (recordedNames === undefined) targetExportNames.set(target, names);
          else if (recordedNames !== null && JSON.stringify(recordedNames) !== JSON.stringify(names)) {
            targetExportNames.set(target, null);
          }
          for (const note of observed.attributions) {
            attributionNotes.push({ ...note, entrypoint });
          }
          for (const [name, summary] of Object.entries(observed.exports)) {
            const variantsForName = conditionalSummaries.get(name) ?? [];
            variantsForName.push({
              conditions: [...variant.conditions],
              summary,
              precedence: variant.precedence
            });
            conditionalSummaries.set(name, variantsForName);
            const branchLabel = variant.conditions.length
              ? [...variant.conditions].sort().join("+")
              : "default";
            const alreadyMerged = mergedBranches.get(name) ?? [];
            const merged = exports[name]
              ? mergeSummaries(exports[name], summary, (domain, shape) =>
                  mergeDivergences.push({
                    entrypoint,
                    export: name,
                    domain,
                    shape,
                    branches: [...alreadyMerged, branchLabel],
                    mechanism: "conditional-branch-merge"
                  })
                )
              : summary;
            mergedBranches.set(name, [...alreadyMerged, branchLabel]);
            if (!merged) {
              throw refuse(
                `${manifest.name} ${entrypoint}:${name} has incompatible semantics across conditional targets: ${JSON.stringify(exports[name])} versus ${JSON.stringify(summary)}`
              );
            }
            exports[name] = merged;
          }
        }
        for (const [name, summaries] of conditionalSummaries) {
          const minimalSummaries = removeRedundantConditionalSummaries(summaries);
          const distinct = new Map(
            minimalSummaries.map(variant => [semanticSummaryKey(variant.summary), variant.summary]),
          );
          // A name observed in only some of this entrypoint's conditional
          // branches is *absent* from the others. Schema v1 cannot say "not
          // exported here", and an unconditional summary would hand a consumer
          // in the other environment a complete claim about an export that does
          // not exist there. Emitting the branches it is actually proven for
          // keeps that consumer environment-gated, so the unproven environment
          // fails closed instead of inheriting the other one's semantics.
          const conditionallyPresent = summaries.length < variants.length;
          if (distinct.size > 1 || conditionallyPresent) {
            // Overlapping branches with different semantics are only ambiguous if
            // nothing says which one wins. The export map does say: it is ordered
            // and first-match-wins, and `precedence` carries that order. The
            // consumer resolves such a set by lowest precedence, but only when
            // every matching variant declares one and the minimum is unique --
            // so a set that cannot satisfy that is still refused here rather than
            // emitted as something the loader would only fail closed on later.
            // A variant is a complete export summary, including its kind. The
            // merged base remains the conservative `value` summary when kinds
            // diverge, while an environment-aware consumer selects the exact
            // function/value branch below. This keeps environment-unaware
            // consumers fail-closed without discarding an export-map fact.
            const orderable = minimalSummaries.every(
              variant => Number.isInteger(variant.precedence)
            ) && new Set(minimalSummaries.map(variant => variant.precedence)).size ===
              minimalSummaries.length;
            if (distinct.size > 1 && !orderable) {
              for (let left = 0; left < minimalSummaries.length; left++) {
                for (let right = left + 1; right < minimalSummaries.length; right++) {
                  if (
                    !sameSummary(minimalSummaries[left].summary, minimalSummaries[right].summary) &&
                    conditionBranchesOverlap(
                      minimalSummaries[left].conditions,
                      minimalSummaries[right].conditions,
                    )
                  ) {
                    throw refuse(
                      `${manifest.name} ${entrypoint}:${name} has different semantics across overlapping conditional-export branches ${JSON.stringify(minimalSummaries[left].conditions)} and ${JSON.stringify(minimalSummaries[right].conditions)} with no resolvable export-map order; split the entrypoint or review an explicit contract`
                    );
                  }
                }
              }
            }
            exports[name] = {
              ...exports[name],
              variants: minimalSummaries
                .map(variant => ({
                  conditions: variant.conditions.length
                    ? [...variant.conditions].sort()
                    : ["default"],
                  summary: variant.summary,
                  ...(Number.isInteger(variant.precedence)
                    ? { precedence: variant.precedence }
                    : {})
                }))
                .sort(
                  (left, right) =>
                    JSON.stringify(left.conditions).localeCompare(
                      JSON.stringify(right.conditions),
                    ) || JSON.stringify(left.summary).localeCompare(JSON.stringify(right.summary)),
                )
            };
          }
        }
        if (Object.keys(exports).length === 0) {
          continue;
        }
        entrypoints[entrypoint] = {
          exports: Object.fromEntries(
            Object.entries(exports)
              .map(([name, summary]) => [name, annotateClaimEvidence(summary)])
              .sort(([left], [right]) => left.localeCompare(right))
          ),
          ...(conditions.size ? { conditions: [...conditions].sort() } : {})
        };
        if (singleProbePlan && Object.keys(singleProbePlan).length) {
          probePlans[entrypoint] = singleProbePlan;
        }
      } catch (error) {
        // Per-ENTRYPOINT granularity, deliberately not per-target: if any one
        // conditional target of this entrypoint could not be analyzed, we do
        // not know that environment's behavior, and merging only the branches
        // that did succeed would assert one environment's semantics for all of
        // them. So the whole entrypoint is refused -- but the other
        // entrypoints, which were analyzed independently, are still emitted.
        // A refused entrypoint is absent from the contract, so a consumer
        // importing it gets an explicit uncertifiable result rather than a
        // wrong claim, and the review plan lists it.
        //
        // Only a *typed* refusal earns that treatment. Anything else reaching
        // here -- a bug in this generator, an unreadable or malformed file, a
        // crashed native process -- is not knowledge about this entrypoint,
        // and converting it into "refused and omitted" would turn a run that
        // used to fail loudly into an exit-0 partial contract. Fail the whole
        // generation instead.
        if (!isGenerationRefusal(error)) throw error;
        refusedEntrypoints.push({
          entrypoint,
          reason: String(error?.message ?? error).split("\n")[0]
        });
        continue;
      }
    }

    if (Object.keys(entrypoints).length === 0) {
      // Every entrypoint refused. Report the first real reason rather than the
      // generic "no runtime ESM exports", which would misdescribe a package
      // that has runtime exports the generator could not certify.
      if (refusedEntrypoints.length) {
        throw refuse(
          `${manifest.name} has no certifiable runtime entrypoint; ${refusedEntrypoints[0].entrypoint}: ${refusedEntrypoints[0].reason}`
        );
      }
      throw refuse(`${manifest.name} has no runtime ESM exports`);
    }
    // A shared runtime target is an analysis-cache opportunity, not proof that
    // two public entrypoints are semantic aliases. One entrypoint can select
    // that target unconditionally while another selects it only as one branch
    // alongside condition-specific implementations. Each entrypoint already
    // merged its own variants above; folding whole summaries across entrypoint
    // boundaries would leak the second entrypoint's other branches into the
    // first (for example @solidjs/web's server-only `ssrGroup` identity into
    // `./jsx-runtime`, which always resolves to the void web implementation).
    // Keep the summaries scoped here. `targetAnalyses` still shares the exact
    // target/condition analysis without sharing its public projection.

    // Computed before the review items, not after the plan: a specifier the walk
    // could not resolve and the program did is a hole in the artifact binding,
    // and the checklist section that names binding holes has to carry it. And
    // before the binding, because the binding's unpinned-module count is read
    // off this record rather than re-walked -- one attested answer, not two
    // independent walks that nothing forced to agree.
    closures = generationClosures(
      output,
      packageRoot,
      moduleResolver,
      targetsByEntrypoint,
      entrypoints,
      legacyProvenance,
      inventories,
      targetExportNames
    );
    // A scoped open-load obligation withdraws exactly the exports whose
    // functions can reach it. Missing exports are the schema-v1 fail-closed
    // spelling: a consumer asks for one and receives an explicit
    // uncertifiable result, while unrelated exports retain their independently
    // proven summaries. Ambiguous attribution stays in `runtimeNotes` and
    // continues to refuse the entire entrypoint during verification.
    for (const [entrypoint, closure] of Object.entries(closures.entrypoints)) {
      const affected = new Set(
        (closure.runtimeObligations ?? []).flatMap(obligation => obligation.exports ?? [])
      );
      for (const name of affected) {
        if (!entrypoints[entrypoint]?.exports?.[name]) continue;
        delete entrypoints[entrypoint].exports[name];
        refusedExports.push({
          entrypoint,
          export: name,
          reason: "its exact call graph can reach an open runtime module load"
        });
      }
      if (entrypoints[entrypoint] && Object.keys(entrypoints[entrypoint].exports).length === 0) {
        delete entrypoints[entrypoint];
        refusedEntrypoints.push({
          entrypoint,
          reason: "every export can reach an open runtime module load"
        });
      }
    }
    if (Object.keys(entrypoints).length === 0) {
      throw refuse(`${manifest.name} has no certifiable runtime entrypoint after open-load attribution`);
    }
    const binding = contractArtifacts(
      output,
      packageRoot,
      targetsByEntrypoint,
      entrypoints,
      closures
    );
    artifactNotes = [
      ...binding.notes,
      ...closureEnumerationNotes(closures),
      ...narrowedAttributionNotes(attributionNotes)
    ];
    const contract = {
      schemaVersion: 1,
      package: { name: manifest.name, version: manifest.version },
      compilerFactsProtocol: 1,
      // Emitted only where schema v1 can carry it honestly. The consumer
      // verifies every artifact hash it finds, so a wrong one fails the
      // `--validate-contract` call below rather than shipping.
      artifacts: binding.artifacts,
      entrypoints,
      evidence: { kind: "inferred", generator: "solid-checker package generator" }
    };
    mkdirSync(dirname(output), { recursive: true });
    const normalized = normalizeContract(contract);
    const candidate = `${output}.tmp-${randomUUID()}`;
    writeFileSync(candidate, `${JSON.stringify(normalized, null, 2)}\n`);
    try {
      runChecked(["--validate-contract", candidate]);
      // The snapshot happens here and not earlier: a generation that fails
      // before this point must leave the reviewed triple exactly where it was,
      // and a generation that reaches it is about to destroy the bytes the
      // previous review was recorded against.
      snapshot = snapshotPreviousReview(output);
      renameSync(candidate, output);
    } finally {
      rmSync(candidate, { force: true });
    }
    // Derived from the document that was just written, not from the
    // pre-normalization model: normalization collapses evidence-only variants,
    // and an item naming a variant the contract does not have is one no
    // reviewer can resolve and no promotion gate can check.
    reviewItems = attachMergeDivergences(
      attachUnknownClaimAttributions(
        collectReviewItems(
          expandContract(normalized).entrypoints,
          selected,
          refusedEntrypoints,
          legacyProvenance?.text,
          artifactNotes
        ),
        attributionNotes
      ),
      mergeDivergences
    );
    const review = renderReviewPlan(manifest.name, manifest.version, output, reviewItems);
    writeFileSync(reviewPlanPath(output), review.text);
    // The same plan, machine-readable: `solid-checker contract review` resolves
    // it item by item, and promotion is refused until every one is decided.
    // It is bound to the bytes just written: a plan is a set of questions about
    // one exact document, and beside another document it is a set of questions
    // nobody asked about it.
    writeFileSync(
      reviewPlanJsonPath(output),
      `${JSON.stringify(
        renderReviewPlanDocument(
          manifest.name,
          manifest.version,
          reviewItems,
          closures,
          sha256Artifact(output)
        ),
        null,
        2
      )}\n`
    );
    const constructionPlanPath = output.toLowerCase().endsWith(".json")
      ? `${output.slice(0, -5)}.probe-plan.json`
      : `${output}.probe-plan.json`;
    writeFileSync(
      constructionPlanPath,
      `${JSON.stringify(
        {
          schemaVersion: 2,
          contract: sha256Artifact(output),
          source: "typescript-value-domain",
          package: { name: manifest.name, version: manifest.version },
          entrypoints: probePlans
        },
        null,
        2
      )}\n`
    );
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
    generationContext.active.delete(generationKey);
  }
  const reviewOutput = reviewPlanPath(output);
  const planOutput = reviewPlanJsonPath(output);
  const partial = [
    refusedEntrypoints.length
      ? `${refusedEntrypoints.length} entrypoint(s) refused and omitted`
      : "",
    refusedExports.length ? `${refusedExports.length} export(s) refused and omitted` : ""
  ].filter(Boolean);
  if (!generationContext.quiet) {
    process.stdout.write(
      `generated ${manifest.name}@${manifest.version} contract with ${Object.keys(entrypoints).length} entrypoints at ${output}${partial.length ? `; ${partial.join("; ")}` : ""}; review plan ${reviewOutput} and ${planOutput} (${reviewItems.length} checklist items)\n`
    );
    if (snapshot?.verified) {
      // No transfer command here, and that is the whole point of the tier: a
      // machine verification is reproduced from the installed artifact, so the
      // upgrade is two commands and no decision.
      process.stdout.write(
        `the previous machine-verified contract and its probe/verify reports were kept at ` +
          `${snapshot.contract}; a verification is reproduced rather than transferred, so ` +
          `re-run: solid-checker contract probe ${output} --write && ` +
          `solid-checker contract verify ${output}\n`
      );
    } else if (snapshot) {
      process.stdout.write(
        `the previous reviewed contract and its review state were kept at ${snapshot.contract}; ` +
          `carry that review forward with: solid-checker contract review ${output} ` +
          `--transfer-from ${snapshot.contract}\n`
      );
    }
  }
  return {
    package: manifest.name,
    version: manifest.version,
    contract: output,
    review: reviewOutput,
    reviewPlan: planOutput,
    entrypoints: Object.keys(entrypoints).length,
    refusedEntrypoints: refusedEntrypoints.length,
    refusedExports: refusedExports.length,
    reviewItems: reviewItems.length,
    ...(snapshot ? { previousContract: snapshot.contract } : {})
  };
}

export async function generatePackageContract(arguments_, { quiet = false } = {}) {
  const options = parseArguments(arguments_);
  const cacheDirectory = mkdtempSync(join(tmpdir(), "solid-checker-dependency-contracts-"));
  const generationContext = {
    active: new Set(),
    contractCache: new Map(),
    generatedContracts: new Set(),
    cacheDirectory,
    explicitContracts: options.contracts.map(contract => resolve(contract)),
    quiet,
    ownsCacheDirectory: true
  };
  try {
    return await generatePackageContractInternal(arguments_, generationContext);
  } finally {
    rmSync(cacheDirectory, { recursive: true, force: true });
  }
}
