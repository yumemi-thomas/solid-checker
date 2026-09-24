import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "vitest";

import { addressing, annotate, compare, recipeSubjects } from "./probe-recipe-addressing.mjs";

const CORPUS = join(import.meta.dirname, "ecosystem-benchmark/probe-recipes");

function inputs(overrides = {}) {
  return {
    recipes: [
      { module: "a.mjs", claimId: "claim:live", subjects: new Set(["@scope/utils"]) },
      { module: "b.mjs", claimId: "claim:dead", subjects: new Set(["@scope/utils"]) },
      { module: "c.mjs", claimId: "claim:elsewhere", subjects: new Set(["other"]) }
    ],
    proposed: new Map([
      ["claim:live", { entrypoint: ".", export: "access", domain: "reads" }],
      ["claim:open", { entrypoint: ".", export: "noop", domain: "reads" }],
      ["claim:wild", { entrypoint: "./src/dom.ts", export: "contains", domain: "reads" }]
    ]),
    withheld: [],
    certified: new Set(["@scope/utils"]),
    sites: new Map([
      ["@scope/utils::noop", 98],
      ["@scope/utils::contains", 24]
    ]),
    ...overrides
  };
}

test("a recipe whose claim the run no longer proposes is stale, not merely unused", () => {
  const result = addressing(inputs());
  assert.equal(result.totals.recipes, 3);
  assert.equal(result.totals.addressed, 1);
  assert.equal(result.totals.stale, 1);
});

test("a recipe whose id moved is still addressed when the run states its recipe address", () => {
  // ADR 0117: a dependency's certified contract changed, so the claim id did,
  // and the loader binds the entry by its byte address instead.
  const result = addressing(
    inputs({
      recipes: [
        { module: "a.mjs", claimId: "claim:live", subjects: new Set(["@scope/utils"]) },
        {
          module: "b.mjs",
          claimId: "claim:dead",
          recipeAddress: "address:b",
          subjects: new Set(["@scope/utils"])
        },
        {
          module: "d.mjs",
          claimId: "claim:gone",
          recipeAddress: "address:nobody",
          subjects: new Set(["@scope/utils"])
        }
      ],
      addresses: new Map([["address:b", "claim:rekeyed"]])
    })
  );
  assert.equal(result.totals.addressed, 2);
  assert.equal(result.totals.addressedByAddress, 1);
  // An address the run never stated addresses nothing, exactly like an id.
  assert.equal(result.totals.stale, 1);
});

test("annotating writes each address after its claim id and refuses a contradiction", () => {
  const manifest = {
    format: "f",
    recipes: [
      { claimId: "claim:a", module: "a.mjs", importKind: "esm" },
      { claimId: "claim:b", module: "b.mjs", importKind: "esm" },
      { claimId: "claim:c", recipeAddress: "address:c", module: "c.mjs", importKind: "esm" }
    ]
  };
  const { manifest: written, written: count, missing, conflicts } = annotate(
    manifest,
    new Map([
      ["claim:a", "address:a"],
      ["claim:c", "address:c"]
    ])
  );
  assert.equal(count, 1);
  assert.deepEqual(Object.keys(written.recipes[0]), ["claimId", "recipeAddress", "module", "importKind"]);
  assert.deepEqual(missing, ["b.mjs"]);
  assert.deepEqual(conflicts, []);
  assert.deepEqual(
    annotate(manifest, new Map([["claim:c", "address:other"]])).conflicts,
    [{ module: "c.mjs", corpus: "address:c", run: "address:other" }]
  );
});

test("a recipe for a package the run never certified is out of scope, never stale", () => {
  const result = addressing(inputs());
  const elsewhere = result.families.find(family => family.subject === "other");
  assert.equal(elsewhere.inScope, false);
  assert.equal(elsewhere.stale, 0);
  assert.equal(result.totals.outOfScope, 1);
});

test("an unserved claim is priced only where a consumer can name the entrypoint", () => {
  const result = addressing(
    inputs({
      withheld: [
        { package: "@scope/utils", export: "noop", domain: "reads", claimId: "claim:open" },
        { package: "@scope/utils", export: "contains", domain: "reads", claimId: "claim:wild" }
      ]
    })
  );
  // `claim:wild` sits at `./src/dom.ts`, which a `./*` wildcard reaches and no
  // import can name, so its 24 sites are not this corpus's to answer.
  assert.deepEqual(result.unserved, [
    { export: "@scope/utils::noop", sites: 98, domains: ["reads"] }
  ]);
  assert.equal(result.totals.unservedSites, 98);
  assert.equal(result.totals.unplaceableClaims, 0);
});

test("a withheld claim no retained proposal places is counted apart, never priced", () => {
  const result = addressing(
    inputs({
      withheld: [
        { package: "@scope/utils", export: "noop", domain: "reads", claimId: "claim:absent" }
      ]
    })
  );
  assert.equal(result.totals.unservedClaims, 1);
  assert.equal(result.totals.unplaceableClaims, 1);
  assert.equal(result.totals.unservedSites, 0);
  assert.deepEqual(result.unserved, []);
});

test("addressing fewer claims than the pin is a regression; addressing more is not", () => {
  const pinned = { totals: { addressed: 20, stale: 4, unservedSites: 500 } };
  assert.deepEqual(compare(pinned, { totals: { addressed: 20, stale: 4, unservedSites: 500 } }), []);
  assert.deepEqual(
    compare(pinned, { totals: { addressed: 25, stale: 0, unservedSites: 400 } }),
    []
  );
  assert.deepEqual(compare(pinned, { totals: { addressed: 19, stale: 4, unservedSites: 500 } }), [
    "recipes addressing a claim: 20 -> 19"
  ]);
  assert.deepEqual(compare(pinned, { totals: { addressed: 20, stale: 4, unservedSites: 501 } }), [
    "consumer sites left open: 500 -> 501"
  ]);
});

test("a recipe's subject is the package it imports, not its filename", () => {
  assert.deepEqual(
    [...recipeSubjects('import { noop } from "@solid-primitives/utils";')],
    ["@solid-primitives/utils"]
  );
  // A subpath import still names the package, and the harness's own relative
  // and builtin imports are never a subject.
  assert.deepEqual(
    [
      ...recipeSubjects(
        'import { x } from "@corvu/utils/reactivity";\n' +
          'import { createSignal } from "solid-js";\n' +
          'import { readFileSync } from "node:fs";\n' +
          'import { emit } from "./contract-probe-harness.mjs";\n'
      )
    ].sort(),
    ["@corvu/utils", "solid-js"]
  );
});

test("every checked-in recipe module names at least one package it probes", () => {
  const manifest = JSON.parse(readFileSync(join(CORPUS, "recipes.json"), "utf8"));
  const anonymous = manifest.recipes.filter(
    entry => recipeSubjects(readFileSync(join(CORPUS, entry.module), "utf8")).size === 0
  );
  // Attribution is what separates a stale recipe from an out-of-scope one, so a
  // module this cannot attribute would silently leave the stale count short.
  assert.deepEqual(anonymous.map(entry => entry.module), []);
});
