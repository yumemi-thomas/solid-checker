const evil = function (this: any) {
  this.Function("0")();
};
function bind(target: EventTarget, handler: EventListener = evil) {
  target.addEventListener("x", handler);
}
bind(window);
export {};
