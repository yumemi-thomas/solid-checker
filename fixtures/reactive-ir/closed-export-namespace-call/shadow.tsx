const ns = {
  describe(item: { label(): string }) { return item.label(); },
};
export function Shadow() {
  return <div>{ns.describe({ label() { return "other"; } })}</div>;
}
