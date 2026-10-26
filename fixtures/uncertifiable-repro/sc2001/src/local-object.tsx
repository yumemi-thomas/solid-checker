// expenses-app src/lib/convex.ts:76-96: setters called from functions in a
// local `value` object that is only handed to the context provider.
import { createContext, createSignal, type ParentProps } from "solid-js";

const AuthContext = createContext<{
  isAuthenticated: () => boolean;
  setAuthenticated: (next: boolean) => void;
  setRefreshing: (next: boolean) => void;
}>({ isAuthenticated: () => false, setAuthenticated: () => {}, setRefreshing: () => {} });

export function AuthProvider(props: ParentProps) {
  const [authenticated, setAuthenticated] = createSignal(false);
  const [refreshing, setRefreshing] = createSignal(false);
  const value = {
    isAuthenticated: () => authenticated() && !refreshing(),
    setAuthenticated: (next: boolean) => {
      setAuthenticated(next);
      if (!next) setRefreshing(false);
    },
    setRefreshing,
  };
  return AuthContext({
    value,
    get children() {
      return props.children;
    },
  });
}
