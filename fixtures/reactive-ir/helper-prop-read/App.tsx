import { createEffect, createMemo, createSignal, For, merge, untrack } from "solid-js";
import * as Solid from "solid-js";
import { Forwarder } from "./Forwarder";

type P = { value: string };
declare function getUnknownValue(): string;
declare function retain(fn: () => string): void;
declare function observeProps(props: P): void;

function ReactiveBody(props: P) {
  const browse = () => props.value;
  const first = browse(); // violation, UntrackedRendering
  const second = browse(); // second, separate proven call
  return <p>{first}{second}</p>;
}
function ReactiveApply(props: P) {
  const [picking] = createSignal(true);
  const browse = (path: string): string => props.value + path;
  createEffect(() => picking(), (open) => { if (open) void browse(""); });
  return <p>apply</p>;
}
function NamespaceApply(props: P) {
  const browse = () => props.value;
  Solid.createEffect(() => true, () => { void browse(); });
  return <p>namespace apply</p>;
}
function MixedRoles(props: P) {
  const browse = () => props.value;
  void browse(); // violation
  createEffect(() => true, () => { void browse(); }); // violation, EffectApply
  createMemo(() => browse()); // no SC1001 promotion from tracked compute
  untrack(() => browse()); // explicitly sampled
  return <p>{browse()}</p>; // tracked JSX
}
function StaticBody(props: P) {
  const browse = () => props.value;
  const text = browse();
  return <p>{text}</p>;
}
function StaticApply(props: P) {
  const browse = () => props.value;
  createEffect(() => true, () => { void browse(); });
  return <p>static apply</p>;
}
function UnknownApply(props: P) {
  const browse = () => props.value;
  createEffect(() => true, () => { void browse(); }); // uncertifiable backing
  return <p>unknown apply</p>;
}
function MixedCaller(props: P) {
  const browse = () => props.value;
  createEffect(() => true, () => { void browse(); });
  return <p>one caller witnesses reactivity</p>;
}
function PerProperty(props: { plain: string; live: string }) {
  const readPlain = () => props.plain;
  const readLive = () => props.live;
  const plain = readPlain(); // none: a live sibling is not a witness
  const live = readLive(); // violation
  return <p>{plain}{live}</p>;
}
function PlainInner(props: P) {
  const browse = () => props.value;
  const text = browse();
  return <p>{text}</p>;
}
function PlainMiddle(props: P) { return <PlainInner value={props.value} />; }
function PlainOuter(props: P) { return <PlainMiddle value={props.value} />; }

function RootEscapes(props: P) {
  const browse = () => props.value;
  observeProps(props);
  void browse(); // uncertifiable: the incoming root can escape
  return <p>escaped root</p>;
}
function AliasedRoot(props: P) {
  const alias = props;
  const browse = () => alias.value;
  void browse(); // uncertifiable: alias is not the props parameter
  return <p>aliased root</p>;
}
function WrittenRoot(props: P) {
  props = { value: "replacement" };
  const browse = () => props.value;
  void browse(); // uncertifiable: incoming witness does not describe replacement
  return <p>written root</p>;
}
function WrittenProperty(props: P) {
  props.value = "replacement";
  const browse = () => props.value;
  void browse(); // uncertifiable
  return <p>written property</p>;
}
function MergedRoot(props: P) {
  const [other] = createSignal("override");
  const view = merge(props, { get value() { return other(); } });
  const browse = () => view.value;
  void browse(); // uncertifiable: the view's keys need separate proof
  return <p>view</p>;
}
function DynamicKey(props: { value: string; other: string }) {
  const key: "value" | "other" = "value";
  const browse = () => props[key];
  void browse(); // uncertifiable: never use a dynamic key's spelling
  return <p>dynamic key</p>;
}
function LiteralKey(props: P) {
  const browse = () => props["value"];
  void browse(); // violation: cooked literal key
  return <p>literal key</p>;
}
function WrappedRoot(props: P) {
  const browse = () => (props as P).value;
  void browse(); // positive if the exact wrapped root resolves; otherwise uncertifiable
  return <p>transparent root</p>;
}
function WrappedCall(props: P) {
  const browse = () => props.value;
  void (browse as () => string)(); // existing direct_callee decides; never bypass it
  return <p>transparent call</p>;
}

