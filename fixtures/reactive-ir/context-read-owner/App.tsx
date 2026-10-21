import { createContext, createMemo, useContext } from "solid-js";

type Theme = "light" | "dark";

// Default-less. This file has no provider, but that is a different claim
// (ContextNotFoundError); here only the owner requirement is under test.
const ThemeContext = createContext<Theme>();
// A default does not discharge the owner requirement: `getContext` checks the
// owner before it looks at the default.
const DefaultedContext = createContext<Theme>("light");

// A project hook whose only statement is the read.
export function useTheme(): Theme {
  return useContext(ThemeContext);
}

// Positive: the hook runs in an event handler, which has no owner. The read
// inside `useTheme` is the proven violation, because the handler is its only
// caller in this file.
export function HandlerHook() {
  return <button onClick={() => useTheme()}>theme</button>;
}

// Positive: a timer callback runs with no owner.
export function TimerRead() {
  setTimeout(() => useContext(ThemeContext), 0);
  return <i>timer</i>;
}

// Positive: module scope runs with no owner.
export const moduleTheme = useContext(DefaultedContext);

// Positive: the code after the first `await` is a continuation with no owner,
// whatever calls this function.
export async function loadTheme(): Promise<Theme> {
  await Promise.resolve();
  return useContext(ThemeContext);
}

// Positive: a context with a default read in an event handler still throws.
export function DefaultedInHandler() {
  return <button onClick={() => useContext(DefaultedContext)}>defaulted</button>;
}

// Negative: a component body has an owner.
export function Panel() {
  const theme = useContext(ThemeContext);
  return <i>{theme}</i>;
}

// Negative: the read is in the body of a component rendered under the provider.
export function Themed() {
  return (
    <ThemeContext value="dark">
      <Panel />
    </ThemeContext>
  );
}

// Negative: a computation a component creates runs under that computation's owner.
export function Memoized() {
  const label = createMemo(() => useContext(DefaultedContext));
  return <i>{label()}</i>;
}

// Negative: a local binding named `useContext` is not the Solid primitive.
export function Shadowed() {
  const useContext = (value: Theme) => value;
  return <button onClick={() => useContext("dark")}>shadowed</button>;
}
