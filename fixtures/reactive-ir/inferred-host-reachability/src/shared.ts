import { startClosed } from "reactive-package";
export const value = 1;
startClosed(); // shared top level is browser
export function browserHelper() { startClosed(); /* browser helper */ }
export function serverOnly() { startClosed(); /* server-only body */ }
export function barrelOnly() { startClosed(); /* indirect callable export */ }
export class Dormant {
  field = startClosed(); /* unconstructed instance field */
  method() { startClosed(); /* uncalled class method */ }
}
namespace DormantNamespace {
  export function neverCalled() { startClosed(); /* uncalled namespace function */ }
}
