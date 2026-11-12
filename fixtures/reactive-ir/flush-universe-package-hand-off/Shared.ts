import { flush } from "solid-js";

// A function another file hands to the package by reference (`Imported` in App.tsx): the reference is in the importing file.
export function shared() {
  flush();
}

// A holder another file hands to the package.
export const holder = {
  run: () => {
    flush();
  },
};
