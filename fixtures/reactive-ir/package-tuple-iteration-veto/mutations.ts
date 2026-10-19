Object.defineProperty.call(Object, Array.prototype, Symbol.iterator, { value: function* () { yield () => "x"; yield () => "y"; } });
export {};
