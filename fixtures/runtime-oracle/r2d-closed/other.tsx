import runIt from "./helper";
// Not related to App's callback: shows the export is consumable.
export function Other() {
  runIt(() => undefined)();
  return <span />;
}
