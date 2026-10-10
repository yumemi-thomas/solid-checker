export function viaDefault(item: { label(): string }) {
  return item.label();
}
