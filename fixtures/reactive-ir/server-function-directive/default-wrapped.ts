"use server";
// A non-function default-export expression under the module directive: the
// wrapped call is provably not a direct function declaration — finding.
import { logged } from "./wrappers";

export default logged(async () => {
  return "report";
});
