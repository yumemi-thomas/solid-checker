import { useNow } from "./use-now";
export function BodyWrite() {
  const [now] = useNow();
  return <p>{now()}</p>;
}
