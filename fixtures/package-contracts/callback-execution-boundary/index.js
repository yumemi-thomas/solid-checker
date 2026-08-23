import { schedule } from "./schedule.js";

export function Inline(onData) {
  onData(1);
  return { done: true };
}

export function Escaping(props, onData) {
  const value = schedule(props.client, () => onData(1));
  return { value };
}

export function Returned(onData) {
  const run = () => onData(1);
  return { run };
}
