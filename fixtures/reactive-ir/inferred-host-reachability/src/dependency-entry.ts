import { value } from "./dependency-throws.ts";
console.log(value);
void "throwing static dependency prevents importer evaluation";
