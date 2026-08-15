// No module-level directive: wrapped exports are ordinary values and the
// rule must stay silent, whatever their shape.
import { logged } from "./wrappers";

export const wrappedButFine = logged(async () => 3);
export default logged(async () => 4);
