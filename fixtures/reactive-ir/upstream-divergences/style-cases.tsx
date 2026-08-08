// v1/style-prop against the vendored known-css-properties set: an unknown
// property whose kebab form is real gets the rename fix, an unknown property
// with no real kebab form is reported without one, custom properties are
// CSS's own escape hatch, and only a *direct* object literal is inspected.
function wrap<T>(value: T): T {
  return value;
}

export function StyleCases() {
  return (
    <div>
      <div style={{ COLOR: "red" }} />
      <div style={{ maxWidth: "3px" }} />
      <div style={{ "max-width": 3 }} />
      <div style={{ "--brand": "red", color: "red" }} />
      <div style={wrap({ marginTop: 4 })} />
      <div style={{ margin: 0 }} />
    </div>
  );
}
