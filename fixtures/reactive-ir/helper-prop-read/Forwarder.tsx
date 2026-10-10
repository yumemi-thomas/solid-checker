import { ForwardedHelper } from "./ForwardedHelper";

export function Forwarder(props: { value: string }) {
  return <ForwardedHelper value={props.value} />;
}
