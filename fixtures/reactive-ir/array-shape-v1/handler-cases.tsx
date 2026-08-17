// `v1/no-array-handlers` (SC8007) decided by the checker's array/tuple
// classification (`arrayShape`) instead of by matching rendered type text.
//
// Every case here type-checks. Verified with
// `node scripts/tsc-oracle.mjs check --dialect v1` against the real
// solid-js@1.9.14 typings, in both the strict and non-strict passes: the aliased
// tuple, the bare tuple, the readonly tuple, the inline literal, and a
// call-returned tuple are all **silent**, because `onXxx` is typed
// `EventHandlerUnion = EHandler | BoundEventHandler`, and `BoundEventHandler` is
// an interface with members `0` and `1` — which a tuple satisfies. So each
// finding below is the checker's own claim, that the value is a bound-handler
// tuple where a function was meant, and not a restatement of a type error.
//
// Only *tuples* live here. A plain array (`X[]`, `ReadonlyArray<X>`, `any[]`)
// has no `0`/`1` members, so it is already TS2322 on `onXxx`; those cases are in
// `clean-cases.tsx` with the duplicate they still produce recorded in
// docs/precision-backlog.md.
//
// Nothing here reads a signal or a component prop: this fixture reports SC8007
// and SC8012 only, so an incidental reactivity finding would blur the claim.

// The reason the fact exists. An alias renders as its own name, so the old text
// screen tested `Handlers` against `[`, `Array<`, `ReadonlyArray<`, `readonly `,
// and a trailing `[]`, matched none, and stayed silent on a real defect.
type Handlers = [(data: number, event: MouseEvent) => void, number];
const aliased: Handlers = [(data, event) => console.log(data, event), 1];

// Aliased twice, to show the classification is not a one-level unwrap.
type NestedHandlers = Handlers;
const nested: NestedHandlers = [(data, event) => console.log(data, event), 2];

declare function makeHandlers(): Handlers;

// A readonly tuple is still a tuple to the compiler, and `EventHandlerUnion`
// accepts it — so this is the rule's to report and not a `readonly ` text match.
const frozen: readonly [(data: number, event: MouseEvent) => void, number] = [
  (data, event) => console.log(data, event),
  3,
];

export function ArrayHandlers() {
  return (
    <div>
      {/* Positives: proven tuple, and legal per solid-js's own types. */}
      <button onClick={aliased} />
      <button onClick={nested} />
      <button onClick={makeHandlers()} />
      <button onClick={frozen} />
      <button onClick={[(data: number, event: MouseEvent) => data, 4]} />
    </div>
  );
}
