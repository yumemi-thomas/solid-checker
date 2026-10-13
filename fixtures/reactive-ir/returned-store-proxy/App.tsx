import { createSignal } from "solid-js";
import { useCrossFileForward } from "./forward";
import {
  useArrowProxy,
  useEscapedRoot,
  useFallthrough,
  useForwarded,
  useForwardedEither,
  useForwardingProxy,
  useHasTrap,
  useHeldHandler,
  useLeakedTarget,
  useOtherReturn,
  useOtherStore,
  usePlainTarget,
  useSetTrap,
  useSignalTarget,
  useSpreadHandler,
  useTwoParameterProxy,
  useWrittenTarget,
} from "./hook";

// Violations: the hook returns a proxy that reads its store, read untracked
// in the body.
export function ForwardingProxy() {
  const query = useForwardingProxy();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function TwoParameterProxy() {
  const query = useTwoParameterProxy();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function ArrowProxy() {
  const query = useArrowProxy();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function Forwarded() {
  const query = useForwarded();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: the handler, the trap, the target or the forward is outside the
// rule.
export function SetTrap() {
  const query = useSetTrap();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function HasTrap() {
  const query = useHasTrap();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function OtherReturn() {
  const query = useOtherReturn();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function OtherStore() {
  const query = useOtherStore();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function WrittenTarget() {
  const query = useWrittenTarget();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function LeakedTarget() {
  const query = useLeakedTarget();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function Fallthrough() {
  const query = useFallthrough();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function PlainTarget() {
  const query = usePlainTarget();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function SignalTarget() {
  const query = useSignalTarget();
  const [config] = createSignal(query.name);
  return <main>{config()}</main>;
}
export function SpreadHandler() {
  const query = useSpreadHandler();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function HeldHandler() {
  const query = useHeldHandler();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function EscapedRoot() {
  const query = useEscapedRoot();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function ForwardedEither() {
  const query = useForwardedEither(true);
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
export function CrossFileForward() {
  const query = useCrossFileForward();
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
