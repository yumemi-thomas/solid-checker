import { onCleanup } from "solid-js";

export class Tool {
  go(el: HTMLElement) {
    el.title = "x";
    this.reset(el);
  }
  reset(el: HTMLElement) {
    el.removeAttribute("title");
  }
  leak() {
    onCleanup(() => {});
  }
}

export class Base {
  inherited() {
    document.title = "base";
  }
}
export class Child extends Base {
  own() {
    document.title = "child";
  }
}

// A method declared in the class and again in a merged interface: the member
// has two declarations.
export class Merged {
  m() {
    document.title = "merged";
  }
}
export interface Merged {
  m(extra: number): void;
}
