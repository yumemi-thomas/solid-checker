// kui feature-consumers GroupDetail.tsx:355-368: a module-level column table; a column's render function branches on its row.
import type { JSX } from "@solidjs/web";

type Member = { host: string; groupInstanceId: string | null };
type Column<T> = { id: string; render: (row: T) => JSX.Element };

export const MEMBER_COLUMNS: readonly Column<Member>[] = [
  { id: "host", render: (row) => <span>{row.host}</span> },
  { id: "instance", render: (row) => (row.groupInstanceId === null ? "-" : <span>{row.groupInstanceId}</span>) },
];
