// SC2003 no-direct-mutation, observed behaviourally. A write to a store root's own
// property outside a setter is dropped, so the DOM keeps the old value. The control
// writes through the setter and the DOM follows. The reflect buttons write a fresh
// value on every click, so a write that reaches the DOM is a real change.
import { createStore } from "solid-js";

let tick = 0;

export function Violation() {
  const [profile] = createStore({ name: "Ada" });
  return (
    <section>
      <button data-probe="violation-write" onClick={() => { profile.name = "Grace"; }}>write</button>
      <button data-probe="violation-reflect" onClick={() => { profile.name = `Grace${++tick}`; }}>reflect</button>
      <p data-probe="violation-name">{profile.name}</p>
    </section>
  );
}

export function Control() {
  const [profile, setProfile] = createStore({ name: "Ada" });
  return (
    <section>
      <button data-probe="control-write" onClick={() => setProfile((draft) => { draft.name = "Grace"; })}>write</button>
      <button data-probe="control-reflect" onClick={() => setProfile((draft) => { draft.name = `Grace${++tick}`; })}>reflect</button>
      <p data-probe="control-name">{profile.name}</p>
    </section>
  );
}
