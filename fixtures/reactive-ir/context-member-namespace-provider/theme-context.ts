import { createContext } from "solid-js";

type Theme = { mode: () => string };
export const ThemeContext = createContext<Theme>();
