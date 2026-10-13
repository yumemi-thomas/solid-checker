import { createSignal } from "solid-js";
import {
  useAfterPattern,
  useArrowTarget,
  useChained,
  useDeferred,
  useForwarded,
  useGuarded,
  useMaybe,
  useReplaced,
  useTimer,
} from "./helpers";

declare const flag: boolean;

export function Forwarded() {
  const [name] = createSignal("a");
  const label = useForwarded(name);
  return <p>{label}</p>;
}

export function Chained() {
  const [name] = createSignal("a");
  const label = useChained(name);
  return <p>{label}</p>;
}

export function Guarded() {
  const [name] = createSignal("a");
  const label = useGuarded(flag, name);
  return <p>{label}</p>;
}

export function Deferred() {
  const [name] = createSignal("a");
  const label = useDeferred(name);
  return <p>{label()}</p>;
}

export function Maybe() {
  const [name] = createSignal("a");
  const label = useMaybe(flag, name);
  return <p>{label}</p>;
}

export function Timer() {
  const [name] = createSignal("a");
  const label = useTimer(name);
  return <p>{label}</p>;
}

export function Replaced() {
  const [name] = createSignal("a");
  const label = useReplaced(name);
  return <p>{label}</p>;
}

export function AfterPattern() {
  const [name] = createSignal("a");
  const label = useAfterPattern({ label: "x" }, name);
  return <p>{label}</p>;
}

export function ArrowTarget() {
  const [name] = createSignal("a");
  const label = useArrowTarget(name);
  return <p>{label}</p>;
}
