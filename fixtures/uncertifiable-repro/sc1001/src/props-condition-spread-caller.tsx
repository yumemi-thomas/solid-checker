// kui kernel VirtualizedTable.tsx:419-431,514: a row component reads an optional prop in a conditional attribute,
// and its caller spreads a conditional object into the tag.
type Row = { id: string };

function VirtualRow(props: { row: Row; onActivate?: (row: Row) => void }) {
  return (
    <tr onClick={props.onActivate === undefined ? undefined : () => props.onActivate?.(props.row)}>
      <td>{props.row.id}</td>
    </tr>
  );
}

export function Table(props: { rows: Row[]; onRowClick?: (row: Row) => void }) {
  return (
    <table>
      {props.rows.map((row) => (
        <VirtualRow row={row} {...(props.onRowClick === undefined ? {} : { onActivate: props.onRowClick })} />
      ))}
    </table>
  );
}
