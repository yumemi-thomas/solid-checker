export function shimmed(item: { label(): string }) {
  return item.label();
}
