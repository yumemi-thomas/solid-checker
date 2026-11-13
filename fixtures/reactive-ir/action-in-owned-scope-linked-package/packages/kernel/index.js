import { action } from "solid-js";

export const makeKernel = () => ({
  run(cb) {
    void action(function* () {
      cb();
    })();
  },
});
