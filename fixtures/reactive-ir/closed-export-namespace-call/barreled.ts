export function viaBarrel(item: { label(): string }) {
  return item.label();
}
