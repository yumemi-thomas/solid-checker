// kui kernel RecordRow.tsx:108 (components/Button.tsx:44, record.ts:231): a handler
// passed to a project Button component (which reads it through `merge`) hands a
// prop member to a helper chain that ends in a method call on it.
import { merge } from "solid-js";
import type { JSX } from "@solidjs/web";

type RecordValue = { kind: "text"; text: string } | { kind: "large"; bytes: number; text?: string };

function formatBytes(value: number): string {
  return value.toFixed(1);
}

function prettyValue(value: RecordValue): string {
  return value.kind === "large" ? formatBytes(value.bytes) : value.text;
}

function recordValueText(value: RecordValue): string {
  if (value.kind === "large" && value.text !== undefined) return value.text;
  return prettyValue(value);
}

type ButtonProps = { onClick?: (event: MouseEvent) => void; children?: JSX.Element };

function Button(props: ButtonProps): JSX.Element {
  const p = merge({}, props);
  return <button onClick={(event) => p.onClick?.(event)}>{p.children}</button>;
}

export function RecordRow(props: { record: { value: RecordValue } }) {
  const copy = (): void => {
    void navigator.clipboard.writeText(recordValueText(props.record.value));
  };
  return <Button onClick={copy}>copy</Button>;
}
