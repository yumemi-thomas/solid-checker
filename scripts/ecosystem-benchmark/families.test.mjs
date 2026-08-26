import { test } from "vitest";
import assert from "node:assert/strict";

import { AUDITED_SOLID_1, FAMILIES, SOLID_RUNTIME_PACKAGES, classifyPackage, familyById, familyOrder } from "./lib/families.mjs";

test("AUDITED_SOLID_1 and SOLID_RUNTIME_PACKAGES are the pinned constants", () => {
  assert.equal(AUDITED_SOLID_1, "1.9.14");
  assert.deepEqual(SOLID_RUNTIME_PACKAGES, ["solid-js", "@solidjs/web", "@solidjs/signals"]);
});

test("FAMILIES is ordered exactly as documented, report order follows the array", () => {
  assert.deepEqual(
    FAMILIES.map(family => family.id),
    ["official-solid", "kobalte", "solid-primitives", "corvu", "tanstack", "solid-devtools", "solid-recharts", "motion-solidjs"]
  );
  // `order` must agree with array position so a caller can sort by either.
  FAMILIES.forEach((family, index) => assert.equal(family.order, index));
});

test("familyById and familyOrder look up by id", () => {
  assert.equal(familyById("kobalte").label, "Kobalte");
  assert.equal(familyById("does-not-exist"), undefined);
  assert.equal(familyOrder("corvu"), familyById("corvu").order);
  assert.equal(familyOrder("does-not-exist"), undefined);
});

test("tanstack is the only family that requires a declared Solid dependency", () => {
  const flags = Object.fromEntries(FAMILIES.map(family => [family.id, family.requireSolidDependency]));
  assert.equal(flags.tanstack, true);
  for (const id of Object.keys(flags)) {
    if (id !== "tanstack") assert.equal(flags[id], false, `${id} should not require a declared Solid dependency`);
  }
});

test("classifyPackage: official-solid scope and standalone package", () => {
  assert.deepEqual(classifyPackage("solid-js"), { family: "official-solid", status: "official" });
  assert.deepEqual(classifyPackage("@solidjs/router"), { family: "official-solid", status: "official" });
  assert.deepEqual(classifyPackage("@solidjs/anything-not-enumerated"), { family: "official-solid", status: "official" });
});

test("classifyPackage: kobalte org scope is official", () => {
  assert.deepEqual(classifyPackage("@kobalte/core"), { family: "kobalte", status: "official" });
  assert.deepEqual(classifyPackage("@kobalte/utils"), { family: "kobalte", status: "official" });
});

test("classifyPackage: a package that mentions kobalte but is not under the org scope is supplemental", () => {
  // This is the fork-detection case: a package matching the search term
  // "kobalte" that is NOT published under @kobalte/* is never counted in the
  // official family totals, only tracked as supplemental.
  assert.deepEqual(classifyPackage("kobalte-community-ui"), { family: "kobalte", status: "supplemental" });
  assert.deepEqual(classifyPackage("solid-kobalte-extras"), { family: "kobalte", status: "supplemental" });
});

test("classifyPackage: corvu has both a standalone package and two org scopes", () => {
  assert.deepEqual(classifyPackage("corvu"), { family: "corvu", status: "official" });
  assert.deepEqual(classifyPackage("@corvu/dialog"), { family: "corvu", status: "official" });
  assert.deepEqual(classifyPackage("@corvu-next/dialog"), { family: "corvu", status: "official" });
  assert.deepEqual(classifyPackage("corvu-fork-widgets"), { family: "corvu", status: "supplemental" });
});

test("classifyPackage: tanstack scope classifies React/Vue/Svelte adapters too (dependency filtering is select.mjs's job)", () => {
  assert.deepEqual(classifyPackage("@tanstack/react-query"), { family: "tanstack", status: "official" });
  assert.deepEqual(classifyPackage("@tanstack/solid-query"), { family: "tanstack", status: "official" });
});

test("classifyPackage: standalone single-package families", () => {
  assert.deepEqual(classifyPackage("solid-devtools"), { family: "solid-devtools", status: "official" });
  assert.deepEqual(classifyPackage("solid-recharts"), { family: "solid-recharts", status: "official" });
  assert.deepEqual(classifyPackage("motion-solidjs"), { family: "motion-solidjs", status: "official" });
});

test("classifyPackage: unrelated package names classify to nothing", () => {
  assert.equal(classifyPackage("react"), null);
  assert.equal(classifyPackage("left-pad"), null);
  assert.equal(classifyPackage(""), null);
  assert.equal(classifyPackage(undefined), null);
});

test("classifyPackage: official membership always wins over a coincidental search-term match", () => {
  // A package genuinely published under @kobalte/* that also happens to
  // contain another family's search term must still resolve to its real
  // scope-based family, not fall through to a supplemental match.
  assert.deepEqual(classifyPackage("@kobalte/core"), { family: "kobalte", status: "official" });
});

test("minimumPackages named in the task are present for official-solid and kobalte", () => {
  const officialSolid = familyById("official-solid");
  for (const name of ["@solidjs/meta", "@solidjs/router", "@solidjs/testing-library", "@solidjs/start", "@solidjs/signals", "@solidjs/universal", "@solidjs/web", "@solidjs/element", "@solidjs/h", "@solidjs/html", "@solidjs/vite-plugin-nitro-2", "@solidjs/image", "@solidjs/vite-plugin", "@solidjs/start-devtools"]) {
    assert.ok(officialSolid.minimumPackages.includes(name), `expected official-solid minimumPackages to include ${name}`);
  }
  const kobalte = familyById("kobalte");
  for (const name of ["@kobalte/utils", "@kobalte/core", "@kobalte/tailwindcss", "@kobalte/vanilla-extract", "@kobalte/solidbase", "@kobalte/themes"]) {
    assert.ok(kobalte.minimumPackages.includes(name), `expected kobalte minimumPackages to include ${name}`);
  }
});
