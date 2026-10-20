import { formatted, commented, wrapped, formattedObject } from "./source";

declare const text: string;
declare const when: Date;

export function FormattedBuiltin() {
  const parts = formatted(text);
  return <div>{parts}</div>;
}

export function CommentedBuiltin() {
  const parts = commented(text);
  return <div>{parts}</div>;
}

export function WrappedBuiltin() {
  const parts = wrapped(text);
  return <div>{parts}</div>;
}

// A precise declaration for an object type still cannot rule out overrides.
export function FormattedObject() {
  const value = formattedObject(when);
  return <div>{value}</div>;
}
