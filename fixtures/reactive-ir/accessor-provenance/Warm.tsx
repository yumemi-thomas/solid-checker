import { makeWarm } from "./warm-factory";

export function WarmConsumer() {
  const store = makeWarm();
  const value = store.other();
  return <p>{value}</p>;
}
