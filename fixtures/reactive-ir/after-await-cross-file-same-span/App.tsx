import { createEffect, createMemo } from "solid-js";
import { goHelper } from "./helper-b";
import { goIife } from "./iife-b";
import { goTag } from "./tag-b";
import { goEach } from "./each-b";
import { goNone } from "./none-b";

const pause = () => Promise.resolve();

// a project function the entry calls.
export function HelperApp() {
  const m = createMemo(async () => {
    await pause();
    /*[helper*/goHelper( )/*]helper*/;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// an immediately invoked arrow.
export function IifeApp() {
  const m = createMemo(async () => {
    await pause();
    /*[iife*/goIife(/*xxxxx*/)/*]iife*/;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// a tagged template.
export function TagApp() {
  const m = createMemo(async () => {
    await pause();
    /*[tag*/goTag(/*xxxxxx*/)/*]tag*/;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// a callback a standard-library call runs inline.
export function EachApp() {
  const m = createMemo(async () => {
    await pause();
    /*[each*/goEach(/*xxxxxxxxxxxxxx*/)/*]each*/;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// silent twin: the entry reads no source.
export function NoneApp() {
  const m = createMemo(async () => {
    await pause();
    /*[none*/goNone(/*x*/)/*]none*/;
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
