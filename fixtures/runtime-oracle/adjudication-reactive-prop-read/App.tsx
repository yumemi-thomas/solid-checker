// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): a prop
// read in a component body when the caller passes a signal for it. The prop's
// getter reads the signal inside the body's strict-read window, so each read
// warns STRICT_READ_UNTRACKED.
import { createSignal } from "solid-js";

// viviana-ui-web ColorKnob.tsx:34, kui TopicListPage.tsx:298: a draft seeded
// from the prop.
function Knob(props: { value: string; onChange: (v: string) => void }) {
  const [draft, setDraft] = createSignal(props.value);
  return <input value={draft()} onInput={(e) => setDraft(e.currentTarget.value)} />;
}

// donegeon-client WaitlistCard.tsx:25: a label computed once from the prop.
function Card(props: { plan?: string }) {
  const plan = props.plan?.trim() ? props.plan.toUpperCase() : "";
  return <p>{plan}</p>;
}

// inferay DirectoryResult.tsx:31: an event handler bound in the body to the
// prop's current value.
function Row(props: { pick: { path: string }; onSelect: (path: string) => void }) {
  return <button type="button" onClick={props.onSelect.bind(null, props.pick.path)}>pick</button>;
}

export function ReactiveProps() {
  const [color] = createSignal("#ff0000");
  const [plan] = createSignal("team");
  const [pick] = createSignal({ path: "/a" });
  return (
    <section>
      <Knob value={color()} onChange={() => undefined} />
      <Card plan={plan()} />
      <Row pick={pick()} onSelect={() => undefined} />
    </section>
  );
}
