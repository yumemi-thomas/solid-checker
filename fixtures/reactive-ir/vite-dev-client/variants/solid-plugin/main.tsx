import { CalledDirectly, CalledOnAny, Logged, Mapped, TemplateSubstitution } from "./App";

export const product = [CalledDirectly, CalledOnAny, TemplateSubstitution, Mapped, Logged];
// Vite's client types (`vite/client`) declare `import.meta.hot`: the program names the package `vite` by its declarations only.
export const hot = import.meta.hot;
