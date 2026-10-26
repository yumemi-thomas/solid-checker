// kui feature-schemas SchemasRoute.tsx:234 (kernel feature/context.tsx:169, data/mutation.ts:58, data.ts:471): a
// mutation wrapper's callback hands `kui.api`, a member of the context value a project hook returns, to a project
// helper that calls `.post` on it, and the component reads the wrapper's state signal, so the hidden dispatch could matter.
import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

interface ApiClient {
  readonly post: (path: string) => Promise<{ ok: boolean }>;
}

const KuiContext = createContext<{ readonly api: ApiClient }>({ api: { post: async () => ({ ok: true }) } });

function useKui() {
  return useContext(KuiContext);
}

function createMutation(call: () => Promise<boolean>) {
  const [state, setState] = createSignal("idle");
  const run = async () => {
    await call();
    setState("done");
  };
  return { state, run };
}

async function registerSchema(api: ApiClient): Promise<boolean> {
  const answer = await api.post("/schemas");
  return answer.ok;
}

export default function Registry(): JSX.Element {
  const kui = useKui();
  const register = createMutation(() => registerSchema(kui.api));
  return <button onClick={() => void register.run()}>{register.state()}</button>;
}
