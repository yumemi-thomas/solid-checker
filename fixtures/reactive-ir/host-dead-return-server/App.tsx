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

// The same components and package as `host-dead-return`, under a Node SSR
// host. `isServer` is `true`, so the fold keeps each guarded
// server return and removes the signal factory after it: nothing is reported.

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

// The server arm's slot is `idle`, a closure the factory declares: it
// resolves, and reads nothing.
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

export function ClientLoop() {
  const [, start] = createClientLoop(frame);
  start();
  return <main />;
}

export function FlaggedLoop() {
  const [, start] = createFlaggedLoop(flag, frame);
  start();
  return <main />;
}

// `isServer || flag` is decided here: `true`.
export function EitherLoop() {
  const [, start] = createEitherLoop(flag, frame);
  start();
  return <main />;
}

export function NamespacedLoop() {
  const [, start] = createNamespacedLoop(frame);
  start();
  return <main />;
}

export function ProjectLoop() {
  const [, start] = createProjectLoop(frame);
  start();
  return <main />;
}
