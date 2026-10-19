import { api } from "./api";
export function DirectRead() {
  const v = api.read();
  return <main>{v}</main>;
}
