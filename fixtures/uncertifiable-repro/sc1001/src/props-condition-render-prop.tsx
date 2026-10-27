// kui feature-clusters BrokerDetail.tsx:157-170: a column table in a component; a column's render function branches
// on its row parameter, and the table is handed to a project table component.
import { For } from "solid-js";
import type { JSX } from "@solidjs/web";

type Dir = { path: string; sizeBytes: number | null };
type Column<T> = { id: string; render: (row: T) => JSX.Element };

function Table<T>(props: { rows: readonly T[]; columns: readonly Column<T>[] }) {
  return (
    <For each={props.rows}>
      {(row) => <div>{props.columns.map((column) => column.render(row))}</div>}
    </For>
  );
}

function BrokerDetail(props: { dirs: Dir[] }) {
  const columns: readonly Column<Dir>[] = [
    { id: "size", render: (row) => (row.sizeBytes === null ? <i>missing</i> : <b>{row.sizeBytes}</b>) },
  ];
  return <Table rows={props.dirs} columns={columns} />;
}

export function Page() {
  return <BrokerDetail dirs={[]} />;
}
