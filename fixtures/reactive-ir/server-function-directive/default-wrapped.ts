"use server";
// A default export holding a call: registered by value like any other export,
// so it is an uncertifiable result (the facts do not show the result is a
// function), never a violation.
import { logged } from "./wrappers";

export default logged(async () => {
  return "report";
});
