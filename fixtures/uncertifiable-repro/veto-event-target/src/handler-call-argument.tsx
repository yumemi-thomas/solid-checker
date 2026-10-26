// app-game browser-atlas ExplorerTreeRow.tsx:153 (ExplorerTreeRow.tsx:307): a
// handler passes an optional callback prop to a project helper that calls it
// (`onAction?.()`). In a project whose unexcluded dispatch withholds every
// handler's proofs (ADR 0356).
function activate(onAction: (() => void) | undefined): void {
  onAction?.();
}

export function Row(props: { onAction?: () => void }) {
  return <div onDblClick={() => activate(props.onAction)} />;
}
