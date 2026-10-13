// Additional renamed bindings and optional getter inputs after the final freeze.
import previous from './noise-zero-fresh-cases-v1.mjs';
export default previous.map(row=>({...row,id:row.id.replace('noise-zero-fresh-','noise-zero-fresh-v2-'),source:row.source.replaceAll('makeTask','composeWork').replace('const get=()=>','const get=(flag:boolean=false)=>'),stages:row.stages.map(stage=>({...stage,helper:stage.helper.replaceAll('makeTask','composeWork').replaceAll('read:','reader:').replaceAll('read()','reader()').replaceAll('child(read)','child(reader)')}))}));
