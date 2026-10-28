import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { clone } from "./model.mjs";
import { registrationLines } from "./registration.mjs";

// Seed 5 of the containers profile prints a callback declared by name and registered on a later line:
//   const cb1 = () => { setTimeout(() => { const f1 = () => { return onCleanup(() => {}); }; ... }, 0); };
//   onSettled(cb1);
const built = (program) => buildCase(program, { id: "reg" });
const program = generateProgram(5, { profile: "containers" });
const text = built(program).text.split("\n");
const registerOn = text.findIndex((line) => /onSettled\(cb1\)/.test(line)) + 1;
const declOn = text.findIndex((line) => /const cb1 = /.test(line)) + 1;
const innerOn = text.findIndex((line) => /return onCleanup/.test(line)) + 1;

describe("registrationLines", () => {
  it("is the fixture it claims to be", () => {
    expect(registerOn).toBeGreaterThan(declOn);
    expect(innerOn).toBeGreaterThan(declOn);
  });
  it("gives the registering call for every line inside the named callback, including the inner function's", () => {
    const of = registrationLines(program, built(program).lineTable);
    expect(of(declOn)).toEqual([registerOn]);
    expect(of(innerOn)).toEqual([registerOn]);
  });
  it("gives nothing for the registering line itself, nor for lines outside the callback", () => {
    const of = registrationLines(program, built(program).lineTable);
    expect(of(registerOn)).toEqual([]);
    expect(of(1)).toEqual([]);
    expect(of(text.length)).toEqual([]);
  });
  it("gives nothing when the name is declared twice (it cannot say which one is registered)", () => {
    const twice = clone(program);
    const decl = twice.components[0].body.find((node) => node.k === "decl" && node.name === "cb1");
    twice.components[0].body.splice(0, 0, clone(decl));
    const of = registrationLines(twice, built(twice).lineTable);
    expect(text.map((_, index) => of(index + 1)).flat()).toEqual([]);
  });
});
