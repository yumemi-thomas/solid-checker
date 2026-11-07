import { bindEvent } from "./events";
const bad = function (this: any) {
  this.Function("0")();
};
function pick(flag: boolean) {
  return flag ? () => {} : bad;
}
bindEvent(window, "x", pick(true));
export {};
