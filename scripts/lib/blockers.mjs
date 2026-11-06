/**
 * A blocker is kebab-case words and an optional `:detail`; a function's name says where, not what is missing (ADR 0680).
 * `proof-step-unnamed` is the last resort and no blocker: a row that lands there has a hole nobody named.
 */
export const isNamedBlocker = (context) => /^[a-z0-9]+(-[a-z0-9]+)+(:.*)?$/s.test(context ?? "") && !String(context).startsWith("proof-step-unnamed");
