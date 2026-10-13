import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";
import { Badge, useLabel, useLabelMemo } from "dep-hooks";

// Positive: the package's `solid` source reads `name` during the call, through
// a module the entry imports relatively.
export function LabelNow() {
  const [name] = createSignal("a");
  const label = useLabel(name);
  return <p>{label}</p>;
}

// Negative: the package defers the read into a memo.
export function LabelMemo() {
  const [name] = createSignal("a");
  const label = useLabelMemo(name);
  return <p>{label()}</p>;
}

// Negative: the package's own untracked read is not the project's finding.
export function Badged() {
  return <p><Badge text="x" /></p>;
}

// Positive: the package's `Object.assign` with an open source does not veto
// this project's context member.
type Account = { user: () => string };
const AccountContext = createContext<Account>();

export function AccountProvider(props: { children?: JSX.Element }) {
  const [user] = createSignal("u");
  return <AccountContext value={{ user }}>{props.children}</AccountContext>;
}

export function Greeting() {
  const account = useContext(AccountContext);
  const user = account.user();
  return <p>{user}</p>;
}
