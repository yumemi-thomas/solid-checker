import { createContext, createMemo, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

type Account = { state: () => number; user: () => number; plain: () => number };
export const AccountContext = createContext<Account>();
export function AccountProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  const user = () => state();
  const account = { state, user, plain: () => 0 };
  return <AccountContext value={account}>{props.children}</AccountContext>;
}
export const useAccount = () => useContext(AccountContext);

const DbContext = createContext<() => number>();
function asyncMemo<T>(compute: () => Promise<T>): () => T {
  return createMemo(compute) as unknown as (() => T);
}
export function DbProvider(props: { children?: JSX.Element }) {
  const [live] = createSignal<number | undefined>(undefined);
  const initial = asyncMemo(() => Promise.resolve(1));
  const cached = () => live() ?? initial();
  return <DbContext value={cached}>{props.children}</DbContext>;
}
export function useDb() {
  const cached = useContext(DbContext);
  return () => cached();
}

const Ambiguous = createContext<Account>();
export function First(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <Ambiguous value={{ state, user: () => state(), plain: () => 0 }}>{props.children}</Ambiguous>;
}
export function Second(props: { children?: JSX.Element }) {
  return <Ambiguous value={{ state: () => 0, user: () => 0, plain: () => 0 }}>{props.children}</Ambiguous>;
}
export const useAmbiguous = () => useContext(Ambiguous);

const Defaulted = createContext<Account>({ state: () => 0, user: () => 0, plain: () => 0 });
export function DefaultProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <Defaulted value={{ state, user: () => state(), plain: () => 0 }}>{props.children}</Defaulted>;
}
export const useDefaulted = () => useContext(Defaulted);

const Escaped = createContext<Account>();
export function EscapedProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <Escaped value={{ state, user: () => state(), plain: () => 0 }}>{props.children}</Escaped>;
}
export const useEscaped = () => useContext(Escaped);

const Aliased = createContext<Account>();
export function AliasedProvider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <Aliased value={{ state, user: () => state(), plain: () => 0 }}>{props.children}</Aliased>;
}
export const useAliased = () => useContext(Aliased);
