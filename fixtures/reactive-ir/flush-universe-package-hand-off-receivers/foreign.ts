import { runStep } from "steplib";

// A method that names `this`, written in another module: the literal that places it names no receiver itself.
export function importedReader(this: any): void {
  void runStep(() => {
    this.m();
  });
}
