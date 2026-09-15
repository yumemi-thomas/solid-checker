// Pins what a package's contract says about a name it re-exports from a
// dependency whose contract it was generated against.
//
// The name is not this package's to describe: `export { clean } from "dep"`
// publishes `dep`'s exact runtime binding under this package's identity, and
// the only thing that knows what it does is `dep`'s own accepted contract.
// Two separate mechanics used to lose that.
//
//  1. Generation runs the whole project analysis over the re-exporting module,
//     and that analysis emits an export fragment for every specifier it sees.
//     An external re-export has no local declaration to walk, so its fragment
//     degrades to the bare value summary. Consulting the analysis first let
//     that degenerate entry shadow the dependency projection for every *named*
//     re-export, leaving the projection reachable only for `export *`.
//
//  2. Re-exporting a dependency name whose own contract leaves claim domains
//     open raises an open-claims obligation *at the re-export statement*,
//     which encloses no function -- so the attribution ladder falls through to
//     its widest mechanism and marks every export in the map unknown.
//
// `@kobalte/utils` hit both: it published `access`, `Key` and `mergeRefs` as
// `{"call":{}}` against an accepted `@solid-primitives/utils` contract that
// states `access`'s callbacks claim outright, and it raised 18 of those
// fallback-all obligations from its nine cross-package re-exports.
//
// An ESM re-export binding is immutable and its target lives in the
// dependency's archive, so no obligation raised in this package is evidence
// about that function's body. What this does *not* change is the local side:
// `mixed`'s own `local` is still marked unknown by the same obligation, which
// is the existing conservatism and is pinned here rather than assumed.

import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";
import { mergeProposalDependencies } from "../packages/cli/scripts/certify-contract.mjs";
import { generatePackageContract } from "../packages/cli/scripts/generate-package-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    "the dependency re-export pin needs fresh native and Type Facts binaries; " +
      "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN"
  );
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function manifest(name, dependencies) {
  return `${JSON.stringify({
    name,
    version: "1.0.0",
    type: "module",
    exports: { ".": { types: "./dist/index.d.ts", default: "./dist/index.js" } },
    ...(dependencies ? { dependencies } : {})
  })}\n`;
}

/// What a document *claims closed* about one export, and what it merely
/// proposes closed. A re-exported name is described by its dependency's
/// contract, so both lists must match the dependency's for the same name: the
/// parent has no implementation of its own to say anything further.
function closure(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(summary, `${exportName} must keep an exact export identity`);
  return {
    closed: [...(summary.call?.closed ?? [])].sort(),
    proposed: [...(summary.call?.proposedClosures ?? [])].sort()
  };
}

/// The one claim both packages must make about the same function value:
/// argument 0 is invoked, on the caller's own stack, untracked.
function invokedFirstArgument(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(summary, `${exportName} must keep an exact export identity`);
  const operations = new Map(
    (summary.call?.operations ?? []).map(operation => [operation.id, operation])
  );
  return (summary.call?.callbacks ?? [])
    .filter(callback => callback.from?.arg === 0 && callback.from?.path?.length === 0)
    .map(callback => operations.get(callback.operation))
    .map(operation => `${operation?.kind}:${operation?.at?.schedule}:${operation?.tracking}`)
    .sort();
}

