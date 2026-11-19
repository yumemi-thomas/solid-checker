import { createContext, createSignal, useContext, type Accessor, type Setter } from "solid-js";

const ThemeContext = createContext<readonly [Accessor<string>, Setter<string>]>();

function ThemeButton() {
  const [theme, setTheme] = useContext(ThemeContext);
  return (
    <button type="button" onClick={() => setTheme((value) => (value === "light" ? "dark" : "light"))}>
      {theme()}
    </button>
  );
}

export function ThemeApp() {
  const value = createSignal("light");
  return (
    <ThemeContext value={value}>
      <ThemeButton />
    </ThemeContext>
  );
}
