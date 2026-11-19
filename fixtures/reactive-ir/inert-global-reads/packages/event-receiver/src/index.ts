// Loads Solid, so a global this package wrote would run code that may call an action.
import "solid-js";

// ADR 1108 part 2: `view` of a `UIEvent` is the window (`view: Window | null`), handed on: the package may write any global.
export const viewOf = (event: UIEvent): Window | null => event.view;
