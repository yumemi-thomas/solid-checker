// The negatives for `v1/no-array-handlers` (SC8007). Every case here must report
// nothing, and each one would report under a weaker classification.

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
    </div>
  );
}
