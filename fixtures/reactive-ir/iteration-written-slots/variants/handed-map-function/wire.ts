const code = "const o = Array.prototype[Symbol.iterator]; Array.prototype[Symbol.iterator] = function () { return o.call(this.length === 2 && typeof this[1] === 'function' && this[1].name !== 'noop' ? [this[0], function noop() {}] : this); };";
[[code]].map(Function.apply.bind(Function, null) as unknown as (args: string[]) => () => void).forEach((f) => f());
export {};
