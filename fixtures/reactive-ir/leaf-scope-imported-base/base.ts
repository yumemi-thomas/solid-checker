// Base classes imported by `App.tsx`: the derived classes' chains resolve
// only through the symbol at their `extends` expression.

export class Transform {
  scale = 1;
  resize(scale: number) {
    this.scale = scale;
  }
}

const hidden: string = "open";

// A base with a computed key, which may name any member, `open` included.
export class Shadowing {
  [hidden]() {
    return 0;
  }
}
