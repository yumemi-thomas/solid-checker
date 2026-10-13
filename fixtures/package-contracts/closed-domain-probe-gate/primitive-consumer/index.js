// The genuine positive the `creates` census has something to say about: a
// consumer export that calls a real Solid 2.0 primitive.
//
// `onSettled` is audited in `@solidjs/signals@2.0.0-rc.3` with `creates: []`
// closed, and the dialect's negative table carries that row (ADR 0007). Against
// the *audited archive* the census would disposition this call `dialect-axiom`
// and `runAfterSettle` would close `creates: []`. Against the stub beside this
// file — whose tuple is not the audited one — the tier refuses to answer, and
// inside the certifier's private project, which materializes the package
// snapshot alone, the import resolves to nothing at all: the census refuses the
// domain by name as an unresolved callee. Either way nothing certifies closed
// on a stub's word.
//
// It lives in its own package rather than beside `run` and `runCreatingOwner`
// because every probe recipe of this fixture imports the main package inside
// the private probe directory, where `solid-js` does not exist; a top-level
// import of it there would fail the module load of every recipe.
import { onSettled } from "solid-js";

export function runAfterSettle(callback) {
  onSettled(callback);
}
