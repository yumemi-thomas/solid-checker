// probus-hk stores/settings.ts:183: a module exports its signals in an object literal
// (`export const settings = { theme, setTheme, ... }`), after a call that is no literal-valued signal.
import { createSignal } from "solid-js";

export type ThemeChoice = "light" | "dark" | "system";

const stored = (): ThemeChoice => "system";
const [theme, setTheme] = createSignal<ThemeChoice>(stored(), { ownedWrite: true });
export const settings = { theme, setTheme };
