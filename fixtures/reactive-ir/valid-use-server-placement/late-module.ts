import { helper } from "./helpers";
"use server";
// A module directive after an import is not in the prologue — finding. The
// module is not a directive file, so nothing below is read as one.
export async function getUser() {
  return helper;
}
