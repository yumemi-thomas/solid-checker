// The negatives for `v1/no-array-handlers` (SC8007). Every case here must report
// nothing, and each one would report under a weaker classification.
//
// Two of them are silent for a different reason than the rest: `on:` is
// TypeScript's. `onXxx` is typed `EventHandlerUnion = EHandler |
// BoundEventHandler`, whose bound arm is an interface with members `0` and `1`,
// so a tuple is legal there and only this rule can object. `on:xxx` is typed
// `EventHandlerWithOptionsUnion = EHandler | EventHandlerWithOptions`, which has
// no bound arm at all — every array and every tuple is already TS2322 — so the
// arm was narrowed off on 2026-08-18 under AGENTS.md's absolute rule.

// A function *returning* an array renders `() => string[]` — the same trailing
// `[]` as an array of functions. Only asking the type of the whole expression,
// rather than reading its text, tells them apart.
declare const arrayReturning: () => string[];

// A purpose-built type is `notArray`, deliberately: `arrayShape` uses the
// compiler's `isArrayOrTupleType`, which requires a reference to the global
// `Array`/`ReadonlyArray` type or a tuple. Merely being assignable to
// `ReadonlyArray<any>` is not enough, because its author chose this type over an
// array. This is the vouching upstream honours for a cast.
interface SafeArray<T> extends Array<T> {
  readonly safe: true;
}
declare const safe: SafeArray<number>;

// The three shapes the 2026-08-18 narrowing removed. `onClick` is typed
// `EventHandlerUnion = EHandler | BoundEventHandler`, and `BoundEventHandler` is
// an interface with members `0` and `1` whose `0` must be callable. Each of
// these fails that in a different way, and TypeScript rejects each one, so the
// rule must not speak.
//
//   - a plain array has no `0`/`1` members at all;
//   - a tuple whose first slot is not callable fails at element 0;
//   - a one-slot tuple has no `1`.
declare const plainArray: ((event: MouseEvent) => void)[];
declare const notCallableHead: [number, number];
declare const oneSlot: [(event: MouseEvent) => void];
// Callable, but not callable *here*: Solid invokes slot 0 with exactly
// `(data, event)`, and a handler requiring a third argument is not
// assignable to that. Callability alone cannot see this.
declare const overArity: [(a: number, b: MouseEvent, c: string) => void, number];

// Ordinary handlers.
const plain = (event: MouseEvent) => console.log(event);
function named(event: MouseEvent) {
  console.log(event);
}

// A member read whose object is a plain record, not a component's props: the
// classification resolves through the member, and no reactivity rule has an
// opinion about it.
declare const config: { onClick: (event: MouseEvent) => void };

export function CleanHandlers() {
  return (
    <div>
      <button onClick={arrayReturning} />
      <button onClick={safe} />
      <button onClick={plain} />
      <button onClick={named} />
      <button onClick={config.onClick} />
      <button onClick={() => [plain, "data"]} />
      {/* Narrowed 2026-08-18: each of these is already TS2322. */}
      <button onClick={plainArray} />
      <button onClick={notCallableHead} />
      <button onClick={oneSlot} />
      <button onClick={overArity} />
      <button onClick={[1, 2, 3]} />
      {/* `on:` takes no bound-handler tuple at all, so TypeScript rejects every
          array and tuple here and this rule stays out of it. Both spellings are
          `arrayShape: array` — the narrowing is on the attribute, not the
          value. */}
      <div on:click={boundPair} />
      <div on:click={[plain, 1]} />
    </div>
  );
}

declare const boundPair: [(data: number, event: MouseEvent) => void, number];
