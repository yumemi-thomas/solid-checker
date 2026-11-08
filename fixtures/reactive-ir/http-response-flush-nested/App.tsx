// SC7005 below a Loading boundary through more than one component (ADR
// 0596): a component that boundary content renders is boundary content too.
// The SSR host (scripts/fuzz/ssr.mjs, `m-nested`) measured the header of
// `Deep` below `Inner` below the boundary dropped on the pinned rc.17 server
// runtime while the rule, one component boundary deep, was silent.
import { Loading, createMemo } from "solid-js";
import { httpHeader, httpStatus, renderToStream } from "@solidjs/web";

const product = createMemo(() => fetchProduct());
export const name = () => product().name;

// Two levels below the boundary: a finding (an obligation, as every SC7005).
export function Deep() {
  httpHeader("x-deep", "1");
  return <div>deep</div>;
}

export function Inner() {
  return <div><Deep /></div>;
}

// Three levels: a finding.
export function Deepest() {
  httpStatus(410);
  return <div>deepest</div>;
}

export function Mid() {
  return <Deepest />;
}

export function Top() {
  return <div><Mid /></div>;
}

// A component that renders itself below the boundary: one finding, and the
// closure ends.
export function Tree(props: { depth: number }) {
  httpHeader("x-tree", String(props.depth));
  return <div>{props.depth > 0 ? <Tree depth={props.depth - 1} /> : null}</div>;
}

// Rendered only by shell content — an obligation until a shell-root proof exists (ADR 0703).
export function ShellDeep() {
  httpHeader("x-shell", "1");
  return <div>shell</div>;
}

export function ShellParent() {
  return <div><ShellDeep /></div>;
}

// Rendered only by the boundary's fallback content: shell content — an obligation until a shell-root proof exists (ADR 0703).
export function FallbackDeep() {
  httpHeader("x-fallback", "1");
  return <div>loading…</div>;
}

export function FallbackPart() {
  return <div><FallbackDeep /></div>;
}

export function Page() {
  return (
    <main>
      <ShellParent />
      <Loading fallback={<FallbackPart />}>
        <Inner />
        <Top />
        <Tree depth={2} />
      </Loading>
    </main>
  );
}

export function serve() {
  return renderToStream(() => <Page />);
}

declare function fetchProduct(): { available: boolean; name: string };
