import * as m from "./exported-getter";

// Redefine every exported function's length before any component renders.
for (const exported of Object.values(m)) {
  if (typeof exported === "function") {
    Object.defineProperty(exported, "length", { value: 1 });
  }
}