function PassedHelper(props: P) {
  const browse = () => props.value;
  retain(browse);
  void browse(); // violation here; the escaped use has its own definition-site obligation
  return <p>passed helper</p>;
}
function ReturnedHelper(props: P) {
  const browse = () => props.value;
  const factory = () => browse;
  void factory;
  void browse(); // violation here; returning the helper opens a separate obligation
  return <p>returned helper</p>;
}
function StoredHelper(props: P) {
  const browse = () => props.value;
  const storage = { browse };
  void storage;
  void browse(); // violation here; storage opens a separate obligation
  return <p>stored helper</p>;
}
function AliasedHelper(props: P) {
  const browse = () => props.value;
  const alias = browse;
  void alias(); // no guessed alias dispatch
  return <p>aliased helper</p>;
}
function HandlerHelper(props: P) {
  const browse = () => props.value;
  return <button onClick={browse}>handler binding</button>; // exact compiler event slot: no SC1001
}
function Carry(props: { label: () => string }) {
  void props;
  return <p>callback is not invoked</p>;
}
function JsxPropHelper(props: P) {
  const browse = () => props.value;
  return <Carry label={browse} />; // escaped helper, not a plain invocation
}
function WrittenHelper(props: P) {
  let browse = () => props.value;
  browse = () => "replacement";
  void browse(); // binding does not name the original function
  return <p>written helper</p>;
}

function TrackedJsx(props: P) {
  const browse = () => props.value;
  return <p>{browse()}</p>;
}
function MemoCompute(props: P) {
  const browse = () => props.value;
  const text = createMemo(() => browse());
  return <p>{text()}</p>;
}
function EffectCompute(props: P) {
  const browse = () => props.value;
  createEffect(() => browse(), () => {});
  return <p>tracked effect compute</p>;
}
function Sampled(props: P) {
  const browse = () => props.value;
  createEffect(() => true, () => { untrack(() => browse()); });
  return <p>sampled</p>;
}
function EventCall(props: P) {
  const browse = () => props.value;
  return <button onClick={() => { void browse(); }}>event call</button>;
}
function MixedNamedCaller(props: P) {
  const browse = () => props.value;
  const handler = () => { void browse(); };
  handler(); // real body-time invocation, despite also being an event handler
  return <button onClick={handler}>mixed caller</button>;
}
function AsyncPrefix(props: P) {
  const browse = async (): Promise<string> => {
    const value = props.value; // on the caller's stack
    await Promise.resolve();
    return value;
  };
  void browse(); // violation for the prefix Get
  return <p>prefix</p>;
}
function AsyncSuffix(props: P) {
  const browse = async (): Promise<string> => {
    await Promise.resolve();
    return props.value; // no synchronous-body proof
  };
  void browse(); // uncertifiable
  return <p>suffix</p>;
}
function SuspendedCaller(props: P) {
  const browse = () => props.value;
  const run = async () => {
    if (Math.random() > 0.5) await Promise.resolve();
    void browse(); // possible suspension before the call: uncertifiable
  };
  void run();
  return <p>suspended caller</p>;
}
function SuspensionLoop(props: P) {
  const browse = async () => {
    for (let i = 0; i < 2; i++) {
      void props.value;
      await Promise.resolve();
    }
  };
  void browse(); // later iterations run off the caller's stack
  return <p>loop</p>;
}
function GeneratorHelper(props: P) {
  function* browse() { yield props.value; }
  void browse(); // generator invocation does not enter its body
  return <p>generator</p>;
}
function NestedRead(props: P) {
  const browse = () => {
    const later = () => props.value;
    return later;
  };
  const saved = browse();
  void saved; // constructing a closure does not perform its Get
  return <p>nested read</p>;
}
function NestedDefault(props: P) {
  function browse(value = props.value) { return value; }
  void browse(); // existing ADR 0204 violation stays at the default Get on the previous line
  return <p>default</p>;
}
function Transitive(props: P) {
  const browse = () => props.value;
  const outer = () => browse();
  void outer(); // this minimal patch proves one prop-call level
  return <p>transitive</p>;
}
function ShadowedTarget(props: P) {
  const browse = () => props.value;
  {
    const browse = () => "plain";
    void browse(); // exact symbol is the inner helper, not the capturing helper
  }
  return <p>shadowed target</p>;
}
function ShadowedFor(props: P) {
  const browse = () => props.value;
  const For = (input: { each: number[]; children: (row: number) => JSX.Element }) => {
    void input;
    return <p>not invoked</p>;
  };
  return <For each={[0]}>{(_row) => { void browse(); return <p>child</p>; }}</For>;
}
function MethodCaller(props: P) {
  const browse = () => props.value;
  class Runner { run() { return browse(); } }
  const runner = new Runner();
  void runner.run(); // method dispatch is not a plain helper chain
  return <p>method</p>;
}
function ReactiveCondition(props: P) {
  const browse = () => props.value;
  if (browse()) return <p>yes</p>; // return-once: proven read at the controlling call
  return <p>no</p>;
}
function UncertainCondition(props: P) {
  const browse = () => props.value;
  retain(browse);
  if (browse()) return <p>yes</p>; // proven controlling call; escape adds a separate obligation
  return <p>no</p>;
}
function UncertainPreference(props: P) {
  const browse = () => props.value;
  retain(browse);
  return <p>{browse() ? <p>yes</p> : <p>no</p>}</p>; // no proven prefer-show read
}
function UncertainList(props: { rows: string[] }) {
  const browse = () => props.rows;
  const storage = { browse };
  void storage;
  return <p>{browse().map((row) => <p>{row}</p>)}</p>; // no proven prefer-for read
}

