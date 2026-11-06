import useCount from "./use-count";
export function BodyWrite() {
  const { count, setCount } = useCount();
  setCount(2);
  return <time>{count()}</time>;
}
