// A mapped-type property access resolves to a synthesized symbol: it has no
// declaration of its own, so the producer cannot mint a declaration-hashed
// identity for it and falls back to a generation-scoped counter. That makes this
// file non-durable, and non-durable files recompute on every generation.
//
// Mapped types are ordinary TypeScript, so this is the common case, not a corner.

type Fields = { [K in "first" | "second"]: number };

declare const fields: Fields;

export const readFirst = fields.first;
export const readSecond = fields.second;
