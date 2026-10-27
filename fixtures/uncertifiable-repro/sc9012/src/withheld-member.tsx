// probus-hk TabBar.tsx:437: another module reads `settings.theme()` in a tracked function; the
// imported object's member is not followed (here it also copies the object, as the app does).
import { settings, type ThemeChoice } from "./withheld-store";

const THEMES: { value: ThemeChoice; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export const snapshot = () => ({ ...settings });

export function ThemeSwitch() {
  const index = () => Math.max(0, THEMES.findIndex((choice) => choice.value === settings.theme()));
  return <button onClick={() => settings.setTheme(THEMES[(index() + 1) % THEMES.length]!.value)}>{THEMES[index()]!.label}</button>;
}
