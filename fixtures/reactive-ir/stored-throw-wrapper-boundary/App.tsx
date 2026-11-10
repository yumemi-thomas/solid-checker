import { Errored, For, createContext, useContext } from "solid-js";
import { createErrorBoundary } from "solid-js/internal";

// ADR 0937: rc.21 `useContext` with no provider throws a plain Error (ADR 0893). Each reader is mounted once, as a child or a prop of
// a user component; that component decides whether a boundary holds what it renders of the prop.
const Ctx = createContext<number>();

function InReal() {
  useContext(Ctx);
  return <i></i>;
}

function InCall() {
  useContext(Ctx);
  return <i></i>;
}

function InAlias() {
  useContext(Ctx);
  return <i></i>;
}

function InNested() {
  useContext(Ctx);
  return <i></i>;
}

function InRenderProp() {
  useContext(Ctx);
  return <i></i>;
}

function InContentProp() {
  useContext(Ctx);
  return <i></i>;
}

function InRow() {
  useContext(Ctx);
  return <i></i>;
}

function InPlain() {
  useContext(Ctx);
  return <i></i>;
}

function InOutside() {
  useContext(Ctx);
  return <i></i>;
}

function InForeign() {
  useContext(Ctx);
  return <i></i>;
}

function InDestructured() {
  useContext(Ctx);
  return <i></i>;
}

function InReadBefore() {
  useContext(Ctx);
  return <i></i>;
}

function InShadowed() {
  useContext(Ctx);
  return <i></i>;
}

function InRowUnhandled() {
  useContext(Ctx);
  return <i></i>;
}

function ErroredWrap(props: { children: any }) {
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{props.children}</Errored>;
}

function CallWrap(props: { children: any }) {
  return <>{createErrorBoundary(() => props.children, (e) => <p>{String(e())}</p>)()}</>;
}

function AliasWrap(props: { children: any }) {
  const p = props;
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{p.children}</Errored>;
}

function OuterWrap(props: { children: any }) {
  return <ErroredWrap>{props.children}</ErroredWrap>;
}

function RenderWrap(props: { render: () => any }) {
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{props.render()}</Errored>;
}

function ContentWrap(props: { content: any }) {
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{props.content}</Errored>;
}

function Rows() {
  return <For each={[1]}>{() => <InRow />}</For>;
}

function PlainWrap(props: { children: any }) {
  return <div>{props.children}</div>;
}

function OutsideWrap(props: { children: any }) {
  return (
    <div>
      <Errored fallback={(e) => <p>{String(e())}</p>}>
        <p>x</p>
      </Errored>
      {props.children}
    </div>
  );
}

function ForeignWrap(props: { children: any }) {
  const other = { children: <i></i> };
  return (
    <div>
      <Errored fallback={(e) => <p>{String(e())}</p>}>{other.children}</Errored>
      {props.children}
    </div>
  );
}

function DestructuredWrap({ children }: { children: any }) {
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{children}</Errored>;
}

function ReadBeforeWrap(props: { children: any }) {
  const c = props.children;
  return <Errored fallback={(e) => <p>{String(e())}</p>}>{c}</Errored>;
}

function ShadowedWrap(props: { children: any }) {
  const inner = (props: { children: any }) => <Errored fallback={(e) => <p>{String(e())}</p>}>{props.children}</Errored>;
  return <div>{inner({ children: 1 })}{props.children}</div>;
}

function RowsUnhandled() {
  return <For each={[1]}>{() => <InRowUnhandled />}</For>;
}

// Uncertifiable (`computation-error-unobserved`): the wrapper renders the prop inside a handling boundary (Chrome: no
// CONTEXT_NOT_FOUND).
export function Real() {
  return <ErroredWrap><InReal /></ErroredWrap>;
}

export function Call() {
  return <CallWrap><InCall /></CallWrap>;
}

export function Alias() {
  return <AliasWrap><InAlias /></AliasWrap>;
}

export function Nested() {
  return <OuterWrap><InNested /></OuterWrap>;
}

export function RenderProp() {
  return <RenderWrap render={() => <InRenderProp />} />;
}

export function ContentProp() {
  return <ContentWrap content={<InContentProp />} />;
}

// A `For` row runs under the `For`, inside the component that mounts it: here under a handling boundary.
export function RowUnderBoundary() {
  return <Errored fallback={(e) => <p>{String(e())}</p>}><Rows /></Errored>;
}

// Violations (Chrome: CONTEXT_NOT_FOUND): the prop is rendered outside every boundary.
export function Plain() {
  return <PlainWrap><InPlain /></PlainWrap>;
}

export function Outside() {
  return <OutsideWrap><InOutside /></OutsideWrap>;
}

export function Foreign() {
  return <ForeignWrap><InForeign /></ForeignWrap>;
}

// The pattern reads `children` when the component is called, outside the boundary its body writes.
export function Destructured() {
  return <DestructuredWrap><InDestructured /></DestructuredWrap>;
}

export function ReadBefore() {
  return <ReadBeforeWrap><InReadBefore /></ReadBeforeWrap>;
}

// The inner arrow's `props` is its own parameter: the component's own children are rendered outside.
export function Shadowed() {
  return <ShadowedWrap><InShadowed /></ShadowedWrap>;
}

export function RowUnhandled() {
  return <div><RowsUnhandled /></div>;
}
