import { createSignal } from "solid-js";
import { NamedImported } from "./named-button";

type Props = { onPress: () => void };
declare function keep(value: unknown): void;

function Named(props: Props) {
  const handler = () => props.onPress();
  return <button onClick={handler} />;
}

function WrappedNamed(props: Props) {
  const handler = () => props.onPress();
  return <button onClick={handler as Props["onPress"]} />;
}

function EagerNamed(props: Props) {
  const handler = () => props.onPress();
  handler();
  return <button onClick={handler} />;
}

function EscapedNamed(props: Props) {
  const handler = () => props.onPress();
  keep(handler);
  return <button onClick={handler} />;
}

function AliasedNamed(props: Props) {
  const handler = () => props.onPress();
  const alias = handler;
  return <button onClick={alias} />;
}

function MutableNamed(props: Props) {
  let handler = () => props.onPress();
  const earlier = handler;
  handler = () => {};
  keep(earlier);
  return <button onClick={handler} />;
}

function ShadowedNamed(props: Props) {
  const handler = () => props.onPress();
  {
    const handler = () => {};
    handler();
  }
  return <button onClick={handler} />;
}

function ShadowedEagerNamed(props: Props) {
  const handler = () => props.onPress();
  handler();
  {
    const handler = () => {};
    return <button onClick={handler} />;
  }
}

function CallbackDefault(props: Props) {
  const handler = (run = () => props.onPress()) => { run(); };
  keep(handler);
  return <button onClick={() => handler()} />;
}

function NamedThroughComponent(props: Props) {
  const handler = () => props.onPress();
  return <NamedImported onPress={handler} />;
}

function RepeatedNamed(props: Props) {
  const handler = () => props.onPress();
  return <div><NamedImported onPress={handler} /><NamedImported onPress={handler} /></div>;
}

function RepeatedInline(props: Props) {
  return <div><NamedImported onPress={props.onPress} /><NamedImported onPress={props.onPress} /></div>;
}

function NamedCycle(props: Props): ReturnType<typeof NamedImported> {
  const handler = () => props.onPress();
  return <NamedCycle onPress={handler} />;
}

// Each caller's signal makes the strict-read question independent of whether
// an exported component's props have enumerated callers.
export function UsesNamed() {
  const [count] = createSignal(0);
  return <Named onPress={() => { count(); }} />;
}
export function UsesWrappedNamed() {
  const [count] = createSignal(0);
  return <WrappedNamed onPress={() => { count(); }} />;
}
export function UsesEagerNamed() {
  const [count] = createSignal(0);
  return <EagerNamed onPress={() => { count(); }} />;
}
export function UsesEscapedNamed() {
  const [count] = createSignal(0);
  return <EscapedNamed onPress={() => { count(); }} />;
}
export function UsesAliasedNamed() {
  const [count] = createSignal(0);
  return <AliasedNamed onPress={() => { count(); }} />;
}
export function UsesMutableNamed() {
  const [count] = createSignal(0);
  return <MutableNamed onPress={() => { count(); }} />;
}
export function UsesShadowedNamed() {
  const [count] = createSignal(0);
  return <ShadowedNamed onPress={() => { count(); }} />;
}
export function UsesShadowedEagerNamed() {
  const [count] = createSignal(0);
  return <ShadowedEagerNamed onPress={() => { count(); }} />;
}
export function UsesCallbackDefault() {
  const [count] = createSignal(0);
  return <CallbackDefault onPress={() => { count(); }} />;
}

export function UsesImportedNamed() {
  const [count] = createSignal(0);
  return <NamedImported onPress={() => { count(); }} />;
}

export function UsesNamedThroughComponent() {
  const [count] = createSignal(0);
  return <NamedThroughComponent onPress={() => { count(); }} />;
}
export function UsesRepeatedNamed() {
  const [count] = createSignal(0);
  return <RepeatedNamed onPress={() => { count(); }} />;
}
export function UsesRepeatedInline() {
  const [count] = createSignal(0);
  return <RepeatedInline onPress={() => { count(); }} />;
}
export function UsesNamedCycle() {
  const [count] = createSignal(0);
  return <NamedCycle onPress={() => { count(); }} />;
}
