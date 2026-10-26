// inferay BackgroundScenePicker.tsx:90,148 (useAppAppearance.tsx:111), WorkspaceSidebar/index.tsx:173: handlers
// call a project function with an object, or a value it wraps in one, that the function hands to another
// project function, on an element or through a project component's prop. The handlers are placed only while no
// dispatch site of the project withholds that proof (ADR 0356), here keyboard.tsx's.
import { createSignal } from "solid-js";

type Patch = { blur?: number; mode?: string };

function project(name: string, input: { patch: Patch }): Patch {
  return { ...input.patch, mode: name };
}

export function updateBackground(patch: Patch): void {
  console.debug(project("background", { patch }));
}

const openSkill = (skillId: string) => project("skills", { patch: { mode: skillId } });

function Choice(props: { onChoose: (skillId: string) => void; skillId: string }) {
  return <button onClick={() => props.onChoose(props.skillId)}>{props.skillId}</button>;
}

export function Picker(props: { skillId: string }) {
  const [blur, setBlur] = createSignal(0);
  return (
    <div>
      <input
        type="range"
        value={blur()}
        onInput={(event) => updateBackground({ blur: Number(event.currentTarget.value) })}
      />
      <button onClick={() => openSkill(props.skillId)}>edit</button>
      <Choice skillId="x" onChoose={(skillId) => openSkill(skillId)} />
    </div>
  );
}
