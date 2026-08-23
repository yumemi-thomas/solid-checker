// The oracle case runner's one piece of provisioning: the `node_modules` link
// each dialect base points at the audited install through.
//
// It is worth its own test because the failure it can have is silent. The
// checker picks its dialect from the nearest `node_modules/solid-js` above the
// project, and the oracle compiles against the same tree -- so a link that
// points somewhere unexpected, or nowhere, changes which catalog runs and which
// typings answer "does TypeScript already report this?". Nothing downstream
// says "the link was wrong"; it says a case's verdict moved.
//
// The cases here use throwaway directories under $TMPDIR: the assertions need
// to create a *dangling* link and a *mispointed* one, which must never be done
// to `rust/target`.
import assert from "node:assert/strict";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { ensureDirectoryLink } from "./lib/tsc-oracle-case.mjs";

/** A throwaway root holding two candidate install trees and a base directory. */
const withBase = (body) => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-oracle-case-test-"));
  try {
    const audited = join(root, "audited", "node_modules");
    const other = join(root, "other", "node_modules");
    for (const tree of [audited, other]) {
      mkdirSync(join(tree, "solid-js"), { recursive: true });
      writeFileSync(join(tree, "solid-js", "package.json"), '{"version":"1.9.14"}\n');
    }
    const base = join(root, "base");
    mkdirSync(base, { recursive: true });
    return body({ root, base, link: join(base, "node_modules"), audited, other });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test("a missing link is created, pointing where it was asked to", () => {
  withBase(({ link, audited }) => {
    assert.equal(ensureDirectoryLink(link, audited), link);
    assert.equal(lstatSync(link).isSymbolicLink(), true);
    assert.equal(readlinkSync(link), audited);
    // Idempotent: a second call on a correct link changes nothing.
    ensureDirectoryLink(link, audited);
    assert.equal(readlinkSync(link), audited);
  });
});

test("a dangling link is not mistaken for someone else's identical link", () => {
  // The defect: `existsSync` *follows* a symlink, so a dangling link makes an
  // `existsSync(link)` guard false; `symlinkSync` then throws `EEXIST`, and
  // swallowing `EEXIST` as "lost a harmless race" leaves the base pointing at
  // nothing. Reachable by moving the checkout -- the recorded target is
  // absolute, `rust/target` travels with the move, and every version check
  // still passes against the new path.
  withBase(({ root, link, audited }) => {
    const gone = join(root, "GONE", "node_modules");
    symlinkSync(gone, link, "dir");
    assert.equal(existsSync(link), false, "the fixture must really be a dangling link");

    // Re-pointed at the tree that does exist, rather than left dangling.
    ensureDirectoryLink(link, audited);
    assert.equal(readlinkSync(link), audited);
    assert.equal(existsSync(link), true);
  });
});

test("a link pointing at the wrong tree is replaced, never accepted", () => {
  // Silently accepting this is what decides that a case compiles against, and
  // is analyzed under, a dialect nobody asked for.
  withBase(({ link, audited, other }) => {
    symlinkSync(other, link, "dir");
    ensureDirectoryLink(link, audited);
    assert.equal(readlinkSync(link), audited);
  });
});

test("a link whose target does not exist at all is a loud failure", () => {
  withBase(({ root, base }) => {
    const missing = join(root, "never-installed", "node_modules");
    assert.throws(
      () => ensureDirectoryLink(join(base, "node_modules"), missing),
      /dangling link|provision/,
      "an absent audited install must announce itself, not be linked to quietly",
    );
  });
});

test("a real directory where the link belongs is a loud failure", () => {
  // `rmSync`-ing it would delete whatever is inside; guessing that it is
  // equivalent to the link would be worse. Refuse and say what to do.
  withBase(({ link, audited }) => {
    mkdirSync(join(link, "solid-js"), { recursive: true });
    assert.throws(() => ensureDirectoryLink(link, audited), /is not a symlink/);
  });
});

test("two callers racing on a cold base agree on the result", () => {
  // The tolerated race: the loser's `EEXIST` is fine *because* the link the
  // winner made is verified to be the same link, not because `EEXIST` is
  // assumed to mean that.
  withBase(({ link, audited }) => {
    ensureDirectoryLink(link, audited);
    ensureDirectoryLink(link, audited);
    ensureDirectoryLink(link, audited);
    assert.equal(readlinkSync(link), audited);
  });
});
