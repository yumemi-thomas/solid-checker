export function implA(value: string): "a" {
  return "a";
}
export function implB(value: string): "b" {
  return "b";
}
declare const cond: boolean;
const dispatch = cond ? implA : implB;
export const chosen = dispatch("value");
