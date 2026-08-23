export * from "bundled-dependency";
export { SiblingCache, siblingFunction, siblingTable } from "./sibling.js";

const LocalCache = class {
  private readonly onChange: () => void;

  constructor(onChange: () => void) {
    this.onChange = onChange;
    onChange();
  }

  notify(): void {
    this.onChange();
  }
};

export const InlineCache = class {
  constructor(onChange: () => void) {
    onChange();
  }
};

export const settings = { retries: 2 };

export { LocalCache };
