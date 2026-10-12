import { createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";
import { ThemeContext } from "./theme-context";
import * as themes from "./theme-context";

export function ThemeProvider(props: { children?: JSX.Element }) {
  const [mode] = createSignal("dark");
  return <ThemeContext value={{ mode }}>{props.children}</ThemeContext>;
}

// A second provider through a namespace member: its tag is not one exact
// identifier, so no provider census is closed and the read below is not
// attributed to the signal.
export function FixedTheme(props: { children?: JSX.Element }) {
  return <themes.ThemeContext value={{ mode: () => "light" }}>{props.children}</themes.ThemeContext>;
}

export function ThemeLabel() {
  const theme = useContext(ThemeContext);
  const mode = theme.mode();
  return <span>{mode}</span>;
}
