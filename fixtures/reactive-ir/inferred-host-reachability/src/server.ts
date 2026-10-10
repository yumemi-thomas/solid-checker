"use server";
import { startClosed } from "reactive-package";
import { serverOnly, browserHelper } from "./shared.ts";
export async function remote(_value: () => void) { startClosed(); /* RPC body */ serverOnly(); browserHelper(); }
