// Clean. Mirrors compass-ui apps/ui/src/components/SettingsView.tsx:85-95: a draft store seeded from
// the app store's accessor, edited by a setter-function in a handler, reset with reconcile.
import { createContext, createStore, reconcile, snapshot, useContext } from "solid-js";

const AppContext = createContext({ config: () => ({ handle: "ada", kind: "linear" }) });

function Settings() {
  const app = useContext(AppContext);
  const seed = () => structuredClone(snapshot(app.config()));
  const [draft, setDraft] = createStore(seed());
  return (
    <div>
      <input
        value={draft.handle}
        onInput={(event) => setDraft((s) => { s.handle = event.currentTarget.value; })}
      />
      <button type="button" onClick={() => setDraft(reconcile(seed()))}>{draft.kind}</button>
    </div>
  );
}

export function App() {
  return <Settings />;
}
