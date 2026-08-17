// Absence, `mixed`, and `unknown` are all "not proven", never "not an array".
// Every case here must report nothing, and each one is a deliberate false
// negative rather than a claim: the checker declines to speak where the type
// does not settle the question.

// An unconstrained type parameter stands for whatever a caller substitutes.
// `arrayShape` answers `unknown` for it — proving the predicate over every
// substitution through a constraint is a separate proof the fact does not
// attempt — so the rule stays silent even though the caller may well pass a
// tuple.
export function Generic<T>(handlers: T) {
  return <button onClick={handlers as never} />;
}

// A constrained type parameter is the same answer today: `unknown`. This is the
// nearest reachable improvement if the fact ever grows constraint resolution.
export function Constrained<T extends [(n: number) => void, number]>(pair: T) {
  return <button onClick={pair} />;
}

// A union that genuinely mixes the two shapes classifies as `mixed`: proven, but
// proving neither side. `notArray` is the only verdict that licenses the
// negative, so this falls through to the syntactic path, where a bare identifier
// is not an array literal.
type Handlers = [(n: number) => void, number];
declare const maybe: Handlers | undefined;

// `any` erases the question entirely.
declare const untyped: any;

// An unresolved import: the symbol never resolves, so no type and no fact.
// "Not found" is not evidence of a non-array — nor of an array.
// @ts-expect-error the module does not exist, which is the point of the case
import { external } from "./missing-module";

export function Unproven() {
  return (
    <div>
      <button onClick={maybe} />
      <button onClick={untyped} />
      <button onClick={external} />
    </div>
  );
}
