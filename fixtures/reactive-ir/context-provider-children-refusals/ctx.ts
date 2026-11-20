import { createContext } from "solid-js";

// A context another module provides: the children arm takes a provider of this file's own `const` only.
export const SharedContext = createContext<number>();
