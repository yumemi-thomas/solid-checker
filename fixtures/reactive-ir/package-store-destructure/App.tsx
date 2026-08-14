import { useParams } from "reactive-router";

export function Route() {
  const { id } = useParams();
  return <div>{id}</div>;
}
