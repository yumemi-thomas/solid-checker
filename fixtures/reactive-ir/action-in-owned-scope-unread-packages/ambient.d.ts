// A global a script provides: no installed package holds its code, and it cannot reach this build's Solid instance unless the program hands it a function.
declare function legacyGlobal(cb: () => void): number;
