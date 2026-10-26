// probus-hk stores/settings.ts:183: a module exports its signals in an object
// literal (`export const settings = { theme, setTheme, ... }`).
import { createSignal } from "solid-js";

export type ThemeChoice = "light" | "dark" | "system";
const [theme, setTheme] = createSignal<ThemeChoice>("system");
export const settings = { theme, setTheme };
