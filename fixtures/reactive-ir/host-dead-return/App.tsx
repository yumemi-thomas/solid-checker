import createDefaultLoop, {
  createBlockLoop,
  createClientLoop,
  createEitherLoop,
  createFlaggedLoop,
  createIdleLoop,
  createLoop,
  createNamespacedLoop,
} from "dep-loop";
import { createProjectLoop } from "./factories";

declare const flag: boolean;
const frame = () => {};

// --- Proven under the browser host: the `isServer` return is folded away. ---

export function Loop() {
  const [running, start, stop] = createLoop(frame);
  start();
  return <button onClick={stop}>{String(running())}</button>;
}

export function DefaultLoop() {
  const [, start] = createDefaultLoop(frame);
  start();
  return <main />;
}

export function IdleLoop() {
  const [, start] = createIdleLoop(frame);
  start();
  return <main />;
}

export function BlockLoop() {
  const [, start] = createBlockLoop(frame);
  start();
  return <main />;
}

// --- Refused (silent). ---

// The live return sits inside `if (!isServer) { ... }`: the fold leaves it
// under a control test (now the literal `!0`), so it is not the factory's one
// unconditional return.
export function ClientLoop() {
  const [, start] = createClientLoop(frame);
  start();
  return <main />;
}

// A guard no host decides: an ordinary parameter.
export function FlaggedLoop() {
  const [, start] = createFlaggedLoop(flag, frame);
  start();
  return <main />;
}

// `isServer || flag`: the host constant takes part but does not decide.
export function EitherLoop() {
  const [, start] = createEitherLoop(flag, frame);
  start();
  return <main />;
}

// `web.isServer` through a namespace import, which the fold does not bind.
export function NamespacedLoop() {
  const [, start] = createNamespacedLoop(frame);
  start();
  return <main />;
}

// A project file is not folded: its factory keeps two return sites.
export function ProjectLoop() {
  const [, start] = createProjectLoop(frame);
  start();
  return <main />;
}
