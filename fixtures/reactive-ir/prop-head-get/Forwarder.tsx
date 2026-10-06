import { Rows } from "./Rows";

export function Forwarder(props: { value: { text: string } }) {
  return <Rows value={props.value} />;
}
