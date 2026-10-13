// Existing source and browser-test actions; no seeded defects or package changes.
import {resolve} from 'node:path';
export default [{
  id:'helge-dev',root:resolve('rust/target/app-import-metric/apps/helge-dev'),
  scope:'offline retained client application; adapted dev config preserves its static directory and Solid 1 icon alias; HMR and dependency prebundling disabled',
  config:{publicDir:'static',alias:{'solid-js/web':'@solidjs/web'}},
  provenance:['vite.config.ts','tests/helge.dev.spec.ts'],
  actions:[
    {id:'home',kind:'goto',path:'/',title:'helge - main'},
    {id:'projects',kind:'click',selector:'.innerContainer .buttons a[href="/projects"]',title:'helge - projects'},
    {id:'about',kind:'click',selector:'.innerContainer .buttons a[href="/about"]',title:'helge - about'},
    {id:'home-return',kind:'click',selector:'.innerContainer .buttons a[href="/"]',title:'helge - main'},
    {id:'open-contact',kind:'click',selector:'.icons [role="button"]',visible:'.modal'},
    {id:'close-contact',kind:'corner-click',hidden:'.modal'},
    {id:'mobile',kind:'viewport',width:600,height:800},
    {id:'open-mobile-menu',kind:'click',selector:'button[aria-label="menu-burger-button"]',visible:'.responsiveButtons .button'},
    {id:'mobile-projects',kind:'click',selector:'.responsiveButtons a[href="/projects"]',title:'helge - projects',closedMenu:true},
    {id:'desktop',kind:'viewport',width:1280,height:900},
    {id:'not-found',kind:'goto',path:'/definitely-not-a-real-path',title:'helge - not found'},
    {id:'not-found-return',kind:'click',selector:'main.home a',title:'helge - main'},
    ...Array.from({length:12},(_,index)=>[
      {id:'contact-repeat-'+index+'-open',kind:'click',selector:'.icons [role="button"]',visible:'.modal'},
      {id:'contact-repeat-'+index+'-close',kind:'corner-click',hidden:'.modal'},
    ]).flat(),
  ],
}];
