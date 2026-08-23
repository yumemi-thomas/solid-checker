// Ecosystem family catalog: which npm scopes/packages belong to which Solid
// ecosystem family, and whether a matching package is an official member of
// that family's org or merely a name that mentions it (a fork, a clone, an
// unrelated package that happens to share a word).
//
// This module is pure and offline: `classifyPackage` cannot call the
// registry, so it cannot literally check npm's `/-/org/<scope>/package`
// listing. It uses the npm scope itself as the listing's proxy — the "Real
// registry facts" audited for this benchmark confirm that an org's scope
// (`@kobalte/*`, `@corvu/*`, ...) IS that org's complete published set, so
// "under the scope" and "in the org listing" are the same fact here. A name
// that only *mentions* a family (matches a search term) without living under
// that scope is therefore never truly "in the org listing" and is classified
// `supplemental`, not `official` — this is how a fork like a community
// `kobalte-*` package is kept out of the official family totals without a
// network call.

// The Solid 1.x release this benchmark audits every non-runtime package
// against for the `solid1` target. Pinned, not "latest 1.x", so the floor of
// the ecosystem corpus never silently drifts when a new 1.x patch ships.
export const AUDITED_SOLID_1 = "1.9.14";

// The packages whose own published releases define what "Solid 2" even means
// for compatibility purposes. Order matters: it is the tie-break order used
// wherever a probe needs "the first runtime package present" (see
// `lib/select.mjs`'s `computeChannel`).
export const SOLID_RUNTIME_PACKAGES = ["solid-js", "@solidjs/web", "@solidjs/signals"];

// Ordered; every report groups and walks families in this order.
export const FAMILIES = [
  {
    id: "official-solid",
    label: "Official Solid",
    order: 0,
    scopes: ["solidjs"],
    packages: ["solid-js"],
    supplementalScopes: [],
    searchTerms: [],
    // Official packages are in-family by identity (scope or exact name), not
    // by declaring a dependency on solid-js — some 2.0 packages depend on
    // @solidjs/web or @solidjs/signals instead and must not be excluded for
    // "declaring the wrong Solid dependency".
    requireSolidDependency: false,
    minimumPackages: [
      "@solidjs/element",
      "@solidjs/h",
      "@solidjs/html",
      "@solidjs/image",
      "@solidjs/meta",
      "@solidjs/router",
      "@solidjs/signals",
      "@solidjs/start",
      "@solidjs/start-devtools",
      "@solidjs/testing-library",
      "@solidjs/universal",
      "@solidjs/vite-plugin",
      "@solidjs/vite-plugin-nitro-2",
      "@solidjs/web",
      "solid-js"
    ]
  },
  {
    id: "kobalte",
    label: "Kobalte",
    order: 1,
    scopes: ["kobalte"],
    packages: [],
    supplementalScopes: [],
    searchTerms: ["kobalte"],
    requireSolidDependency: false,
    minimumPackages: [
      "@kobalte/core",
      "@kobalte/solidbase",
      "@kobalte/tailwindcss",
      "@kobalte/themes",
      "@kobalte/utils",
      "@kobalte/vanilla-extract"
    ]
  },
  {
    id: "solid-primitives",
    label: "Solid Primitives",
    order: 2,
    scopes: ["solid-primitives"],
    packages: [],
    supplementalScopes: [],
    searchTerms: [],
    requireSolidDependency: false,
    // The task names no fixed @solid-primitives membership list, unlike
    // official-solid and kobalte; discovery owns enumerating this org.
    minimumPackages: []
  },
  {
    id: "corvu",
    label: "Corvu",
    order: 3,
    scopes: ["corvu", "corvu-next"],
    packages: ["corvu"],
    supplementalScopes: [],
    searchTerms: ["corvu"],
    requireSolidDependency: false,
    minimumPackages: ["corvu"]
  },
  {
    id: "tanstack",
    label: "TanStack",
    order: 4,
    scopes: ["tanstack"],
    packages: [],
    supplementalScopes: [],
    searchTerms: [],
    // The @tanstack scope ships React/Vue/Svelte/neutral packages alongside
    // Solid adapters. Without this flag the Solid benchmark would otherwise
    // pull in packages that have nothing to do with Solid at all; requiring a
    // declared range for a SOLID_RUNTIME_PACKAGE is the only signal that
    // distinguishes a Solid adapter from its React/Vue siblings.
    requireSolidDependency: true,
    minimumPackages: []
  },
  {
    id: "solid-devtools",
    label: "Solid Devtools",
    order: 5,
    scopes: ["solid-devtools"],
    packages: ["solid-devtools"],
    supplementalScopes: [],
    searchTerms: [],
    requireSolidDependency: false,
    minimumPackages: ["solid-devtools"]
  },
  {
    id: "solid-recharts",
    label: "Solid Recharts",
    order: 6,
    scopes: [],
    packages: ["solid-recharts"],
    supplementalScopes: [],
    searchTerms: [],
    requireSolidDependency: false,
    minimumPackages: ["solid-recharts"]
  },
  {
    id: "motion-solidjs",
    label: "Motion for Solid",
    order: 7,
    scopes: [],
    packages: ["motion-solidjs"],
    supplementalScopes: [],
    searchTerms: [],
    requireSolidDependency: false,
    minimumPackages: ["motion-solidjs"]
  }
];

export function familyById(id) {
  return FAMILIES.find(family => family.id === id);
}

export function familyOrder(id) {
  return familyById(id)?.order;
}

function scopeOf(name) {
  const match = /^@([^/]+)\//.exec(name);
  return match ? match[1] : null;
}

/**
 * Classifies a package name into a family and a status.
 *
 * Two full passes over FAMILIES, in report order: official membership (exact
 * standalone package name or npm scope) always wins over a supplemental
 * match, so a package that is genuinely under `@kobalte/*` is never demoted
 * to supplemental merely because its name also contains "kobalte" (it always
 * will). Only a name that fails every official check but still matches a
 * search term — the fork/clone case — falls through to the supplemental
 * pass.
 */
export function classifyPackage(name) {
  if (typeof name !== "string" || name.length === 0) return null;
  const scope = scopeOf(name);

  for (const family of FAMILIES) {
    if (family.packages.includes(name)) return { family: family.id, status: "official" };
    if (scope && family.scopes.includes(scope)) return { family: family.id, status: "official" };
  }

  const lowerName = name.toLowerCase();
  for (const family of FAMILIES) {
    if (scope && family.supplementalScopes.includes(scope)) return { family: family.id, status: "supplemental" };
    if (family.searchTerms.some(term => lowerName.includes(term.toLowerCase()))) {
      return { family: family.id, status: "supplemental" };
    }
  }

  return null;
}
