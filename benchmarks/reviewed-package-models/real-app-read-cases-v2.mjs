// Existing source/test actions plus explicitly authored offline HTTP data; no seeded source defects.
import {resolve} from 'node:path';
export default [{
  id:'helge-dev',root:resolve('rust/target/app-import-metric/apps/helge-dev'),
  scope:'offline retained client application; adapted dev config preserves its static directory and Solid 1 icon alias; HMR and dependency prebundling disabled',
  config:{publicDir:'static',alias:{'solid-js/web':'@solidjs/web'}},
  offlineResponses:[
    {url:'https://dev.to/api/articles?username=helgelol',body:[
      {id:10001,title:'Offline experiment article',description:'Authored offline sample data',tags:'typescript',category:'programming',link:'https://example.invalid/10001'},
      {id:422939,title:'Filtered offline article',description:'Exercises the existing blacklist',tags:'solid',category:'programming',link:'https://example.invalid/422939'},
    ]},
    {url:'https://dev.to/api/articles/10001',body:{title:'Offline experiment article',url:'https://example.invalid/10001',body_html:'<p>Authored offline article body.</p>'}},
  ],
  provenance:['vite.config.ts','tests/helge.dev.spec.ts'],
  actions:[
    {id:'home',kind:'goto',path:'/',title:'helge - main'},
    {id:'projects',kind:'click',selector:'.innerContainer .buttons a[href="/projects"]',title:'helge - projects'},
    {id:'about',kind:'click',selector:'.innerContainer .buttons a[href="/about"]',title:'helge - about'},
    {id:'home-return',kind:'click',selector:'.innerContainer .buttons a[href="/"]',title:'helge - main'},
    {id:'blog-with-offline-data',kind:'click',selector:'.innerContainer .buttons a[href="/blog"]',title:'helge - blog',visible:'.article'},
    {id:'article-with-offline-data',kind:'click',selector:'.article a[href="/blog/10001"]',title:'Helge — Offline experiment article',visible:'h1.title'},
    {id:'home-after-article',kind:'click',selector:'.innerContainer .buttons a[href="/"]',title:'helge - main'},
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
