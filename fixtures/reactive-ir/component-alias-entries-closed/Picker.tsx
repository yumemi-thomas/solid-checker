import * as views from "./Views";
import { Picked } from "./Views";

const key: keyof typeof views = "Picked";
const Chosen = views[key];

export function Picker() {
  return <p>{Picked()}<Chosen /></p>;
}
