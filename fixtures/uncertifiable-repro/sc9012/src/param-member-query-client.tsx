import { createContext, createMemo, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

interface ApiClient {
  readonly get: (path: string) => Promise<{ ok: boolean }>;
}

const ApiContext = createContext<ApiClient>({ get: async () => ({ ok: true }) });

const loaders = new Map<string, () => Promise<boolean>>();

function createQuery(options: { key: () => string; load: () => Promise<boolean> }) {
  const bound = createMemo(() => {
    const key = options.key();
    loaders.set(key, options.load);
    return key;
  });
  return { state: () => bound() };
}

async function fetchTopic(api: ApiClient, name: string): Promise<boolean> {
  const answer = await api.get(`/topics/${name}`);
  return answer.ok;
}

export default function Topic(): JSX.Element {
  const api = useContext(ApiContext);
  const topic = createQuery({
    key: () => `topics/${"orders"}`,
    load: () => fetchTopic(api, "orders"),
  });
  return <p>{topic.state()}</p>;
}
