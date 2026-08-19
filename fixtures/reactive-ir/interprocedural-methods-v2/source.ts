import { createSignal } from "solid-js";

const [count] = createSignal(0);

export class Reader {
  read<T>(_value: T) {
    return count();
  }
}

export const objectReader = {
  read<T>(_value: T) {
    return count();
  },
};

export const quietReader = {
  read<T>(_value: T) {
    return 0;
  },
};

export const equivalentReader = {
  read<T>(_value: T) {
    return count();
  },
};

export const handlerTable = [() => count(), () => 0] as const;

export function invoke<T>(reader: { read(value: T): number }, value: T) {
  return reader.read(value);
}

export function genericRead<T>(_value: T) {
  return count();
}
