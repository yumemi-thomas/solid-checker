import { makeCache } from "./cache-factory";
export function CacheConsumer() {
  const store = makeCache();
  const value = store.ready();
  return <p>{value}</p>;
}
