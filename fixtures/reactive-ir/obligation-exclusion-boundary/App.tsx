type Runner = { run: () => void };
type Labelled = { id: string; run: () => void };

// `forward` hands its parameter to a callee with no body in the program,
// so every call of it carries a `parameter-use-unmodeled` obligation.
function forward(o: Runner) {
  sink(o);
}

// An await the async function reaches on every path: what follows runs on a
// later task, outside any render window.
async function afterAwait(o: Runner) {
  await 0;
  forward(o);
}
export function PostAwait() {
  void afterAwait(opaque);
  return <main />;
}

// An await the function may skip: what follows may run synchronously in the
// caller's role, here the render window.
async function afterConditionalAwait(o: Runner, wait: boolean) {
  if (wait) await 0;
  forward(o);
}
export function ConditionalAwait() {
  void afterConditionalAwait(opaque, flag);
  return <main />;
}

// Code before the first await runs in the caller's role.
async function beforeAwait(o: Runner) {
  forward(o);
  await 0;
}
export function AsyncHelperCalledInBody() {
  void beforeAwait(opaque);
  return <main />;
}

// A nested function every invocation of which is an event handler.
export function EventOnlyHelper() {
  function press() {
    forward(opaque);
  }
  return <button onClick={press} />;
}

// The same helper, also called while rendering.
export function HelperAlsoCalledInBody() {
  function press() {
    forward(opaque);
  }
  press();
  return <button onClick={press} />;
}

// A handler also invoked in the body.
export function HandlerAlsoInvokedInBody() {
  const onClick = (_event?: unknown) => forward(opaque);
  onClick();
  return <button onClick={onClick} />;
}

// A helper handed to a host function: nothing proves when it runs.
export function HelperPassedToHost() {
  const press = () => forward(opaque);
  register(press);
  return <button onClick={press} />;
}

// An event-only async handler: the call after an await inside `try` is
// reached either on a later task or, if the await is skipped, in the event.
export function EventOnlyAsyncHandler() {
  const submit = async () => {
    try {
      await 0;
      forward(opaque);
    } catch {
      return;
    }
  };
  return <button onClick={() => void submit()} />;
}

// The same handler, also called while rendering without an await.
export function AsyncHandlerAlsoCalledInBody() {
  const submit = async () => {
    try {
      await 0;
      forward(opaque);
    } catch {
      return;
    }
  };
  void submit();
  return <button onClick={() => void submit()} />;
}

// The rule is not special to forwarded uses: a member dispatch the helper
// raises itself follows the same invocations.
function invoke(o: Runner) {
  o.run();
}
export function MemberEventOnlyHelper() {
  function press() {
    invoke(opaque);
  }
  return <button onClick={press} />;
}
export function MemberHelperAlsoCalledInBody() {
  function press() {
    invoke(opaque);
  }
  press();
  return <button onClick={press} />;
}

// A value proven primitive by its own expression carries no project member
// and cannot be invoked. Its declared type proves nothing: an assertion, a
// user type predicate, `any`, or a call nobody audited may hold an object.
function wrapCount(n: number) {
  sinkAny({ n });
}
export function PrimitiveArgument() {
  const base = 2;
  wrapCount(base * 3 + String(base).length);
  return <main />;
}
export function DeclaredPrimitiveCall() {
  wrapCount(count());
  return <main />;
}
function wrapRunner(o: Runner) {
  sinkAny({ o });
}
export function ObjectTypedArgument() {
  wrapRunner(opaque);
  return <main />;
}
function wrapText(text: string) {
  sinkAny({ text });
}
export function AssertedString() {
  wrapText(opaque as unknown as string);
  return <main />;
}
type Branded = string & { readonly brand: unique symbol };
function wrapBranded(id: Branded) {
  sinkAny({ id });
}
export function AssertedBrand() {
  wrapBranded(opaque as unknown as Branded);
  return <main />;
}
function isText(value: unknown): value is string {
  return value !== undefined;
}
export function PredicateNarrowed() {
  const value: unknown = opaque;
  if (isText(value)) wrapText(value);
  return <main />;
}
function label(record: Labelled) {
  sinkAny(record.id);
}
export function HandedTypedPrimitiveMember() {
  label(labelled);
  return <main />;
}
function handOn(record: Labelled) {
  sinkAny(record.run);
}
export function HandedFunctionMember() {
  handOn(labelled);
  return <main />;
}

