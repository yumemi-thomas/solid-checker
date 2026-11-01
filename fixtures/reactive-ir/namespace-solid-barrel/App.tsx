import * as Star from "./star";
import * as Named from "./named";
import * as Shadow from "./shadow";

// Violations, each confirmed by rc.17 in Chrome (ADR 0517): a namespace of a
// project module that re-exports Solid names the primitive its member's
// symbol resolves to.
export function StarRead() {
  const [r] = Star.createSignal(1);
  const seen = r();
  return <p>{seen}</p>;
}

export function StarWriteInMemo() {
  const [v, setV] = Star.createSignal(0);
  Star.createMemo(() => { setV(1); return 1; });
  return <p>{v()}</p>;
}

export function NamedRead() {
  const [r] = Named.createSignal(1);
  const seen = r();
  return <p>{seen}</p>;
}

// Clean: the read is tracked by JSX.
export function StarTracked() {
  const [r] = Star.createSignal(1);
  return <p>{r()}</p>;
}

// Clean: the module's own `createSignal` shadows the star export.
export function ShadowRead() {
  const [r] = Shadow.createSignal(1);
  const seen = r();
  return <p>{seen}</p>;
}
