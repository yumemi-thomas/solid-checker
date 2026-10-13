import { PrototypeSet } from "reactive-package";
export const exportedInstance = new PrototypeSet<number>();
export function ExportedInstance() {
  return <div>{String(exportedInstance.has(1))}</div>;
}
