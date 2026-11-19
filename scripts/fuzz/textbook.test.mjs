import { describe, expect, it } from "vitest";
import { judgeTextbook, textbookText } from "./textbook.mjs";

// ADR 1118: the textbook tier. Every (site, code) of a legal program not in its `u` list is proved clean.
describe("judgeTextbook", () => {
  const entry = { certified: false, u: ["App.tsx@1-3|SC2001", "App.tsx@5-9|SC1001"] };
  it("a violation is a false positive, a key outside u is lost, a key of u that left is gained", () => {
    const now = { certified: false, sites: new Map([["App.tsx@1-3|SC2001", "U"], ["App.tsx@20-25|SC4001", "U"], ["App.tsx@30-31|SC1001", "V"]]) };
    expect(judgeTextbook(now, entry)).toEqual({ falsePositives: ["App.tsx@30-31|SC1001"], lost: ["App.tsx@20-25|SC4001"], gained: ["App.tsx@5-9|SC1001"] });
  });
  it("a certified program that is no longer certified is a loss even with keys it held", () => {
    expect(judgeTextbook({ certified: false, sites: new Map([["k|SC1", "U"]]) }, { certified: true, u: [] }).lost).toEqual(["k|SC1"]);
  });
  it("writes programs and keys sorted, one program a line", () => {
    expect(textbookText({ programs: { b: { hash: "h", certified: true, u: [] }, a: { hash: "g", certified: false, u: ["z", "y"] } } })).toBe('{"programs":{\n"a":{"hash":"g","certified":false,"u":["y","z"]},\n"b":{"hash":"h","certified":true,"u":[]}\n}}\n');
  });
});
