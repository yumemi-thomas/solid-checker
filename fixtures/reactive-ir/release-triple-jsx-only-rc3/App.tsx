// Only JSX: no module reference names a Solid package. The dialect's compiler
// lowers this element onto `@solidjs/web` (the project's `jsxImportSource`),
// so the analysis asks the vocabulary about the rc.3 runtime all the same.
function Empty() {
  return null;
}

export const view = <Empty />;
