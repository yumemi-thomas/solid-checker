import { generateShape } from "./matrix.mjs";
const [code, context, variant] = process.argv.slice(2);
const files = generateShape({ seed: 1 }, { code, context }, { variant: variant || null, binding: "direct", layer: "none" }, 0).built.files;
for (const [k, v] of Object.entries(files)) console.log("=== " + k + "\n" + v);
