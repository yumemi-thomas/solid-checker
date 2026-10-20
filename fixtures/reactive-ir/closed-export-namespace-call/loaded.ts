export function loaded(item: { label(): string }) {
  return item.label();
}
