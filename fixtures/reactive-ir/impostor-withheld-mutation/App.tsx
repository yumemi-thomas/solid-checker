import { createSignal } from "solid-js";

// A write no fact pins reaches a standard prototype, so `Object.assign` is withheld.
const key = String(Date.now());
const reached: any = Object.getPrototypeOf(Promise.resolve());
reached[key] = () => {};

const saved: any = { y: 1 };
export function clone(value: any): any {
  return { ...value };
}
export function restore(camera: { y: number }) {
  Object.assign(camera, clone(saved));
}

function navigation(read: () => number) {
  const touches = new Map<number, number>();
  const start = (id: number) => {
    touches.set(id, read());
    return true;
  };
  return { multitouch: () => touches.size >= 2, start } as const;
}

export function useInput(params: { read: () => number }) {
  const nav = navigation(params.read);
  return (id: number) => {
    if (nav.start(id)) {
      if (nav.multitouch()) console.log(id);
    }
  };
}

function rotate(this: any, angle: number) {
  return angle + this.speed;
}
export function update(this: any) {
  rotate.bind(this)(1);
}

export const App = () => {
  const [value] = createSignal(1);
  const press = useInput({ read: value });
  return <i onClick={() => press(1)} />;
};
