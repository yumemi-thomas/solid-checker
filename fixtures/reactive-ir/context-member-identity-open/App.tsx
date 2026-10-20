import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
import { TabsContext, useTabs } from "./tabs";
import { AccountContext, useAccount } from "./account";

export function TabsProvider(props: { children?: JSX.Element }) {
  const [active] = createSignal("a");
  const [orientation] = createSignal("h");
  return <TabsContext value={{ active, orientation }}>{props.children}</TabsContext>;
}

// Control: in an open program an unseen importer can provide TabsContext or
// call useTabs, so the export list keeps the context open.
export function TabsList() {
  const ctx = useTabs();
  const orientation = ctx.orientation();
  return <p>{orientation}</p>;
}

// Control: exported declarations are no different. An importer could render
// its own AccountContext provider or replace `name` on the shared object.
export function AccountProvider(props: { children?: JSX.Element }) {
  const [name] = createSignal("ada");
  return <AccountContext value={{ name }}>{props.children}</AccountContext>;
}

export function AccountName() {
  const account = useAccount();
  const name = account.name();
  return <p>{name}</p>;
}
