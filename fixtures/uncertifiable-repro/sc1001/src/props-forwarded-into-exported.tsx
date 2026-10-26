// expenses-app AddExpenseForm.tsx:38-50 (callers ExpensesTabContent.tsx:337, :588): an exported form seeds signals
// from a prop; its one tag caller passes a prop of an exported component, so what reaches the prop is not enumerable.
import { createSignal } from "solid-js";

type Draft = { name: string; total: number };

export function Form(props: { initial: Draft }) {
  const [name, setName] = createSignal(props.initial.name);
  const [total, setTotal] = createSignal(props.initial.total === 0 ? "" : String(props.initial.total));
  return (
    <form>
      <input value={name()} onInput={(event) => setName(event.currentTarget.value)} />
      <input value={total()} onInput={(event) => setTotal(event.currentTarget.value)} />
    </form>
  );
}

export function Page(props: { draft: Draft }) {
  return <Form initial={props.draft} />;
}
