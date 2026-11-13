import { action } from "solid-js";
export const vendorStep = (cb) => {
  void action(function* () {
    cb();
  })();
};
