import { enableExternalSource as configure } from "@solidjs/signals";
configure({ factory: fn => ({ track: fn, dispose() {} }) });
