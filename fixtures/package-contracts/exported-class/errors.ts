class BaseError extends Error {}

class ChildError extends BaseError {
  constructor(detail: string) {
    super(`bad ${detail}`);
  }
}

class Watcher {
  private readonly onChange: () => void;

  constructor(onChange: () => void) {
    this.onChange = onChange;
    onChange();
  }

  notify(): void {
    this.onChange();
  }
}

export { BaseError, ChildError, Watcher };
