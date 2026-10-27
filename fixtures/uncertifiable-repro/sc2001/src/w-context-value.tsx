// error-menu-web account.tsx:19-40, 52: a provider hands a context value whose members are its
// local functions (`value={{ state, reload }}`); `reload` writes, and the context consumers are
// not followed.
import type { JSX } from "@solidjs/web";
import { createContext, createSignal, useContext } from "solid-js";

type Account = { name: () => string; reload: () => Promise<void> };

const AccountContext = createContext<Account>();

export const AccountProvider = (props: { children?: JSX.Element }) => {
  const [name, setName] = createSignal("");

  const reload = async (): Promise<void> => {
    setName("loading");
  };

  return <AccountContext value={{ name, reload }}>{props.children}</AccountContext>;
};

export const useAccount = (): Account => useContext(AccountContext);
