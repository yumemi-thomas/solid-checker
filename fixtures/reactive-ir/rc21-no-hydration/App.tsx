import { createSignal, createStore, Hydration, NoHydration } from "solid-js";
import { hydrate } from "@solidjs/web";

const [st] = createStore({ a: 1 });

function Read() { const [a] = createSignal(0); const v = a(); return <i>{v}</i>; }
function Write() { st.a = 2; return <i />; }
function Nested() { const [a] = createSignal(0); const v = a(); return <i>{v}</i>; }

function Outside() { const [a] = createSignal(0); const v = a(); return <i>{v}</i>; }

export const App = () => (
  <main>
    <NoHydration>
      <Read />
      <Write />
      <div><Hydration><Nested /></Hydration></div>
    </NoHydration>
    <Outside />
  </main>
);
hydrate(() => <App />, document.body);
