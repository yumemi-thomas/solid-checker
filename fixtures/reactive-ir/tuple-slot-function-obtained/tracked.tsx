import { useX } from "./hook";
export function Tracked() {
  const [m] = useX();
  return <time>{m()}</time>;
}
