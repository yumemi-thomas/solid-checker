import { createTrackedEffect, onSettled } from "solid-js";
import { listen, startTicker, startTickerAlways, startTickerSilent } from "reactive-package";

// `listen` registers a cleanup on every call: inside the leaf owner that is
// CLEANUP_IN_FORBIDDEN_SCOPE.
export function CleanupInTrackedEffect() {
  createTrackedEffect(() => {
    listen();
  });
  return <div />;
}

// `startTickerAlways` registers a computation on every call: inside the leaf
// owner that is PRIMITIVE_IN_FORBIDDEN_SCOPE.
export function ComputationInTrackedEffect() {
  createTrackedEffect(() => {
    startTickerAlways();
  });
  return <div />;
}

// An owner-backed onSettled is a leaf owner too.
export function CleanupInSettled() {
  onSettled(() => {
    listen();
  });
  return <div />;
}

// `startTicker` only may register: no proven violation.
export function MaybeInTrackedEffect() {
  createTrackedEffect(() => {
    startTicker();
  });
  return <div />;
}

// `startTickerSilent` registers nothing.
export function SilentInTrackedEffect() {
  createTrackedEffect(() => {
    startTickerSilent();
  });
  return <div />;
}

// A function built in the leaf callback runs later, outside the leaf scope.
export function DeferredInTrackedEffect() {
  createTrackedEffect(() => {
    queueMicrotask(() => listen());
  });
  return <div />;
}

// In the component body the owner is a normal one.
export function InComponentBody() {
  listen();
  startTickerAlways();
  return <div />;
}
