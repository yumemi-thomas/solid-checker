import { createContext, createSignal, useContext, type Accessor, type Setter } from "solid-js";
import * as S from "solid-js";

const ThemeContext = createContext<readonly [Accessor<string>, Setter<string>]>();

// Certified: the slots are the provider's signal, read tracked and written in a handler.
function ThemeButton() {
  const [theme, setTheme] = useContext(ThemeContext);
  return <button onClick={() => setTheme((value) => (value === "light" ? "dark" : "light"))}>{theme()}</button>;
}

// Violation (was uncertifiable): the accessor slot read in the body, untracked.
function ThemeLabel() {
  const [theme] = S.useContext(ThemeContext);
  const now = theme();
  return <span>{now}</span>;
}

export function ThemeApp() {
  const value = createSignal("light");
  return (
    <ThemeContext value={value}>
      <ThemeButton />
      <ThemeLabel />
    </ThemeContext>
  );
}

const InlineContext = createContext<readonly [Accessor<number>, Setter<number>]>();

// Violation (was uncertifiable): an inline creation as the provider's value.
function Count() {
  const [count] = useContext(InlineContext);
  const now = count();
  return <span>{now}</span>;
}

export function InlineApp() {
  return (
    <InlineContext value={createSignal(0)}>
      <Count />
    </InlineContext>
  );
}
