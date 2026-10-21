// SC2003 no-direct-mutation, observed behaviourally. A write to a store root's own
// property outside a setter is refused: the store keeps its value and the DOM keeps the
// old name. The control writes through the setter and the DOM follows. The store refuses
// the direct write, so no binding to the store can show it changed; the violation's probe
// is a node its own handler fills with the value it assigns, which shows the write was
// issued with a new value. Each reflect button sets the value its observation declares.
import { createSignal, createStore } from "solid-js";

export function Violation() {
  const [profile] = createStore({ name: "Ada" });
  const [assigned, setAssigned] = createSignal("");
  return (
    <section>
      <button data-probe="violation-write" onClick={() => { profile.name = "Grace"; }}>write</button>
      <button data-probe="violation-reflect" onClick={() => { profile.name = "Grace1"; setAssigned("Grace1"); }}>reflect</button>
      <p data-probe="violation-name">{profile.name}</p><i data-probe="violation-probe">{assigned()}</i>
    </section>
  );
}

export function Control() {
  const [profile, setProfile] = createStore({ name: "Ada" });
  return (
    <section>
      <button data-probe="control-write" onClick={() => setProfile((draft) => { draft.name = "Grace"; })}>write</button>
      <button data-probe="control-reflect" onClick={() => setProfile((draft) => { draft.name = "Grace2"; })}>reflect</button>
      <p data-probe="control-name">{profile.name}</p><i data-probe="control-probe">{profile.name}</i>
    </section>
  );
}
