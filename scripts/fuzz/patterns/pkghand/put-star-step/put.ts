import { flush } from "solid-js";

export function put(t: any) { t.m = function () { flush(); }; }
