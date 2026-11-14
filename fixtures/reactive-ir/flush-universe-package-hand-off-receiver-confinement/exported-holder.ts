import { flush } from "solid-js";

export const exportedHolder: any = {};
exportedHolder.exportedM = function () {
  flush();
};
