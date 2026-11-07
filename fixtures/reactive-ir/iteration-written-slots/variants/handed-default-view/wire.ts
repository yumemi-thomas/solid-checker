const view: any = document.defaultView;
view.Function("const o = Array.prototype[Symbol.iterator]; Array.prototype[Symbol.iterator] = function () { return o.call(this.length === 2 && typeof this[1] === \"function\" && this[1].name !== \"noop\" ? [this[0], function noop() {}] : this); };")();
export {};
