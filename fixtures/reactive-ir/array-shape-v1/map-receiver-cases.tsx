// `v1/prefer-for` (SC8014) consults `arrayShape` only after its receiver is
// proven reactive. These declared receivers are static/unproven and therefore
// stay clean before the fix gate; the shapes remain here as negative controls.
//
// Rewriting an Immutable.js collection — or anything else that has `.map` but
// is not an array — would change behaviour, so future reactive variants must
// still require a proven array before offering a fix.
type Rows = string[];
type RowTuple = [string, string];

declare const aliasedArray: Rows;
declare const aliasedTuple: RowTuple;

// Has `.map`, is not an array. The classification is `notArray`, so no fix.
declare const collection: { map<U>(project: (row: string) => U): U[] };

// Unproven: an unconstrained type parameter is `unknown`, so no fix either — the
// same fail-closed answer as `notArray`, reached for a different reason.
declare function unproven<T extends { map<U>(project: (row: string) => U): U[] }>(
  source: T,
): void;

export function Rows() {
  return (
    <ul>
      {aliasedArray.map((row) => (
        <li>{row}</li>
      ))}
      {aliasedTuple.map((row) => (
        <li>{row}</li>
      ))}
      {collection.map((row) => (
        <li>{row}</li>
      ))}
    </ul>
  );
}

export function UnprovenRows<T extends { map<U>(project: (row: string) => U): U[] }>(
  source: T,
) {
  unproven(source);
  return (
    <ul>
      {source.map((row) => (
        <li>{row}</li>
      ))}
    </ul>
  );
}
