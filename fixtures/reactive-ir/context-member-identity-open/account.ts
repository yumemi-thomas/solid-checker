import { createContext, useContext } from "solid-js";

type Account = { name: () => string };
export const AccountContext = createContext<Account>();
export const useAccount = () => useContext(AccountContext);
