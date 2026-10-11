import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";
import { replaceCrossAccount } from "./context-mutation";

type Account = { state: () => number };
const Mutated = createContext<Account>();
export function MutatedProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const account = { state };
  return <Mutated value={account}>{props.children}</Mutated>;
}
const useMutated = () => useContext(Mutated);
function replaceAccount() {
  const replaced = useMutated();
  replaced.state = () => 0;
}
export function SecondConsumerMutation() {
  replaceAccount();
  const account = useMutated();
  const value = account.state();
  return <p>{value}</p>;
}

export const CrossAccount = createContext<Account>();
export function CrossProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const account = { state };
  return <CrossAccount value={account}>{props.children}</CrossAccount>;
}
export function CrossConsumerMutation() {
  replaceCrossAccount();
  const account = useContext(CrossAccount);
  const value = account.state();
  return <p>{value}</p>;
}

const Escaping = createContext<Account>();
export function EscapingProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const account = { state };
  return <Escaping value={account}>{props.children}</Escaping>;
}
const useEscaping = () => useContext(Escaping);
declare function opaque(value: Account): void;
function escapeAccount() {
  const escaped = useEscaping();
  opaque(escaped);
}
export function SecondConsumerEscape() {
  escapeAccount();
  const account = useEscaping();
  const value = account.state();
  return <p>{value}</p>;
}

const ProviderEscaping = createContext<Account>();
export function ProviderEscape(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const account = { state };
  opaque(account);
  return <ProviderEscaping value={account}>{props.children}</ProviderEscaping>;
}
export function ProviderEscapeConsumer() {
  const account = useContext(ProviderEscaping);
  const value = account.state();
  return <p>{value}</p>;
}

const Unbound = createContext<Account>();
export function UnboundProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const account = { state };
  return <Unbound value={account}>{props.children}</Unbound>;
}
export function UnboundMutationConsumer() {
  useContext(Unbound).state = () => 0;
  const account = useContext(Unbound);
  const value = account.state();
  return <p>{value}</p>;
}
