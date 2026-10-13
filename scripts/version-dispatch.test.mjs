// "Adding a `Version` variant is a compile error at every site that must be
// revisited" is a design property this repository states in two places
// (`docs/adding-a-dialect.md`, `effect_api.rs`) and could not check.
//
// Rust enforces half of it: an exhaustive `match` stops compiling when the enum
// grows. What Rust cannot stop is the repair — adding `_ => {}` to make the
// build green is a one-character edit, and it turns a site that was demanding a
// decision into one that silently answers 2.0's behaviour for a dialect nobody
// wrote. Asserting a compile *failure* would want a `trybuild`-style harness
// this workspace does not carry; this checks the same property from the source,
// and catches the wildcard repair as well, which trybuild would not.
//
// Two claims, both keyed on the enum itself so a new variant fails this test
// until each site has been visited:
//
//   1. every dispatch site names every variant;
//   2. neither dispatching `match` block has a catch-all arm.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const VERSION_ENUM = "rust/crates/solid-dialect/src/lib.rs";

const read = (path) => readFileSync(join(ROOT, path), "utf8");

/**
 * The `Version` variants, read from the enum rather than listed here.
 *
 * Listing them would defeat the test: the failure it exists to catch is a
 * variant being added without every site being visited, and a hand-written
 * list is one more site that would not have been.
 */
function versionVariants() {
  const source = read(VERSION_ENUM);
  const at = source.indexOf("pub enum Version {");
  assert.notEqual(at, -1, `${VERSION_ENUM} no longer declares 'pub enum Version'`);
  const body = source.slice(at, source.indexOf("\n}", at));
  const variants = [...body.matchAll(/^\s{4}(V\d+),$/gm)].map((match) => match[1]);
  assert.ok(
    variants.length >= 2,
    `parsed ${variants.length} Version variant(s); the parse is wrong, not the enum`
  );
  return variants;
}

/**
 * Every site that must answer for each `Version`, and why it is one.
 *
 * A site belongs here when it maps a version to behaviour. Files that merely
 * *mention* a variant do not — `solid_2.rs` names its own `V2` and nothing
 * else, and `diagnostics.rs` names `V1` in a comment.
 */
const DISPATCH_SITES = [
  {
    path: VERSION_ENUM,
    why: "`Version::dialect` maps a version to the vocabulary this build carries for it",
  },
  {
    path: "rust/crates/solid-reactive-ir/src/effect_api.rs",
    why: "the effect-call seam decides where a dialect puts `createEffect`'s apply slot, and fails closed for one it does not model",
  },
];

/** The `{ … }` block a `match` opens at `marker`, by brace depth. */
function matchBlock(source, marker) {
  const at = source.indexOf(marker);
  assert.notEqual(at, -1, `the match on '${marker}' is gone; update this test with it`);
  const open = source.indexOf("{", at + marker.length - 1);
  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    else if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open, i + 1);
    }
  }
  throw new Error(`unbalanced braces after '${marker}'`);
}

test("every Version dispatch site names every variant", () => {
  const variants = versionVariants();
  for (const { path, why } of DISPATCH_SITES) {
    const source = read(path);
    for (const variant of variants) {
      assert.ok(
        source.includes(`Version::${variant}`) || source.includes(`Self::${variant}`),
        `${path} does not name Version::${variant}, and it must: ${why}`
      );
    }
  }
});

// The arms may be `{}` or a fail-closed return -- what they may not be is a
// wildcard, because that is the shape that makes a new variant compile without
// anyone deciding what it means.
test("neither dispatching match absorbs a new variant with a catch-all", () => {
  for (const [path, marker] of [
    [VERSION_ENUM, "pub fn dialect(self) -> Option<&'static dyn Dialect> {"],
    [
      "rust/crates/solid-reactive-ir/src/effect_api.rs",
      "match lookup.dialect.version() {",
    ],
  ]) {
    const block = matchBlock(read(path), marker);
    assert.doesNotMatch(
      block,
      /^\s*(_|_\s*=>|.*\b_\s*=>)/m,
      `${path}'s version dispatch has a catch-all arm; a new Version variant would compile there without a decision`
    );
  }
});

// The guard above is worth nothing if `matchBlock` silently returns something
// that is not the match, so pin what it extracted.
test("the extracted blocks really are the version dispatches", () => {
  const dialect = matchBlock(read(VERSION_ENUM), "pub fn dialect(self) -> Option<&'static dyn Dialect> {");
  assert.match(dialect, /Self::V1 => None/);
  assert.match(dialect, /Self::V2 => Some\(&Solid2\)/);

  const effect = matchBlock(
    read("rust/crates/solid-reactive-ir/src/effect_api.rs"),
    "match lookup.dialect.version() {"
  );
  assert.match(effect, /Version::V2 => \{\}/);
  assert.match(effect, /Version::V1 =>/);
});
