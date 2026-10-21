// SC1004 components-return-once, observed behaviourally. The component body reads
// the signal once, and the early return it selects stays on screen after the write:
// the DOM is stale. The control returns one tree and moves the branch into Show.
import { createSignal, Show } from "solid-js";

export function Violation() {
  const [user, setUser] = createSignal<string | null>(null);
  if (!user()) {
    return (
      <section>
        <button data-probe="violation-login" onClick={() => setUser("ada")}>login</button>
        <p data-probe="violation-status">logged out</p>
      </section>
    );
  }
  return (
    <section>
      <p data-probe="violation-status">logged in as {user()}</p>
    </section>
  );
}

export function Control() {
  const [user, setUser] = createSignal<string | null>(null);
  return (
    <section>
      <button data-probe="control-login" onClick={() => setUser("ada")}>login</button>
      <Show when={user()} fallback={<p data-probe="control-status">logged out</p>}>
        {(name) => <p data-probe="control-status">logged in as {name()}</p>}
      </Show>
    </section>
  );
}
