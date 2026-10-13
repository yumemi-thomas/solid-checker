// Vite can retain an unchanged importer transform after its dependency changes.
// Retire that transform along with its analysis revision before serving again.
import createPlugin from './async-read-transform-v12.mjs';
export default function projectReadPlugin(){
  const plugin=createPlugin(),retire=plugin.handleHotUpdate,cacheInvalidations=[];
  plugin.cacheInvalidations=cacheInvalidations;
  plugin.handleHotUpdate=function(context){
    retire.call(plugin,context);const seen=new Set(),paths=new Set(plugin.transformed.map(row=>row.path)),invalidated=[];
    for(const path of paths)for(const module of context.server.moduleGraph.getModulesByFile(path)??[]){
      context.server.moduleGraph.invalidateModule(module,seen,context.timestamp,true,false);invalidated.push(path);
    }
    cacheInvalidations.push({file:context.file,timestamp:context.timestamp,consumers:[...new Set(invalidated)]});
  };
  return plugin;
}
