// Caller-proven prop reactivity is not one question but two, and they have
// opposite quantifiers. Solid 2.0 compiles a statically passed prop to a
// plain property and a dynamically passed one to a getter, so:
//
//   "SOME caller passes a reactive expression"  -- one witness proves it, and
//     the witness is monotone: a consumer outside the project can add a call
//     site, never unwrite the one here. Sound in an open world.
//   "EVERY caller passes a static value"        -- one unseen caller falsifies
//     it. Needs the complete caller set, so it needs a closed world.
//
// Before this fixture an exported component forfeited *both* halves: the
// classification collapsed to "nothing about its props is provable", which
// threw away in-project witnesses and reported a proof obligation where a
// violation was proven.
import { createSignal } from "solid-js";

// Exported, so the caller set is open -- but the visible call site passes a
// dynamic value, which no external caller can remove. Proven violation.
export function WitnessedDynamic(props: { title: string }) {
  const title = props.title;
  return <h1>{title}</h1>;
}

// Exported with only a static visible call site. An external consumer may
// pass a dynamic value, so this stays an uncertifiable obligation -- it must
// never certify as silent on the strength of the static witness.
export function OnlyStaticWitness(props: { label: string }) {
  const label = props.label;
  return <h1>{label}</h1>;
}

// Not exported and only ever passed a static value: the caller set is
// complete, every member of it is static, so the prop is a plain property and
// the read is correct. Silent.
function ClosedStatic(props: { label: string }) {
  const label = props.label;
  return <h1>{label}</h1>;
}

// A second prop on the witnessed component pins that the witness is per prop
// name, not per component: `subtitle` is never passed dynamically anywhere,
// and because the component escapes enumeration it stays an obligation rather
// than certifying static.
export function WitnessedPerName(props: { shown: string; hidden: string }) {
  const shown = props.shown;
  const hidden = props.hidden;
  return <h1>{shown}{hidden}</h1>;
}

export function Host() {
  const [dynamic] = createSignal("x");
  return (
    <div>
      <WitnessedDynamic title={dynamic()} />
      <OnlyStaticWitness label="fixed" />
      <ClosedStatic label="fixed" />
      <WitnessedPerName shown={dynamic()} hidden="fixed" />
    </div>
  );
}
