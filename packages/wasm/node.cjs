"use strict";

const { checkSync, planSync } = require("./solid-checker-wasm.wasi.cjs");

module.exports = { checkSync, planSync };
