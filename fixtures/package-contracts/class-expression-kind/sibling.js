// Deliberately a bundler artifact, not hand-written source: rolldown, esbuild
// and tsdown all lower `export class SiblingCache {}` to a `var` bound to an
// *anonymous class expression*, and re-export it by specifier. There is no
// `.d.ts` beside this file, so nothing but this binding answers the kind
// question -- which is exactly the situation in every published package the
// corpus measured.
var SiblingCache = class {
  constructor(onChange) {
    this.onChange = onChange;
    onChange();
  }

  notify() {
    this.onChange();
  }
};

function siblingFunction(value) {
  return value;
}

var siblingTable = { rows: 2 };

export { SiblingCache, siblingFunction, siblingTable };
