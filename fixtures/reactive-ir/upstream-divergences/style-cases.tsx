// v1/style-prop after the 2026-08-17 narrowing (see docs/precision-backlog.md).
//
// The object-key checks are TypeScript's on an intrinsic element: `csstype`'s
// `CSSProperties` reports a camelCase key (TS2561, with the kebab suggestion),
// an unknown key (TS2353), and a unitless number for a length (TS2322), and its
// excess-property check has the same subject this rule inspects -- a fresh
// object literal written in place. So on a `<div>` every one of those keys is
// now silent here.
//
// Two shapes still report, and both are cases no type answers:
//
//   * a `-`-prefixed key, which `CSSProperties`'s
//     `[key: `-${string}`]: string | number | undefined` index signature
//     absorbs whatever it is spelled -- upstream's own case 02;
//   * any key on a *component*, whose props are whatever it declares.
//
// The string form is unaffected on every element and keeps its own fixture
// cases elsewhere; `--` custom properties stay CSS's escape hatch, and only a
// *direct* object literal is ever inspected.
function wrap<T>(value: T): T {
  return value;
}

// Permissive props: TypeScript admits every key, so the rule is the only thing
// that can speak here.
const Panel = (props: Record<string, unknown>) => <div>{String(props.style)}</div>;

export function StyleCases() {
  return (
    <div>
      {/* Silent now -- each is a TypeScript diagnostic on an intrinsic. */}
      <div style={{ COLOR: "red" }} />
      <div style={{ maxWidth: "3px" }} />
      <div style={{ "max-width": 3 }} />
      {/* Reported: the index signature absorbs `-`-led keys, so `tsc` is
          silent and the kebab rename is the checker's to offer. */}
      <div style={{ "-webkitAlignContent": "center" }} />
      {/* Reported without a fix: no real kebab form exists either. */}
      <div style={{ "-fooBar": 1 }} />
      {/* Reported: a component's props may admit anything. */}
      <Panel style={{ COLOR: "red" }} />
      <Panel style={{ maxWidth: "3px" }} />
      {/* Clean: a custom property, a wrapper-built object, and a legal entry. */}
      <div style={{ "--brand": "red", color: "red" }} />
      <div style={wrap({ marginTop: 4 })} />
      <div style={{ margin: 0 }} />
    </div>
  );
}
