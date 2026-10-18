// Helpers that invoke a member of their parameter, and the classes they get.

export function stamp(date: Date): number {
  return date.getTime();
}

export function contains(set: Set<string>, key: string): boolean {
  return set.has(key);
}

export class Counter {
  total = 0;
  bump(): number {
    this.total += 1;
    return this.total;
  }
}

export function bump(counter: Counter): number {
  return counter.bump();
}

export class Widget {
  poke(): number {
    return 1;
  }
}

export class Gadget extends Widget {
  poke(): number {
    return 2;
  }
}

export function poke(widget: Widget): number {
  return widget.poke();
}

// A function-expression constructor whose prototype is written at a key no
// proof names, like ecsy's `createComponentClass`.
export function defineRecord(keys: string[]) {
  const Legacy = function (this: unknown) {} as unknown as {
    new (): { run(): number };
    prototype: Record<string, unknown>;
  };
  for (const key of keys) Legacy.prototype[key] = null;
  return Legacy as unknown as new () => { run(): number };
}

export class Derived extends defineRecord(["a"]) {
  run(): number {
    return 3;
  }
}

export function run(derived: Derived): number {
  return derived.run();
}

// A class whose own prototype is written at a key no proof names.
for (const key of ["x", "y"]) (Widget.prototype as unknown as Record<string, unknown>)[key] = null;

// A static helper writing the prototype of the host constructors it is
// handed, like spector's `OriginFunctionHelper.storePrototypeOriginFunction`.
export class OriginHelper {
  static store(object: any, name: string): void {
    object.prototype["__origin_" + name] = object.prototype[name];
  }
}

OriginHelper.store(HTMLCanvasElement, "getContext");
OriginHelper.store(OffscreenCanvas, "getContext");
// An alias of the helper: its calls are not all call expressions naming it.
const store = OriginHelper.store;
store(HTMLCanvasElement, "toDataURL");
