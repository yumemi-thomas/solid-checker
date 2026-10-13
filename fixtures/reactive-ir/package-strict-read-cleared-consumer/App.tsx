import { watchStatus, peekStatus } from "reactive-package";
import { readClearedFromModule } from "./wrapper";

export function ClearedAtCall() {
  watchStatus();
  return <div />;
}

export function NoClearingFact() {
  peekStatus();
  return <div />;
}

function localWrapper() {
  watchStatus();
}

export function ClearedThroughWrapper() {
  localWrapper();
  return <div />;
}

export function ClearedThroughModule() {
  readClearedFromModule();
  return <div />;
}
