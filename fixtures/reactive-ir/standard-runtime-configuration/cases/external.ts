import { enableExternalSource } from "solid-js";
enableExternalSource({ factory: fn => ({ track: fn, dispose() {} }) });