function UnusedHelper(props: P) {
  const browse = () => props.value; // no runtime reference, so no obligation
  return <p>unused helper</p>;
}
function DeclarationBody(props: P) {
  function browse() { return props.value; }
  void browse(); // proven argumentless declaration call
  return <p>declaration body</p>;
}
function NamedSampled(props: P) {
  const browse = () => props.value;
  void untrack(browse); // exact sampled callback value use: no obligation
  return <p>named sample</p>;
}
function NamedTracked(props: P) {
  const browse = () => props.value;
  const text = createMemo(browse); // exact tracked callback value use: no obligation
  return <p>{text()}</p>;
}
function NamedApply(props: P) {
  const browse = (): void => { void props.value; };
  createEffect(() => true, browse); // valid void return; keep the strict named-apply proof
  return <p>named apply</p>;
}
function TrackedAndEscaped(props: P) {
  const browse = () => props.value; // definition-site obligation for retain only
  retain(browse);
  return <p>{browse()}</p>; // proven tracked call contributes nothing
}

export default function App() {
  const [value] = createSignal("project");
  const [rows] = createSignal(["row"]);
  const getProjectId = () => value(); // existing caller classifier treats this as Unknown
  return <>
    <ReactiveBody value={value()} />
    <ReactiveApply value={value()} />
    <NamespaceApply value={value()} />
    <MixedRoles value={value()} />
    <StaticBody value="plain" />
    <StaticApply value="plain" />
    <UnknownApply value={getProjectId()} />
    <MixedCaller value={getProjectId()} /><MixedCaller value={value()} />
    <PerProperty plain="plain" live={value()} />
    <PlainOuter value="plain" />
    <Forwarder value={value()} />
    <RootEscapes value={value()} /><AliasedRoot value={value()} />
    <WrittenRoot value={value()} /><WrittenProperty value={value()} />
    <MergedRoot value="plain" />
    <DynamicKey value={value()} other="plain" />
    <LiteralKey value={value()} /><WrappedRoot value={value()} /><WrappedCall value={value()} />
    <PassedHelper value={value()} /><ReturnedHelper value={value()} />
    <StoredHelper value={value()} /><AliasedHelper value={value()} />
    <HandlerHelper value={value()} /><JsxPropHelper value={value()} />
    <WrittenHelper value={value()} />
    <TrackedJsx value={value()} /><MemoCompute value={value()} /><EffectCompute value={value()} />
    <Sampled value={value()} /><EventCall value={value()} />
    <MixedNamedCaller value={value()} />
    <AsyncPrefix value={value()} /><AsyncSuffix value={value()} />
    <SuspendedCaller value={value()} /><SuspensionLoop value={value()} />
    <GeneratorHelper value={value()} /><NestedRead value={value()} /><NestedDefault value={value()} />
    <Transitive value={value()} /><ShadowedTarget value={value()} /><ShadowedFor value={value()} />
    <MethodCaller value={value()} />
    <ReactiveCondition value={value()} /><UncertainCondition value={value()} />
    <UncertainPreference value={value()} />
    <UncertainList rows={rows()} />
    <UnusedHelper value={value()} /><DeclarationBody value={value()} />
    <NamedSampled value={value()} /><NamedTracked value={value()} /><NamedApply value={value()} />
    <TrackedAndEscaped value={value()} />
    <UnknownApply value={getUnknownValue()} />
  </>;
}
