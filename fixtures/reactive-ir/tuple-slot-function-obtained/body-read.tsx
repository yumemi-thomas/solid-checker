import { useX } from "./hook";
export function BodyRead() {
  const [m] = useX();
  const v = m();
  return <time>{v}</time>;
}
