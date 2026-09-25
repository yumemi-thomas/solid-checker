// The `@solidjs/signals` release every Solid 2 probe is measured with, pinned
// explicitly instead of left to `solid-js`'s caret range.
//
// No probe spec names `@solidjs/signals` unless the manifest row declares it:
// `solid-js@2.0.0-rc.3` depends on `@solidjs/signals ^2.0.0-rc.3`, so what a
// probe installed was whatever the registry's newest release was on the day its
// lockfile was first written, and that lockfile lives only in the gitignored
// cache under rust/target/install-locks. A cold cache, or a new spec set, would
// have resolved today's newest signals -- a release no audit has read -- while
// the report still named the same `solid` tuple. The environment a contract is
// certified in is part of what was proven (bundles are keyed by it), so it has to
// be a fact of the manifest, not of the cache.
//
// The pins below are the versions and integrities the cached locks resolved on
// 2026-09-26, so pinning moves no probe: 111 cached locks resolve rc.0 and 134
// resolve rc.6, and every one of them names the integrity recorded here.
// `install.test.mjs` fails if the cache and these pins ever disagree.

// sha512 integrities, by version.
//
// - rc.3 and rc.6 are ADR 0007's audited archives
//   (rust/crates/solid-dialect/src/solid_2.rs `AUDITED_ARCHIVES`, mirrored in
//   rust/crates/solid-dialect/audited-archives.json); rc.6's `package.json` is
//   checked in at benchmarks/package-contract-v2/phase0/rc6/solidjs-signals.
// - rc.0 has no audited archive. Its integrity is the one all 111 cached rc.0
//   locks record and the one every rc.0 environment of the shipped tier
//   (pkg/contracts/accepted/index.json) states. It is an install pin, never an
//   authority: nothing is denied on rc.0's bytes.
export const SOLID_SIGNALS_RELEASES = Object.freeze({
  "2.0.0-rc.0":
    "sha512-oKZSfvsCcKw1uJjOGbUkJ+OqlhXLHtZ+rShSyu9KH0lUH7UUwfMfsKeh81JPiQxDDg4YLhEwI38hg0JkwzTdvA==",
  "2.0.0-rc.3":
    "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==",
  "2.0.0-rc.6":
    "sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw=="
});

// The floor tuple is rc.0 of all three runtime packages: `solid-js@2.0.0-rc.0`
// was measured with the signals release it shipped beside.
export const SIGNALS_FLOOR = "2.0.0-rc.0";

// Every other Solid 2 environment in the corpus -- the rc.3 head, the two rc.2
// floors, the beta.19 `only` rows, and the rows that install no `solid-js` of
// their own -- was measured with rc.6, the prerelease the ecosystem installs and
// the one ADR 0007's re-audit read.
export const SIGNALS_HEAD = "2.0.0-rc.6";

/**
 * The signals release a corpus probe installs.
 *
 * Keyed by the probe's `solid-js` release, not by its kind, because `solid-js`
 * is what declares the range the pin has to sit inside: the two
 * `@tanstack/solid-router*` floors install `solid-js@2.0.0-rc.2`, whose
 * `^2.0.0-rc.2` excludes rc.0, and their cached locks resolve rc.6. A row that
 * names `@solidjs/signals` itself is installed at exactly that version.
 *
 * Returns `null` for a version this table has no integrity for, which the
 * caller turns into a refusal rather than an unpinned install.
 */
export function corpusSignalsPin(probe, completion = {}) {
  const solid = { ...(probe?.solid ?? {}), ...completion };
  const explicit = solid["@solidjs/signals"];
  const version = typeof explicit === "string"
    ? explicit
    : solid["solid-js"] === SIGNALS_FLOOR
      ? SIGNALS_FLOOR
      : SIGNALS_HEAD;
  const integrity = SOLID_SIGNALS_RELEASES[version] ?? null;
  return integrity ? { version, integrity } : null;
}
