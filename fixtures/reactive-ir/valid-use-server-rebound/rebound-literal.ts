"use server";

// Violation: registration gets the literal initializer and throws at boot;
// rebinding it to a function afterwards is too late.
export let n: any = 1;
n = async () => {};
