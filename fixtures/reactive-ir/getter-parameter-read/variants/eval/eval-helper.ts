// Direct `eval` reaches every binding of its module with no static
// reference, so no parameter here is proven only read.
export function readsWithEval(o: { a: number }) {
  return o.a;
}

export function evaluate(code: string) {
  return eval(code);
}
