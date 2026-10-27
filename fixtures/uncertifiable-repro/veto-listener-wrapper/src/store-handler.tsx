// probus-hk AlertSheet.tsx:44, SettingsSheet.tsx:341,384 (stores/settings.ts, sheets.ts): handlers call members of
// a module's object literal that runs a call before it, one on an element, one through a project component's
// prop. Both are placed as event handlers only while no dispatch site of the project withholds that
// proof (ADR 0356), here keyboard.tsx's.
import { createSignal } from "solid-js";

function storedLang(): string {
  return "en";
}

const [lang, setLang] = createSignal(storedLang(), { ownedWrite: true });

export const settings = {
  lang,
  setLang(next: string) {
    setLang(next);
  },
  reset() {
    setLang("en");
  },
};

function Toggle(props: { onChange: (next: string) => void }) {
  return <button onClick={() => props.onChange("zh")}>zh</button>;
}

export function LangSwitch() {
  return (
    <div>
      <button onClick={() => settings.reset()}>{settings.lang()}</button>
      <Toggle onChange={(next) => settings.setLang(next)} />
    </div>
  );
}
