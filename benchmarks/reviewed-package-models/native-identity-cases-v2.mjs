// Retain the invalid generic factory as typing exclusions; add valid twins.
import original from './native-identity-cases-v1.mjs';
const generic=original.filter(row=>row.id.includes('generic-factory'));
export default [
  ...original.map(row=>generic.includes(row)?{...row,provenance:{...row.provenance,expectedTypingCode:2769}}:row),
  ...generic.map(row=>({...row,id:row.id.replace('generic-factory','typed-generic-factory'),
    files:{'factory.ts':`import {createSignal} from 'solid-js';export function make<T>(initial:Exclude<T,Function>){return createSignal<T>(initial);}`},
    provenance:{...row.provenance,pair:'typed-generic-factory',realTypingCorrection:true}})),
];
