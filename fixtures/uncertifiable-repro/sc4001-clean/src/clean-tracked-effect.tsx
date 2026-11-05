// Clean. Mirrors viviana-ui-web apps/web/src/components/showcase/GlasselatedShell.tsx:30-33: a
// tracked effect reads a signal and schedules a frame, with no write and no owned primitive inside.
import { createSignal, createTrackedEffect } from "solid-js";

export function Shell() {
  const [theme, setTheme] = createSignal("dark");
  const align = () => console.log("aligned");
  createTrackedEffect(() => {
    theme();
    requestAnimationFrame(align);
  });
  return <button type="button" onClick={() => setTheme(theme() === "dark" ? "light" : "dark")}>{theme()}</button>;
}
