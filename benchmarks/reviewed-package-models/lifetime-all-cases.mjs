import lifecycle from './lifetime-cases.mjs';
import continuations from './lifetime-continuation-cases.mjs';
import rafAndCost from './lifetime-raf-cost-cases.mjs';
import coercions from './lifetime-coercion-cases.mjs';
export default [...lifecycle, ...continuations, ...rafAndCost, ...coercions];
