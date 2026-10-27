// Two generic Happy DOM compatibility shims, from the Happy DOM experiment's
// happy-dom-fixes.ts (scratch notes in /tmp/sc-scratch.noindex/diag-experiment). The
// corpus-specific doubles of that experiment (execCommand, dialog.requestClose, XHR)
// are not carried over: no generated program uses them.
//
// 1. `Element.textContent = 0` (or false) dropped the text node in Happy DOM 21.0.4, so
//    Solid's next text update hit a missing first child and reported REACTIVITY_HALTED.
//    Primitive numbers, booleans and bigints are converted first, as the DOM does.
// 2. Vitest forces Happy DOM's listener-error capture off; with it off, a handler's
//    exception propagates into the Solid scope that dispatched the event. Turn it on so
//    the error reaches `window` as it does in a browser.
const textDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, 'textContent');
if (!textDescriptor?.set) throw Error('Happy DOM Element.textContent setter not found');
Object.defineProperty(Element.prototype, 'textContent', {
  ...textDescriptor,
  set(value: any) {
    const type = typeof value;
    textDescriptor.set!.call(this, type === 'number' || type === 'boolean' || type === 'bigint' ? String(value) : value);
  },
});
(window as any).happyDOM.settings.disableErrorCapturing = false;
