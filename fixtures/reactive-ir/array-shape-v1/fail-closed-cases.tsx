// Absence, `mixed`, and `unknown` are all "not proven", never "not an array".
// Type-correct cases report `uncertifiable`; only the unresolved import stays
// silent because that diagnostic belongs to TypeScript.

// An unconstrained type parameter stands for whatever a caller substitutes.
// `arrayShape` answers `unknown` for it — proving the predicate over every
// substitution through a constraint is a separate proof the fact does not
// attempt — so the rule retains an explicit uncertainty.
export function Generic<T>(handlers: T) {
  return <button onClick={handlers as never} />;
}

// A constrained type parameter is the same answer today: `unknown`. This is the
// nearest reachable improvement if the fact ever grows constraint resolution.
export function Constrained<T extends [(n: number) => void, number]>(pair: T) {
  return <button onClick={pair} />;
}

// A union used to sit here too. It no longer does: `tupleShape` reports the meet
// of a union's constituents, so `Handlers | undefined` is now proven a bound pair
// and is a positive in `handler-cases.tsx`. The union that still proves nothing —
// a pair or a plain function — is in `uncertain-cases.tsx`.

// `any` erases the question entirely, which is an uncertainty rather than a
// safety proof.
declare const untyped: any;

// An unresolved import: the symbol never resolves, so no type and no fact.
// "Not found" is not evidence of a non-array — nor of an array.
// @ts-expect-error the module does not exist, which is the point of the case
import { external } from "./missing-module";

export function Unproven() {
  return (
    <div>
      <button onClick={untyped} />
      <button onClick={external} />
    </div>
  );
}
