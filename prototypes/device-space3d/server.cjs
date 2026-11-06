// server.cjs v1.0.0
'use strict';
const http=require('node:http'),fs=require('node:fs'),path=require('node:path');
const root=__dirname,ui=path.resolve(root,'../../ui');
const shared={'/icons.js':path.join(ui,'icons.js'),'/locale.js':path.join(ui,'locale.js'),'/logo.png':path.join(ui,'assets/oaclis-particles.png')};
const types={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.svg':'image/svg+xml','.png':'image/png','.txt':'text/plain; charset=utf-8'};
http.createServer((request,response)=>{
 if(request.method!=='GET'&&request.method!=='HEAD'){response.writeHead(405);response.end();return;}
 let name;try{name=decodeURIComponent(new URL(request.url,'http://localhost').pathname);}catch{response.writeHead(400);response.end();return;}
 const file=shared[name]||path.resolve(root,'.'+(name==='/'?'/index.html':name));
 if(!shared[name]&&!file.startsWith(root+path.sep)){response.writeHead(403);response.end();return;}
 if(!shared[name]&&!['.html','.js','.css','.svg','.png','.txt'].includes(path.extname(file))){response.writeHead(404);response.end();return;}
 fs.readFile(file,(error,body)=>{if(error){response.writeHead(404);response.end();return;}response.writeHead(200,{'Content-Type':types[path.extname(file)]||'application/octet-stream','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});response.end(request.method==='HEAD'?undefined:body);});
}).listen(23234,'127.0.0.1',()=>console.log('Bozza Oaclis 3D: http://localhost:23234/'));
