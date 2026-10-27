// Clean. Mirrors donegeon-client web/apps/client/src/context/ApiContext.tsx:11-21: a provider element
// with a value and a hook that reads the context and narrows it, used under the provider.
import { createContext, useContext } from "solid-js";

type Api = { name: () => string };
const ApiContext = createContext<Api>();

function useApi(): Api {
  const ctx = useContext(ApiContext);
  if (!ctx) throw new Error("ApiContext is not available");
  return ctx;
}

function Label() {
  const api = useApi();
  return <p>{api.name()}</p>;
}

function ApiProvider(props: { client: Api; children: any }) {
  return <ApiContext value={props.client}>{props.children}</ApiContext>;
}

export function App() {
  return (
    <ApiProvider client={{ name: () => "api" }}>
      <Label />
    </ApiProvider>
  );
}
