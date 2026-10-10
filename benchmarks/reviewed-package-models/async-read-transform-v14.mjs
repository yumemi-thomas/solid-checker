// Retire cached consumer transforms when a recorded analysis input changes.
// Unrelated dev-server files do not change TypeScript source authority.
import createPlugin from './async-read-transform-v12.mjs';
import {validateProjectInputs} from './project-read-session-v2.mjs';
export default function projectReadPlugin(){
  const plugin=createPlugin(),retire=plugin.handleHotUpdate,cacheInvalidations=[];
  plugin.cacheInvalidations=cacheInvalidations;
  plugin.handleHotUpdate=function(context){
    const inputs=plugin.session.inputs();
    if(!inputs.some(input=>input.path===context.file)&&validateProjectInputs(inputs).valid)return;
    retire.call(plugin,context);const seen=new Set(),paths=new Set(plugin.transformed.map(row=>row.path)),invalidated=[];
    for(const path of paths)for(const module of context.server.moduleGraph.getModulesByFile(path)??[]){
      context.server.moduleGraph.invalidateModule(module,seen,context.timestamp,true,false);invalidated.push(path);
    }
    cacheInvalidations.push({file:context.file,timestamp:context.timestamp,consumers:[...new Set(invalidated)]});
  };
  return plugin;
}
