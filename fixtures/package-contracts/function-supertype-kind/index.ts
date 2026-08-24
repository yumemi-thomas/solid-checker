// lib.es5.d.ts's signature-less `Function`-supertype family. Every type here
// is the *real* one -- `Function` and `Record` come from the default library
// and nothing in this fixture redeclares them, which matters because a stub
// that loosened any of these would invent the outcome instead of reproducing
// it.
//
// `Function` declares `apply`/`call`/`bind` and no call or construct
// signature, so `Callability::NonCallable` and
// `Constructability::NonConstructable` are both truthful about the declared
// type -- and every function value in JavaScript is assignable to it, so
// `typeof handler === "function"` can hold at runtime for every one of these
// bindings. `export_kind_proof` reads the pair as its full negative and the
// generator publishes `kind: "value"`, which is the maximal certified negative:
// "invokes no caller-supplied callback".
//
// **That claim is wrong and this fixture pins it wrong on purpose.** It is a
// pre-existing hole, not one the constructability fact opened: through
// callability alone these answered `NonCallable` with no class syntax to
// contradict it, and published `value` for exactly the same reason. Nothing on
// the consumer side can detect the family -- assignability to `Function` is not
// one of these facts. TypeScript-Go's own `typeof` narrowing
// (`checker.isFunctionObjectType`) gets it right through a `bind`-member
// subtype-of-`Function` fallback that `callabilityOfType` and
// `constructabilityOfType` do not carry.
//
// The flip that fixes this row is named in the producer's ADR 0020: give both
// signature walks that same `bind`-member fallback. When it lands, `raw` (and
// each of the three below it) answers `Unknown` rather than a closed negative,
// this entrypoint is *refused* rather than published, and this file's
// `expected.json` becomes the empty-entrypoints shape. See
// docs/precision-backlog.md.
declare const raw: Function;
// The rest of the family, which is the same hole reached without naming
// `Function` at all: a function value is assignable to each of these, and none
// of them declares a signature of its own.
declare const bag: object;
declare const empty: {};
declare const table: Record<string, unknown>;

// The control. A `number` is in no sense a function, answers the same closed
// pair, and `kind: "value"` is the correct claim for it -- which is why the
// pair cannot simply be distrusted wholesale.
declare const retries: number;

export { bag, empty, raw, retries, table };
