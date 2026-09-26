// Holds every fixture dialect stub to being present, parseable, tracked, and
// a major this build carries a dialect for.
//
// Dialect selection follows the nearest `node_modules/solid-js/package.json`
// above the project, so a stub decides which catalog a fixture is checked
// under. Four ways that goes wrong leave no other trace:
//
// - the stub is missing, or its directory is empty (git cannot record an empty
//   directory, so the stub never arrives) -- the fixture falls back silently
//   to the default dialect;
// - the stub has no `.gitignore` exception under the repository-wide
//   `**/node_modules/` rule, so it is present locally and absent in CI;
// - the version field is not a version -- same silent fallback;
// - the version names a major no carried dialect models. This one is newer
//   than the others and is not a fallback at all: `dialect.rs` refuses such a
//   project with `SC9013` (ADR 0110 § 1), so the fixture asserts a refusal
//   rather than whatever it was written to assert, and its snapshot records
//   that as if intended.
//
// `eslint-plugin-corpus-v1` shipped the first shape and `solid-reexport` the
// second, so this is a check, not a hypothetical.
//
// This lives apart from `coverage.mjs` so it can be tested against a throwaway
// tree: a gate whose failure path nothing exercises is a gate nobody knows the
// shape of.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";

import { loadDialectManifests } from "../dialect-manifests.mjs";

/** The fixture groups whose projects select a dialect from a stub. */
export const STUB_GROUPS = [
  "reactive-ir",
  "engine",
  "package-contracts",
  "ownership-cases",
  "partial-audit",
];

/**
 * The majors this build carries a dialect for, read from the assembly
 * manifests rather than written down here -- `solid-v2` is major 2, and
 * `loadDialectManifests` already refuses an id that does not match its
 * directory.
 */
export const carriedSolidMajors = (projectRoot) =>
  new Set(
    loadDialectManifests({ projectRoot }).map((manifest) =>
      Number(manifest.id.slice("solid-v".length))
    )
  );

/**
 * The major a `solid-js` version string names, or `null` when the string is
 * not a version.
 *
 * The same parse `Version::for_solid_js` performs: leading range characters
 * stripped, the first component taken, prerelease and build metadata ignored.
 */
export const solidMajor = (version) => {
  if (typeof version !== "string") return null;
  const head = version.trim().replace(/^[\^~=v><\s]+/, "").split(/[.\-+]/)[0];
  if (!/^\d+$/.test(head)) return null;
  return Number(head);
};

/**
 * Every `node_modules/solid-js` stub at any depth below `directory`, skipping
 * the inside of `node_modules` trees themselves. A fixture may hold a nested
 * package (`closed-domain-probe-gate/primitive-consumer/`) with a stub of its
 * own, and a stub one level down is exactly as load-bearing for dialect
 * selection -- and exactly as silently absent in CI without its `.gitignore`
 * exception -- as one at the fixture root.
 */
function* stubDirectories(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const path = join(directory, entry.name);
    if (entry.name === "node_modules") {
      const stub = join(path, "solid-js");
      if (existsSync(stub)) yield { stub, fixture: directory };
      continue;
    }
    yield* stubDirectories(path);
  }
}

/**
 * The Solid 2 packages beside a `solid-js` stub whose releases the installation
 * review also reads (`resolved_releases` in dialect.rs): `@solidjs/signals`
 * decides the store typing, `until` and `omit`'s predicate form, and
 * `@solidjs/web` decides `dynamic`'s options. A stub of either that is present
 * locally and absent in CI moves those answers only in CI, exactly as an
 * untracked `solid-js` stub moves the dialect, so it is held to the same
 * presence, parse and tracking checks. Absence is legitimate here -- a missing
 * `@solidjs/signals` is a stated gap the notice reports, and a missing web is
 * never asked about -- so only a directory that exists is checked.
 */
export const COMPANION_PACKAGES = ["@solidjs/signals", "@solidjs/web"];

function companionStubProblems(projectRoot, solidStub, fixture, tracked) {
  const problems = [];
  const nodeModules = join(solidStub, "..");
  for (const name of COMPANION_PACKAGES) {
    const directory = join(nodeModules, ...name.split("/"));
    if (!existsSync(directory)) continue;
    const manifest = join(directory, "package.json");
    const id = relative(projectRoot, manifest);
    if (!existsSync(manifest)) {
      problems.push(`${id}: missing -- the installation review reads ${name} as unresolved`);
      continue;
    }
    let version;
    try {
      version = JSON.parse(readFileSync(manifest, "utf8")).version;
    } catch (error) {
      problems.push(`${id}: unparseable (${error.message})`);
      continue;
    }
    if (typeof version !== "string" || version === "") {
      problems.push(`${id}: no "version" -- the installation review reads ${name} as unresolved`);
    }
    if (!tracked.has(id)) {
      problems.push(
        `${id}: not tracked by git -- add '!${relative(projectRoot, fixture)}/node_modules/'` +
          ` and its '/**' twin to .gitignore, or the stub is absent in CI`
      );
    }
  }
  return problems;
}

const trackedUnderFixtures = (projectRoot) =>
  new Set(
    execFileSync("git", ["ls-files", "-z", "fixtures"], {
      cwd: projectRoot,
      encoding: "utf8",
    })
      .split("\0")
      .filter(Boolean)
  );

/**
 * Every reason a fixture's dialect stub cannot be trusted, as printable lines.
 * Empty means every stub decides the catalog its fixture was written for.
 *
 * `tracked` and `majors` are injectable so a test can supply a tree that is not
 * this repository; both default to reading this one.
 */
export function dialectStubProblems({
  projectRoot,
  groups = STUB_GROUPS,
  tracked = trackedUnderFixtures(projectRoot),
  majors = carriedSolidMajors(projectRoot),
} = {}) {
  const problems = [];
  for (const group of groups) {
    const base = join(projectRoot, "fixtures", group);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      for (const { stub: stubDirectory, fixture } of stubDirectories(join(base, entry.name))) {
        const manifest = join(stubDirectory, "package.json");
        const id = relative(projectRoot, manifest);
        if (!existsSync(manifest)) {
          problems.push(`${id}: missing -- the fixture falls back to the default dialect`);
          continue;
        }
        let version;
        try {
          version = JSON.parse(readFileSync(manifest, "utf8")).version;
        } catch (error) {
          problems.push(`${id}: unparseable (${error.message})`);
          continue;
        }
        if (typeof version !== "string" || version === "") {
          problems.push(`${id}: no "version" -- dialect selection cannot resolve it`);
        } else {
          const major = solidMajor(version);
          if (major === null) {
            problems.push(
              `${id}: version ${JSON.stringify(version)} is not a version --` +
                ` the fixture falls back to the default dialect instead of selecting one`
            );
          } else if (!majors.has(major)) {
            problems.push(
              `${id}: solid-js ${version} names major ${major}, which no carried dialect` +
                ` models -- the checker refuses such a project with SC9013 rather than` +
                ` analyzing it, so this fixture asserts a refusal, not its own subject`
            );
          }
        }
        if (!tracked.has(id)) {
          problems.push(
            `${id}: not tracked by git -- add '!${relative(projectRoot, fixture)}/node_modules/'` +
              ` and its '/**' twin to .gitignore, or the stub is absent in CI`
          );
        }
        problems.push(...companionStubProblems(projectRoot, stubDirectory, fixture, tracked));
      }
    }
  }
  return problems;
}
