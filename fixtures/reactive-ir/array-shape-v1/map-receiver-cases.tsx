// The second `arrayShape` consumer: `v1/prefer-for` (SC8012) reports every
// single-callback `.map()` in JSX by name, as upstream does, but offers the
// `<For each>` rewrite only when the receiver is a *proven* array. Rewriting an
// Immutable.js collection — or anything else that has a `.map` but is not an
// array — would change behaviour.
//
// So these cases are about the *fix*, not the report: all four report, and only
// the array-receiver ones carry an autofix.
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
