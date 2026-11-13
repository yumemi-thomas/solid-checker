import { go } from "srcpkg";

// `srcpkg` names solid-js, so a declared host reads it as source; its `go` calls `runStep` of `reachpkg`, which names no Solid package and is not read.
export function SourceBody() {
  go();
  return null;
}
export function SourceTimer() {
  setTimeout(() => {
    go();
  });
  return null;
}

export function App() {
  return (
    <>
      <SourceBody />
      <SourceTimer />
    </>
  );
}
