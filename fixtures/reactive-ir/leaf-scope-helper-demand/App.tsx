import { createSignal, onCleanup, onSettled } from "solid-js";
import { importedHelper } from "./imported";

const label = " value ";
function readLabel() {
  const value = label.trim();
  return value.length;
}
function middle() {
  readLabel();
}
function register() {
  readLabel();
  onCleanup(() => {});
}

export function LocalHelperClean() {
  onSettled(() => { middle(); });
  return <div />;
}

export function LocalHelperForbidden() {
  onSettled(() => { register(); });
  return <div />;
}

export function SetterCallbackClean() {
  const [, setLabel] = createSignal("");
  const update = () => {
    setLabel(() => {
      const value = new Date().toLocaleTimeString();
      return value;
    });
  };
  onSettled(() => { update(); });
  return <div />;
}

export function WrappedHelperClean() {
  onSettled(() => { (middle satisfies () => void)(); });
  return <div />;
}

export function ShadowedHelper(props: { middle: () => void }) {
  const middle = props.middle;
  onSettled(() => { middle(); });
  return <div />;
}

declare const supplied: { run(): void };
function opaqueHelper() {
  supplied.run();
}
export function OpaqueMember() {
  onSettled(() => { opaqueHelper(); });
  return <div />;
}

export function ImportedHelperStillOpen() {
  onSettled(() => { importedHelper(); });
  return <div />;
}
