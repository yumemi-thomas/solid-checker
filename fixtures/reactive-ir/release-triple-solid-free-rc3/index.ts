// No Solid code: a relative import of a project module and a bare import of a
// package whose installed dependency closure (`plain-format` -> `plain-leaf`)
// names none of the Solid 2 packages. The rc.3 triple installed beside it is
// not something this project can reach.
import { format } from "plain-format";
import { label } from "./label";

export const title = format(label("kobalte"));