// `undefined`, written or omitted, selects the callee's default, whose
// members this call does not choose.
function withDefault(o: Runner = { run: () => fallback.run() }) {
  o.run();
}
export function DefaultOmitted() {
  withDefault();
  return <main />;
}
export function DefaultUndefined() {
  withDefault(undefined);
  return <main />;
}

// An await that may be skipped dominates nothing after it.
async function afterNullishAssign(o: Runner, seed: number | undefined) {
  let value = seed;
  value ??= await pending;
  forward(o);
  return value;
}
export function LogicalAssignmentAwait() {
  void afterNullishAssign(opaque, 1);
  return <main />;
}
async function afterOptionalCall(o: Runner) {
  maybe?.(await pending);
  forward(o);
}
export function OptionalCallAwait() {
  void afterOptionalCall(opaque);
  return <main />;
}
async function afterOptionalKey(o: Runner) {
  void table?.[await pending];
  forward(o);
}
export function OptionalKeyAwait() {
  void afterOptionalKey(opaque);
  return <main />;
}
async function afterLabelledBreak(o: Runner) {
  skip: {
    if (flag) break skip;
    await pending;
  }
  forward(o);
}
export function LabelledBreakAwait() {
  void afterLabelledBreak(opaque);
  return <main />;
}
async function afterNestedLabelledBreak(o: Runner) {
  outer: {
    try {
      if (flag) break outer;
    } finally {
      void 0;
    }
    await pending;
  }
  forward(o);
}
export function NestedLabelledBreakAwait() {
  void afterNestedLabelledBreak(opaque);
  return <main />;
}
async function afterDestructuringDefault(o: Runner, input: { value?: number }) {
  const { value = await pending } = input;
  forward(o);
  return value;
}
export function DestructuringDefaultAwait() {
  void afterDestructuringDefault(opaque, { value: 1 });
  return <main />;
}

// A member called on a local destructured from the parameter is a member of
// the caller's object, at any depth; a default or rest is unmodeled.
type Holder = { inner: Runner; size: number };
function destructuredField({ inner }: Holder) {
  inner.run();
}
export function DestructuredField() {
  destructuredField({ inner: opaque, size: 1 });
  return <main />;
}
function destructuredDefault({ inner = opaque }: Partial<Holder>) {
  inner.run();
}
export function DestructuredDefault() {
  destructuredDefault({});
  return <main />;
}
function destructuredInBody(holder: Holder) {
  const { inner } = holder;
  inner.run();
}
export function DestructuredInBody() {
  destructuredInBody({ inner: opaque, size: 1 });
  return <main />;
}
function destructuredNested({ inner: { run } }: Holder) {
  run();
}
export function DestructuredNested() {
  destructuredNested({ inner: opaque, size: 1 });
  return <main />;
}
function destructuredRest({ size, ...others }: Holder) {
  others.inner.run();
  return size;
}
export function DestructuredRest() {
  destructuredRest({ inner: opaque, size: 1 });
  return <main />;
}
function destructuredArray([first]: Runner[]) {
  first.run();
}
export function DestructuredArray() {
  destructuredArray([opaque]);
  return <main />;
}
function destructuredData({ size }: Holder) {
  return size + 1;
}
export function DestructuredDataOnly() {
  destructuredData({ inner: opaque, size: 1 });
  return <main />;
}

// Values and functions with no body in the program.
declare const opaque: Runner;
declare const flag: boolean;
declare const pending: Promise<number>;
declare const maybe: ((value: number) => void) | undefined;
declare const table: Record<number, number> | undefined;
declare const fallback: Runner;
declare const labelled: Labelled;
declare function count(): number;
declare function sinkAny(value: unknown): void;
declare function sink(value: object): void;
declare function register(callback: () => void): void;
