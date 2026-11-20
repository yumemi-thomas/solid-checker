import { createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (F5): an HTML parser decodes `&#106;` to `j` in an attribute's value and Oxc keeps the reference undecoded, so the literal text test cannot read the scheme: an `&` among the scheme's bytes is unknown.
export function Page() {
  return (
    <div onClick={() => { count(); }}>
      <a href="&#106;avascript:void 0">x</a>
    </div>
  );
}
