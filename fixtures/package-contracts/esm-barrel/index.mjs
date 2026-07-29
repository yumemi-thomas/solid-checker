import { createValue } from "./create-value.mjs";

const createLocal = ((...values) => values.length);
const createAlias = createValue;
const createConditional = true
  ? function createConditionalLeft() {
      return 1;
    }
  : function createConditionalRight() {
      return 2;
    };
const factory = {
  create() {
    return (...values) => values.length;
  },
};
const createFromMemberFactory = factory.create();
const proxiedFactory = new Proxy(factory, {});
const factoryComponent = proxiedFactory.create();
const bootstrapSource = "return (value) => value";

export {
  bootstrapSource,
  createAlias,
  createConditional,
  createFromMemberFactory,
  createLocal,
  createValue,
  factoryComponent,
};
