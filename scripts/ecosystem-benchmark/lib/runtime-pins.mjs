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
// The pins were first the versions and integrities the cached locks resolved on
// 2026-09-26, so pinning moved no probe: 111 cached locks resolved rc.0 and 134
// resolved rc.6. Since 2026-09-27 the head is rc.9 (see `SIGNALS_HEAD`), which is
// a deliberate re-pin: every non-floor probe moves to rc.9, and the cache
// re-resolves it on the first corpus run. `runtime-pins.test.mjs` fails if a
// cached lock written under a pin ever disagrees with it.

// sha512 integrities, by version.
//
// - rc.3, rc.6 and rc.9 are audited archives
//   (rust/crates/solid-dialect/src/solid_2.rs `AUDITED_ARCHIVES`, mirrored in
//   rust/crates/solid-dialect/audited-archives.json); rc.6's and rc.9's
//   `package.json` are checked in at
//   benchmarks/package-contract-v2/phase0/{rc6,rc9}/solidjs-signals. rc.6 stays
//   because the shipped tier's head environments were proven with it.
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
    "sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==",
  "2.0.0-rc.9":
    "sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw=="
});

// The floor tuple is rc.0 of all three runtime packages: `solid-js@2.0.0-rc.0`
// was measured with the signals release it shipped beside.
export const SIGNALS_FLOOR = "2.0.0-rc.0";

// Every other Solid 2 environment in the corpus -- the rc.9 head, the rc.6 and
// rc.8 floors, the beta.19 `only` rows, and the rows that install no `solid-js`
// of their own -- is measured with rc.9.
//
// It was rc.6 until 2026-09-27, while the audited Solid 2 ceiling was rc.3 and
// rc.9 was kept out of certification. With the ceiling at rc.9
// (`AUDITED_SOLID_2`) there is no choice left: `solid-js@2.0.0-rc.9` declares
// `@solidjs/signals ^2.0.0-rc.9`, and so does rc.8 (the tanstack router
// floors) with `^2.0.0-rc.8`, so rc.6 no longer sits inside the range the
// head's own `solid-js` declares. rc.9 is also what a fresh install of any
// 2.0 prerelease `solid-js` resolves today, and it is an audited archive, so
// its negative rows answer.
export const SIGNALS_HEAD = "2.0.0-rc.9";

/**
 * The signals release a corpus probe installs.
 *
 * Keyed by the probe's `solid-js` release, not by its kind, because `solid-js`
 * is what declares the range the pin has to sit inside: the two
 * `@tanstack/solid-router*` floors install `solid-js@2.0.0-rc.8`, whose
 * `^2.0.0-rc.8` excludes rc.0 and rc.6. A row that names `@solidjs/signals`
 * itself is installed at exactly that version.
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
