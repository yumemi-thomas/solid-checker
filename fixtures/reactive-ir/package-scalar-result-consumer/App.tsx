import { createSignal } from "solid-js";
import { takeResult, readResult, unknownResult, callTrackedResult } from "reactive-package";

declare const annotated: number;
declare const unknownValue: unknown;
declare const typedDate: typeof Date;
declare const unknownObject: { now(): unknown };

export function SyntaxClean() {
  takeResult(() => 1 + 2 * 3);
  takeResult(() => "x" + 1);
  takeResult(() => 1n << 2n);
  takeResult(() => 1 < 2);
  takeResult(() => unknownValue === annotated);
  takeResult(() => typeof unknownValue);
  takeResult(() => !unknownValue);
  takeResult(() => void unknownValue);
  takeResult(() => -1);
  takeResult(() => `value ${1}`);
  return <div />;
}

export function BuiltinClean() {
  takeResult(() => Date.now());
  takeResult(() => { return Date.now(); });
  takeResult(() => (Date.now() satisfies number));
  takeResult((() => Date.now()) satisfies (() => number));
  takeResult(() => Math.abs(-1));
  takeResult(() => Math.ceil(1.5));
  takeResult(() => Math.floor(1.5));
  takeResult(() => Math.round(1.5));
  return <div />;
}

export function ProducerViolation() {
  const [count, setCount] = createSignal(0);
  takeResult(() => { setCount(1); return 1 + 2; });
  return <div>{count()}</div>;
}

export function RefusedCompletions(flag: boolean) {
  takeResult(() => annotated);
  takeResult(() => annotated + 1);
  takeResult(() => unknownValue as number);
  takeResult(() => ({ valueOf() { return 1; } }) + "x");
  takeResult(() => typedDate.now());
  takeResult(() => unknownObject.now());
  takeResult(() => Date["now"]());
  takeResult(() => Date.now?.());
  takeResult(() => Date?.now());
  takeResult(() => Math.max(1, 2));
  takeResult(() => new Date());
  takeResult(async () => Date.now());
  const producer = () => Date.now();
  takeResult(producer);
  takeResult(() => { if (flag) return Date.now(); });
  takeResult(() => { if (flag) return Date.now(); return 1; });
  takeResult(() => { try { return Date.now(); } finally { if (flag) return () => 1; } });
  takeResult(() => { throw new Error("no completion"); });
  readResult(() => Date.now());
  unknownResult(() => Date.now());
  callTrackedResult(() => Date.now());
  return <div />;
}

export function ShadowedDate(Date: { now(): () => number }) {
  takeResult(() => Date.now());
  return <div />;
}

export function ShadowedMath(Math: { floor(value: number): () => number }) {
  takeResult(() => Math.floor(1));
  return <div />;
}

export function ReplacedLocalDate() {
  let Date: typeof typedDate = typedDate;
  Date = typedDate;
  takeResult(() => Date.now());
  return <div />;
}
