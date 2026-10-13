// The tracer for ADR 0139: a constructor that keeps its caller's callable on
// the instance, under a fixed key, for members to invoke later. Bundler
// spelling (`var C = class { … }`), as `@tanstack/router-core` publishes it.

// --- Certifies: the callable is kept, and only a member invokes it. ---

// Kept, and invoked only by a member nothing at construction reaches.
var Keeper = class {
  constructor(callback) {
    this.callback = callback;
  }
  run(value) {
    return this.callback(value);
  }
};

// Invoked once during construction, in the constructor's own frame, and kept.
var Primed = class {
  constructor(callback) {
    callback(0);
    this.callback = callback;
  }
  run(value) {
    return this.callback(value);
  }
};

// Primed's bytes again, under a proposal that describes only the kept item:
// the call in the constructor's own frame is a site no item names.
var PrimedUndescribed = class {
  constructor(callback) {
    callback(0);
    this.callback = callback;
  }
  run(value) {
    return this.callback(value);
  }
};

// --- Keeps callbacks open: the producer states nothing about the argument. ---

// Nothing is kept.
var KeeperUnkept = class {
  constructor(callback) {
    this.ready = true;
  }
};

// The constructor reaches the member that invokes the callable, so it runs
// during construction through `this.run`, which no call item describes.
var RunsAtConstruction = class {
  constructor(callback) {
    this.callback = callback;
    this.run(0);
  }
  run(value) {
    return this.callback(value);
  }
};

// A member hands the kept callable on instead of calling it.
var HandsOn = class {
  constructor(callback) {
    this.callback = callback;
  }
  later() {
    return [1].map(this.callback);
  }
};

// The instance escapes its constructor, so code that never received the
// constructed value can reach the callable through it.
var Escapes = class {
  constructor(callback) {
    this.callback = callback;
    registry.push(this);
  }
  run(value) {
    return this.callback(value);
  }
};

// The key is written again, so what a member invokes is not the argument.
var Rewritten = class {
  constructor(callback) {
    this.callback = callback;
  }
  reset(next) {
    this.callback = next;
  }
  run(value) {
    return this.callback(value);
  }
};

// A member defined outside the class body, the HMR shape of
// `RouterCore.prototype._refreshRoute = …`.
var Augmented = class {
  constructor(callback) {
    this.callback = callback;
  }
};
Augmented.prototype.run = function (value) {
  return this.callback(value);
};

const registry = [];

export {
  Augmented,
  Escapes,
  HandsOn,
  Keeper,
  KeeperUnkept,
  Primed,
  PrimedUndescribed,
  Rewritten,
  RunsAtConstruction,
};
