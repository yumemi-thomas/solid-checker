// The generator's member-alias proposal (ADR 0103, amended 2026-09-23): which
// exports it proposes `creates` and `callbacks` for, beside `reads`, from syntax
// alone. Whether the aliased member is one the certifier has reviewed is not
// decided here; its default-library alias census decides that.

// --- Proposed: a `const` of one identifier, initialized by a member access. ---

// Exported where it is declared.
export const direct = Object.keys;

// Declared once and exported by name, which is how a bundler writes it.
const viaSpecifier = Object.values;
export { viaSpecifier };

// A member of this package's own object. Proposed all the same, because syntax
// cannot tell it from a default-library member; the certifier refuses it, having
// no reviewed identity to close on.
const helpers = {
  run(value) {
    return value;
  }
};
export const ownMember = helpers.run;

// --- Not proposed. ---

// A `let` can be reassigned before anything reads it.
export let reassignable = Object.freeze;

// A computed member names nothing syntax can see. (Aimed at a local object:
// a computed member of `Object` itself could be `defineProperty`, which the
// closure scan flags as accessor installation and withdraws `reads` for the
// whole module over.)
const table = { run: helpers.run };
export const computed = table["run"];

// A call initializer hands back a new value, not the member.
export const bound = Object.keys.bind(Object);