describe("a contract describes what it re-exports from an accepted dependency", () => {
  let directory;
  let dependency;
  const consumers = {};

  beforeAll(async () => {
    // Canonical, not the `/var` symlink macOS hands out: the generation-time
    // dependency index is keyed by the exact importer path, and a resolver
    // that realpaths one side of that key binds nothing.
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-dependency-reexport-")));
    const modules = join(directory, "node_modules");

    // `clean` states its claim outright. `opaque` cannot: its value comes from
    // an unresolved global, so every domain stays open and a consumer that
    // re-exports it inherits that openness as an obligation.
    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "export function clean(handler) {\n  handler();\n}\n" +
        "export const opaque = globalThis.hostFactory();\n"
    );
    write(
      join(modules, "depkg/dist/index.d.ts"),
      "export declare function clean(handler: () => void): void;\n" +
        "export declare const opaque: (node: Element) => unknown;\n"
    );

    const consumerSources = {
      // Only the proven name. Pins mechanic 1 on its own.
      reexporter: 'export { clean } from "depkg";\n',
      // The proven name beside the open one. Pins mechanic 2.
      mixed: 'export { clean, opaque } from "depkg";\n'
    };
    const consumerDeclarations = {
      reexporter: 'export { clean } from "depkg";\n',
      mixed: 'export { clean, opaque } from "depkg";\n'
    };
    for (const [name, source] of Object.entries(consumerSources)) {
      write(join(modules, name, "package.json"), manifest(name, { depkg: "1.0.0" }));
      write(
        join(modules, name, "dist/index.js"),
        `${source}\nexport function local(handler) {\n  handler();\n}\n`
      );
      write(
        join(modules, name, "dist/index.d.ts"),
        `${consumerDeclarations[name]}export declare function local(handler: () => void): void;\n`
      );
    }

    const dependencyOutput = join(directory, "depkg.json");
    const generated = await generatePackageContract(
      [
        "--package-root", join(modules, "depkg"),
        "--output", dependencyOutput,
        "--integrity", "sha512-dependency",
        "--entrypoint", ".",
        "--certification-importer", join(modules, "reexporter/dist/index.js")
      ],
      { quiet: true }
    );
    dependency = JSON.parse(readFileSync(dependencyOutput, "utf8"));

    // The exact artifact case the dependency proposal describes; the private
    // graph index binds the projection to this identity and to nothing else.
    const plan = JSON.parse(readFileSync(generated.plan, "utf8"));
    const artifactCases = new Set();
    (function visit(value) {
      if (Array.isArray(value)) for (const child of value) visit(child);
      else if (value && typeof value === "object") {
        if (typeof value.artifactCase === "string") artifactCases.add(value.artifactCase);
        for (const child of Object.values(value)) visit(child);
      }
    })(plan);
    assert.equal(artifactCases.size, 1, "the dependency must resolve one exact artifact case");

    for (const name of Object.keys(consumerSources)) {
      const importer = join(modules, name, "dist/index.js");
      const merged = mergeProposalDependencies(
        [
          {
            viaSpecifier: "depkg",
            node: { packageName: "depkg", importer },
            planning: {
              proposal: dependencyOutput,
              resolution: resolvePackageArtifacts({
                importer,
                specifier: "depkg",
                conditions: [],
                integrity: "sha512-dependency"
              })
            },
            demandPlan: {
              selectedArtifactCase: [...artifactCases][0],
              candidateSemanticDigest: plan.semanticDigest
            },
            reexportImporters: [importer]
          }
        ],
        join(directory, `${name}-graph`)
      );
      const output = join(directory, `${name}.json`);
      await generatePackageContract(
        [
          "--package-root", join(modules, name),
          "--output", output,
          "--integrity", `sha512-${name}`,
          "--entrypoint", ".",
          "--certification-importer", join(directory, "app.ts")
        ],
        {
          quiet: true,
          proposalDependencies: merged.proposalDependencies,
          proposalDependencyCatalog: merged.catalog,
          privateGraphPreparation: true
        }
      );
      consumers[name] = JSON.parse(readFileSync(output, "utf8"));
    }
  }, 300_000);

  afterAll(() => {
    if (directory) rmSync(directory, { recursive: true, force: true });
  });

  test("the dependency states the claim it owns", () => {
    expect(invokedFirstArgument(dependency, "clean")).toEqual(["invoke:same-stack:untracked"]);
  });

  test("the re-exported name carries the dependency's claim, not an empty summary", () => {
    expect(invokedFirstArgument(consumers.reexporter, "clean")).toEqual([
      "invoke:same-stack:untracked"
    ]);
  });

  test("a locally declared export is still described by the local analysis", () => {
    expect(invokedFirstArgument(consumers.reexporter, "local")).toEqual([
      "invoke:same-stack:untracked"
    ]);
  });

  test("an open sibling re-export does not reopen a proven claim", () => {
    const artifactCase = consumers.mixed.entrypoints["."].cases[0];
    assert.ok(artifactCase.exports.opaque, "the open re-export must still be published");
    expect(invokedFirstArgument(consumers.mixed, "opaque")).toEqual([]);
    expect(invokedFirstArgument(consumers.mixed, "clean")).toEqual([
      "invoke:same-stack:untracked"
    ]);
  });

  test("a re-exported name publishes the dependency's closure, not a silence", () => {
    // The generator's own walks are all silent about `clean` here: there is no
    // local symbol for them to reach, so `creates_walk_clean`,
    // `returns_walk_clean` and `direct_callback_parameters` are the fail-closed
    // defaults. Running the local proposal filters against that silence used to
    // discard the dependency's certified closure for every domain
    // (`phase21/2026-09-15-closure-gap-plan.md` § 1). The inherited premise
    // replaces those filters; what it must publish is exactly the dependency's
    // answer, name for name.
    //
    // Compared against the dependency's own answer rather than a literal, so
    // the pin survives a census gaining a domain. The non-vacuity assertion
    // below is what makes that comparison mean anything: two empty lists agree
    // just as well as two full ones.
    const stated = closure(dependency, "clean");
    expect(stated.proposed).not.toEqual([]);
    expect(closure(consumers.reexporter, "clean")).toEqual(stated);
    expect(closure(consumers.mixed, "clean")).toEqual(stated);
  });

  test("an open re-export inherits no closure", () => {
    // `opaque`'s value comes from an unresolved global, so its dependency
    // contract closes nothing. An inherited premise closes exactly what the
    // dependency closed, so this is the falsifier for the test above: a closure
    // appearing here would be one no contract states.
    expect(closure(consumers.mixed, "opaque")).toEqual({ closed: [], proposed: [] });
    expect(closure(dependency, "opaque")).toEqual({ closed: [], proposed: [] });
  });

  test("the same obligation still reaches this package's own exports", () => {
    // Not a happy outcome, and deliberately not fixed here: the obligation is
    // raised about `opaque` and encloses no function, so the ladder's widest
    // mechanism marks `local` unknown too. Narrowing that is a separate change
    // to the attribution ladder itself; pin today's answer so it cannot move
    // silently.
    expect(invokedFirstArgument(consumers.mixed, "local")).toEqual([]);
  });
});
