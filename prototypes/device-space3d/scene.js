// scene.js v1.12.207
import * as THREE from 'three';
import {OrbitControls} from './vendor/OrbitControls.js';
import {RoundedBoxGeometry} from './vendor/RoundedBoxGeometry.js';
import {RoomEnvironment} from './vendor/RoomEnvironment.js';
import {createBody,releaseBody,stepWorld,moveBody} from './physics.js';

const host=document.querySelector('#scene'),panel=document.querySelector('#panel'),labels=document.querySelector('#labels');
const roles=[{id:'sync',name:'Sincronizzazione',icon:'sync',color:'#ff3048'},{id:'send',name:'Invio',icon:'send',color:'#ffa71b'},{id:'receive',name:'Ricezione',icon:'receive',color:'#10d8ef'},{id:'console',name:'Console remota',icon:'square-stop',color:'#ff63c5'}];
const devices=[
 {id:'rog',name:'ROG',type:'Master',icon:'desktop',color:'#51777b',x:-1,z:1,online:true,chats:[{number:4,name:'Oaclis',icon:'code'},{number:7,name:'Vulcano',icon:'image'},{number:9,name:'3D i-vector',icon:'box'}]},
 {id:'kj',name:'KJ',type:'Linux · Jetson',icon:'cpu',color:'#7c8176',x:4,z:-4,online:true,chats:[{number:4,name:'Oaclis',icon:'code'},{number:9,name:'3D i-vector',icon:'box'}]},
 {id:'kdell',name:'KDell',type:'Linux · WSL',icon:'laptop',color:'#6c798d',x:-7,z:-3,online:false,chats:[{number:4,name:'Oaclis',icon:'code'},{number:7,name:'Vulcano',icon:'image'}]},
 {id:'s24',name:'S24',type:'Android · PWA',icon:'mobile',color:'#929e9e',x:5.8,z:4.4,online:true,chats:[{number:4,name:'Oaclis',icon:'code'},{number:7,name:'Vulcano',icon:'image'},{number:9,name:'3D i-vector',icon:'box'}]},
];
const connections=[{from:'rog',to:'kj',role:'sync',targetRole:'sync',both:true},{from:'rog',to:'kdell',role:'send',targetRole:'receive'},{from:'kdell',to:'rog',role:'send',targetRole:'receive'},{from:'rog',to:'s24',role:'console',targetRole:'console',both:true}];
const scene=new THREE.Scene();scene.background=new THREE.Color('#10191e');scene.fog=new THREE.Fog('#10191e',30,65);
const camera=new THREE.PerspectiveCamera(38,innerWidth/innerHeight,.1,100);
const renderer=new THREE.WebGLRenderer({antialias:true,alpha:false});renderer.setPixelRatio(Math.min(devicePixelRatio,1.5));renderer.setSize(innerWidth,innerHeight);renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.PCFSoftShadowMap;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.05;host.append(renderer.domElement);
const pmrem=new THREE.PMREMGenerator(renderer),room=new RoomEnvironment();scene.environment=pmrem.fromScene(room,.06).texture;scene.environmentIntensity=.75;room.dispose();pmrem.dispose();
const controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=false;controls.minDistance=11;controls.maxDistance=70;controls.maxPolarAngle=Math.PI*.47;controls.minPolarAngle=.12;controls.target.set(-.2,.5,0);controls.listenToKeyEvents(host);
scene.add(new THREE.HemisphereLight('#dceaf3','#243037',1.4));
const key=new THREE.DirectionalLight('#fff3dc',3.2);key.position.set(-7,15,9);key.castShadow=true;key.shadow.mapSize.set(2048,2048);Object.assign(key.shadow.camera,{left:-20,right:20,top:20,bottom:-20,near:1,far:60});key.shadow.normalBias=.04;key.shadow.bias=-.0001;scene.add(key);
const rim=new THREE.DirectionalLight('#b9d5ed',1.8);rim.position.set(8,7,-10);scene.add(rim);
const stage=new THREE.Mesh(new RoundedBoxGeometry(28,.22,21,2,.1),new THREE.MeshStandardMaterial({color:'#18282e',roughness:.75,metalness:.18}));stage.position.y=-.14;stage.receiveShadow=true;scene.add(stage);
const pickables=[],linkMeshes=[],flowDots=[];let selection=null,drag=null,pointerDown=null,noticeTimer,flowFrame=0,renderFrame=0,frames=0,physicsFrame=0,lastPhysics=0,hoverGhost=null,ambientFrame=0,lastAmbient=0;
let ambientMotion=!matchMedia('(prefers-reduced-motion: reduce)').matches;
const cursor={x:innerWidth/2,y:innerHeight/2};
const raycaster=new THREE.Raycaster(),pointer=new THREE.Vector2(),projected=new THREE.Vector3();
const geometry=new RoundedBoxGeometry(2.2,2.2,2.2,4,.19);
const ghostGeometry=new THREE.LatheGeometry([new THREE.Vector2(0,-.25),new THREE.Vector2(.31,-.25),new THREE.Vector2(.32,-.13),new THREE.Vector2(.30,.1),new THREE.Vector2(.285,.22),new THREE.Vector2(.23,.33),new THREE.Vector2(.13,.40),new THREE.Vector2(0,.43)],32);
const ghostVertices=ghostGeometry.attributes.position,ghostBase=ghostVertices.array.slice();
for(let i=0;i<ghostVertices.count;i++){const x=ghostVertices.getX(i),y=ghostVertices.getY(i),z=ghostVertices.getZ(i);if(y<-.12&&Math.hypot(x,z)>.1)ghostVertices.setY(i,y+Math.cos(Math.atan2(z,x)*6)*.055*(-.12-y)/.13);}
ghostGeometry.computeVertexNormals();
function createGhost(device,role){
 const pupils=[],ghost=new THREE.Group(),visual=new THREE.Group(),body=new THREE.Mesh(ghostGeometry,new THREE.MeshStandardMaterial({color:role.color,roughness:.65,metalness:0,envMapIntensity:.12,emissive:role.color,emissiveIntensity:.3}));ghost.add(visual);body.castShadow=true;body.userData={device,role};visual.add(body);pickables.push(body);
 for(const side of [-1,1]){
  const eye=new THREE.Mesh(new THREE.SphereGeometry(1,16,12),new THREE.MeshStandardMaterial({color:'#fffdf4',roughness:.4}));eye.position.set(side*.105,.16,.27);eye.scale.set(.078,.103,.034);visual.add(eye);
  const pupil=new THREE.Mesh(new THREE.SphereGeometry(1,12,8),new THREE.MeshBasicMaterial({color:'#203039'}));pupil.position.set(side*.105,.15,.303);pupil.scale.set(.032,.047,.016);visual.add(pupil);pupils.push({pupil,side});
 }
 const symbol=new THREE.Sprite(new THREE.SpriteMaterial({map:iconMaps.get(role.icon),transparent:true,depthWrite:false}));symbol.scale.set(.34,.34,1);symbol.position.y=.72;visual.add(symbol);ghost.userData={device,role,pupils,visual,phase:devices.indexOf(device)*1.1+roles.indexOf(role)*.9};return ghost;
}

async function iconTexture(name,color='#e9f3f3'){
 const svg='<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 24 24" fill="none" stroke="'+color+'" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round">'+OaclisIcons[name]+'</svg>';
 const image=new Image();image.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(svg);await image.decode();const texture=new THREE.Texture(image);texture.colorSpace=THREE.SRGBColorSpace;texture.needsUpdate=true;return texture;
}
const iconMaps=new Map(await Promise.all([...new Set([...devices.map(d=>d.icon),...roles.map(r=>r.icon),...devices.flatMap(d=>d.chats.map(chat=>chat.icon))])].map(async name=>[name,await iconTexture(name)])));
for(const device of devices){
 device.body=createBody(device.x,device.z);
 const group=device.group=new THREE.Group();group.position.set(device.x,0,device.z);scene.add(group);
 const cube=device.cube=new THREE.Mesh(geometry,new THREE.MeshPhysicalMaterial({color:device.color,roughness:.3,metalness:.32,clearcoat:.65,clearcoatRoughness:.3,envMapIntensity:.9}));cube.position.y=1.1;cube.castShadow=true;cube.receiveShadow=true;cube.userData={device};group.add(cube);pickables.push(cube);
 for(const [position,rotation]of [[[0,0,1.108],[0,0,0]],[[0,0,-1.108],[0,Math.PI,0]],[[1.108,0,0],[0,Math.PI/2,0]],[[-1.108,0,0],[0,-Math.PI/2,0]],[[0,1.108,0],[-Math.PI/2,0,0]],[[0,-1.108,0],[Math.PI/2,0,0]]]){
  const mark=new THREE.Mesh(new THREE.PlaneGeometry(.92,.92),new THREE.MeshStandardMaterial({map:iconMaps.get(device.icon),transparent:true,depthWrite:false,roughness:.6,metalness:0,emissive:'#a7cecc',emissiveIntensity:.15}));mark.position.set(...position);mark.rotation.set(...rotation);cube.add(mark);
 }
 if(device.id==='rog'){
  const shape=new THREE.Shape();shape.moveTo(-.37,0);shape.lineTo(-.45,.36);shape.lineTo(-.18,.22);shape.lineTo(0,.47);shape.lineTo(.18,.22);shape.lineTo(.45,.36);shape.lineTo(.37,0);shape.closePath();
  const crown=new THREE.Mesh(new THREE.ExtrudeGeometry(shape,{depth:.075,bevelEnabled:true,bevelSegments:2,steps:1,bevelSize:.025,bevelThickness:.025}),new THREE.MeshStandardMaterial({color:'#d5bd84',metalness:.8,roughness:.28}));crown.position.set(-.03,1.30,.02);crown.rotation.z=.12;cube.add(crown);
 }
 const ring=new THREE.Mesh(new THREE.RingGeometry(1.65,1.665,64),new THREE.MeshBasicMaterial({color:device.color,transparent:true,opacity:.65,side:THREE.DoubleSide}));ring.rotation.x=-Math.PI/2;ring.position.y=.005;group.add(ring);
 device.ports={};for(const role of roles){const ghost=createGhost(device,role);group.add(ghost);device.ports[role.id]=ghost;}
 const label=device.label=document.createElement('button');label.type='button';label.className='device-label';label.dataset.device=device.id;label.setAttribute('aria-expanded','false');label.innerHTML='<strong>'+device.name+'</strong><small>'+device.type.split(' · ')[0]+'</small>';label.onclick=()=>openPanel(device);label.onkeydown=event=>{if(event.code==='Space'){event.preventDefault();device.body.height=Math.min(9,device.body.height+3);releaseBody(device.body);startPhysics();}};labels.append(label);
}
const bodies=devices.map(device=>device.body);
for(const connection of connections){
 const role=roles.find(r=>r.id===connection.role);connection.material=new THREE.MeshStandardMaterial({color:role.color,metalness:.3,roughness:.45,transparent:true,opacity:.65,emissive:role.color,emissiveIntensity:.12});
 const mesh=connection.mesh=new THREE.Mesh(new THREE.BufferGeometry(),connection.material);scene.add(mesh);linkMeshes.push(mesh);
 const source=devices.find(device=>device.id===connection.from),target=devices.find(device=>device.id===connection.to);
 connection.chats=source.chats.filter(chat=>target.chats.some(other=>other.number===chat.number)).map(chat=>{
  const ball=new THREE.Mesh(new THREE.SphereGeometry(.36,32,24),new THREE.MeshPhysicalMaterial({color:'#466a7d',roughness:.26,metalness:.22,clearcoat:.85,clearcoatRoughness:.18}));
  const icon=new THREE.Sprite(new THREE.SpriteMaterial({map:iconMaps.get(chat.icon),transparent:true,depthWrite:false}));icon.scale.set(.48,.48,1);icon.renderOrder=2;ball.castShadow=true;ball.receiveShadow=true;ball.add(icon);ball.userData={device:source,chat,connection,object:ball,icon};scene.add(ball);pickables.push(ball);return {chat,ball};
 });
 for(const reverse of connection.both?[false,true]:[false]){const dot=new THREE.Mesh(new THREE.SphereGeometry(.075,12,8),new THREE.MeshBasicMaterial({color:'#e4fff4'}));dot.visible=false;scene.add(dot);flowDots.push({connection,dot,reverse});}
}
function updateLinks(){
 for(const device of devices){
  const angle=Math.atan2(camera.position.x-device.group.position.x,camera.position.z-device.group.position.z);
  roles.forEach((role,index)=>{const a=angle+(index-1.5)*.7,ghost=device.ports[role.id];ghost.position.set(Math.sin(a)*2.05,.9,Math.cos(a)*2.05);ghost.rotation.y=angle;});
 }
 scene.updateMatrixWorld(true);
 for(const connection of connections){
  const from=devices.find(d=>d.id===connection.from),to=devices.find(d=>d.id===connection.to),a=from.ports[connection.role].getWorldPosition(new THREE.Vector3()),b=to.ports[connection.targetRole].getWorldPosition(new THREE.Vector3());
  const distance=a.distanceTo(b),reach=Math.min(distance*.32,3),normalA=a.clone().sub(from.group.position).setY(0).normalize(),normalB=b.clone().sub(to.group.position).setY(0).normalize();
  const c=a.clone().addScaledVector(normalA,reach),d=b.clone().addScaledVector(normalB,reach);c.y+=Math.min(4,distance*.3);d.y+=Math.min(4,distance*.3);
  connection.curve=new THREE.CubicBezierCurve3(a,c,d,b);connection.mesh.geometry.dispose();connection.mesh.geometry=new THREE.TubeGeometry(connection.curve,48,.018,6,false);
  connection.chats.forEach(({ball},index)=>ball.position.copy(connection.curve.getPointAt((index+1)/(connection.chats.length+1))));
 }
 requestRender();
}
function screenPoint(point){projected.copy(point).project(camera);return {x:(projected.x*.5+.5)*innerWidth,y:(-.5*projected.y+.5)*innerHeight,visible:projected.z<1};}
function render(){
 renderFrame=0;frames++;animateGhostVisuals(performance.now());
 for(const connection of connections)for(const {ball}of connection.chats)ball.userData.icon.position.copy(camera.position).sub(ball.position).normalize().multiplyScalar(.372);
 renderer.render(scene,camera);
 for(const device of devices){const p=screenPoint(device.cube.getWorldPosition(new THREE.Vector3())),bottom=Math.max(...Object.values(device.ports).map(ghost=>screenPoint(ghost.getWorldPosition(new THREE.Vector3())).y));device.label.style.left=p.x+'px';device.label.style.top=bottom+32+'px';device.label.hidden=!p.visible;}
 if(selection){
  const object=selection.object||(selection.role?selection.device.ports[selection.role.id]:selection.device.cube),p=screenPoint(object.getWorldPosition(new THREE.Vector3())),box=panel.getBoundingClientRect();
  const x=p.x<box.left?box.left:p.x>box.right?box.right:Math.max(box.left+18,Math.min(box.right-18,p.x)),y=Math.max(box.top+20,Math.min(box.bottom-20,p.y));
  document.querySelector('#tether path').setAttribute('d',p.visible?'M'+p.x+' '+p.y+'Q'+(p.x+x)/2+' '+(p.y+y)/2+' '+x+' '+y:'');
 }
}
function animateGhostVisuals(now){
 const time=ambientMotion?now/1000:0;
 for(let i=0;i<ghostVertices.count;i++){
  const x=ghostBase[i*3],y=ghostBase[i*3+1],z=ghostBase[i*3+2];
  if(y<-.12&&Math.hypot(x,z)>.1)ghostVertices.setY(i,y+Math.cos(Math.atan2(z,x)*6+time*8)*.055*(-.12-y)/.13);
 }
 ghostVertices.needsUpdate=true;ghostGeometry.computeVertexNormals();
 for(const device of devices)for(const ghost of Object.values(device.ports)){
  const {visual,phase,pupils}=ghost.userData,t=(now-(ghost.userData.hoverAt??-1000))/900,hover=t>=0&&t<1;
  visual.position.y=(ambientMotion?Math.sin(time*2.8+phase)*.07:0)+(hover?Math.sin(t*Math.PI)*.26:0);
  visual.rotation.z=(ambientMotion?Math.sin(time*2.4+phase)*.04:0)+(hover?Math.sin(t*Math.PI*2)*.13*(1-t):0);
  visual.scale.set(1,ambientMotion?1+Math.sin(time*4+phase)*.025:1,1);
  const at=screenPoint(ghost.getWorldPosition(new THREE.Vector3())),dx=cursor.x-at.x,dy=cursor.y-at.y,length=Math.max(40,Math.hypot(dx,dy));
  for(const {pupil,side}of pupils){pupil.position.x=side*.105+dx/length*.035;pupil.position.y=.15-dy/length*.038;}
 }
}
function animateGhosts(now){ambientFrame=0;if(!ambientMotion||document.hidden)return;if(now-lastAmbient>=1000/30){lastAmbient=now;requestRender();}ambientFrame=requestAnimationFrame(animateGhosts);}
function startAmbient(){if(ambientMotion&&!ambientFrame&&!document.hidden)ambientFrame=requestAnimationFrame(animateGhosts);}
document.querySelector('#ghost-motion').onclick=()=>{ambientMotion=!ambientMotion;document.querySelector('#ghost-motion').setAttribute('aria-pressed',String(ambientMotion));cancelAnimationFrame(ambientFrame);ambientFrame=0;startAmbient();requestRender();};
window.addEventListener('pointermove',event=>{cursor.x=event.clientX;cursor.y=event.clientY;requestRender();},{passive:true});
function requestRender(){if(!renderFrame)renderFrame=requestAnimationFrame(render);}
function notice(text){clearTimeout(noticeTimer);const box=document.querySelector('#notice');box.textContent=text;box.hidden=false;noticeTimer=setTimeout(()=>{box.hidden=true;},2400);}
function openPanel(device,role=null,chat=null,object=null){
 selection={device,role,chat,object};panel.hidden=false;devices.forEach(d=>d.label.setAttribute('aria-expanded',String(d===device)));
 document.querySelector('#panel-kind').textContent=chat?'Chat '+chat.number+' · '+device.name:role?device.name+' · collegamento':device.type;
 document.querySelector('#panel-title').textContent=chat?chat.name:role?role.name:device.name;
 document.querySelector('#panel-body').innerHTML='<div class="summary"><span>Stato dimostrativo</span><strong class="state">'+(device.online?'Collegato':'Non raggiungibile')+'</strong></div>'+(chat?[chat]:device.chats).map(chat=>'<div class="chat-row">'+OaclisIcon(chat.icon)+'<div><strong>'+chat.name+'</strong><small>Chat '+chat.number+'</small></div><button type="button" aria-label="Apri console '+chat.name+'" title="Apri console">'+OaclisIcon('terminal')+'</button></div>').join('')+(role||chat?'':'<div class="port-legend">'+roles.map(r=>'<button type="button" data-role="'+r.id+'">'+OaclisIcon(r.icon)+'<span>'+r.name+'</span></button>').join('')+'</div>')+'<p class="panel-note">Bozza con dati statici. Le console sono simulate.</p>';
 panel.querySelectorAll('[data-role]').forEach(button=>button.onclick=()=>openPanel(device,roles.find(role=>role.id===button.dataset.role)));
 panel.querySelectorAll('.chat-row button').forEach(button=>button.onclick=()=>notice('Console dimostrativa · nessuna sessione reale aperta'));
 requestRender();
}
function closePanel(){selection=null;panel.hidden=true;document.querySelector('#tether path').setAttribute('d','');devices.forEach(d=>d.label.setAttribute('aria-expanded','false'));requestRender();}
function resetCamera(top=false){controls.target.set(-.2,.5,0);camera.position.set(...(top?[0,27,.01]:[16,17,23]));if(innerWidth<600)camera.position.multiplyScalar(1.8);scene.fog.near=innerWidth<600?55:30;scene.fog.far=innerWidth<600?105:65;controls.update();requestRender();}
function pick(event){const box=renderer.domElement.getBoundingClientRect();pointer.set((event.clientX-box.left)/box.width*2-1,-(event.clientY-box.top)/box.height*2+1);raycaster.setFromCamera(pointer,camera);return raycaster.intersectObjects(pickables,false)[0]?.object.userData;}
function startPhysics(){if(!physicsFrame&&!document.hidden){lastPhysics=performance.now();physicsFrame=requestAnimationFrame(advancePhysics);}}
function advancePhysics(now){
 physicsFrame=0;const elapsed=(now-lastPhysics)/1000;lastPhysics=now;let active=stepWorld(bodies,elapsed);
 for(const device of devices){
  device.group.position.set(device.body.x,device.body.height,device.body.z);
  const scale=device.body.scale;device.cube.scale.set(1/Math.sqrt(scale),scale,1/Math.sqrt(scale));device.cube.position.y=1.1*scale;
  for(const ghost of Object.values(device.ports)){const t=(now-(ghost.userData.hoverAt??-1000))/900;active=(t>=0&&t<1)||active;}
 }
 updateLinks();if(active&&!document.hidden)physicsFrame=requestAnimationFrame(advancePhysics);
}
function releaseDrag(){
 if(drag){const {device}=drag;device.body.held=false;if(device.body.height>0){releaseBody(device.body);startPhysics();}drag=null;}
 controls.enabled=true;pointerDown=null;
}
function moveDragged(event){
 pick(event);const point=raycaster.ray.intersectPlane(drag.plane,new THREE.Vector3());
 if(point){point.add(drag.offset);point.set(THREE.MathUtils.clamp(point.x,-11.5,11.5),THREE.MathUtils.clamp(point.y,0,9),THREE.MathUtils.clamp(point.z,-8,8));moveBody(drag.device.body,point,bodies);drag.device.group.position.set(drag.device.body.x,drag.device.body.height,drag.device.body.z);startPhysics();updateLinks();}
}
renderer.domElement.addEventListener('pointerdown',event=>{
 if(event.button!==0)return;
 const hit=pick(event);pointerDown={x:event.clientX,y:event.clientY};
 if(hit&&!hit.role&&!hit.chat){
  const plane=new THREE.Plane().setFromNormalAndCoplanarPoint(camera.getWorldDirection(new THREE.Vector3()),hit.device.group.position),point=raycaster.ray.intersectPlane(plane,new THREE.Vector3());
  if(point){drag={device:hit.device,plane,offset:hit.device.group.position.clone().sub(point),moved:false};hit.device.body.held=true;hit.device.body.awake=false;hit.device.body.velocity=0;controls.enabled=false;renderer.domElement.setPointerCapture(event.pointerId);}
 }
},true);
renderer.domElement.addEventListener('wheel',event=>{
 if(!drag)return;event.preventDefault();event.stopImmediatePropagation();drag.moved=true;
 drag.offset.addScaledVector(camera.getWorldDirection(new THREE.Vector3()),event.deltaY*.01);moveDragged(event);
},{capture:true,passive:false});
renderer.domElement.addEventListener('pointermove',event=>{
 if(drag){drag.moved||=Math.hypot(event.clientX-pointerDown.x,event.clientY-pointerDown.y)>=4;moveDragged(event);return;}
 const hit=pick(event),ghost=hit?.role?hit.device.ports[hit.role.id]:null;
 if(ghost&&hoverGhost!==ghost){ghost.userData.hoverAt=performance.now();startPhysics();}hoverGhost=ghost;
 host.style.cursor=hit?(hit.role||hit.chat?'pointer':'grab'):'grab';requestRender();
});
renderer.domElement.addEventListener('pointerup',event=>{
 if(drag){const {device,moved}=drag;releaseDrag();if(renderer.domElement.hasPointerCapture(event.pointerId))renderer.domElement.releasePointerCapture(event.pointerId);if(!moved)openPanel(device);return;}
 if(pointerDown&&Math.hypot(event.clientX-pointerDown.x,event.clientY-pointerDown.y)<5){const hit=pick(event);if(hit)openPanel(hit.device,hit.role,hit.chat,hit.object);else closePanel();}pointerDown=null;
});
renderer.domElement.addEventListener('pointercancel',releaseDrag);
controls.addEventListener('change',updateLinks);
document.querySelector('#close-panel').onclick=closePanel;
document.querySelector('#reset').onclick=()=>resetCamera();document.querySelector('#top').onclick=()=>resetCamera(true);
function stopFlow(){cancelAnimationFrame(flowFrame);flowFrame=0;flowDots.forEach(({dot})=>{dot.visible=false;});document.querySelector('#flow').removeAttribute('aria-busy');requestRender();}
document.querySelector('#flow').onclick=()=>{
 if(flowFrame){stopFlow();return;}
 const started=performance.now();document.querySelector('#flow').setAttribute('aria-busy','true');
 const animate=now=>{const elapsed=now-started;if(document.hidden||elapsed>=5300){stopFlow();return;}const t=(elapsed%2150)/1150;
  flowDots.forEach(({connection,dot,reverse})=>{dot.visible=t<=1;if(t<=1)dot.position.copy(connection.curve.getPointAt(reverse?1-t:t));});requestRender();flowFrame=requestAnimationFrame(animate);
 };flowFrame=requestAnimationFrame(animate);
};
document.addEventListener('visibilitychange',()=>{if(document.hidden){stopFlow();cancelAnimationFrame(ambientFrame);ambientFrame=0;cancelAnimationFrame(physicsFrame);physicsFrame=0;releaseDrag();}else{startAmbient();if(devices.some(d=>d.body.awake))startPhysics();requestRender();}});
window.addEventListener('keydown',event=>{if(event.key==='Escape')closePanel();});
window.addEventListener('resize',()=>{camera.aspect=innerWidth/innerHeight;camera.updateProjectionMatrix();renderer.setSize(innerWidth,innerHeight);requestRender();});
window.Oaclis3DDemo={scene,devices,connections,roles,renderer,camera,controls,openPanel,updateLinks,screenPoint,startPhysics,get ambientMotion(){return ambientMotion;},get physicsRunning(){return physicsFrame!==0;},get frames(){return frames;}};
resetCamera();updateLinks();document.querySelector('#loading').hidden=true;document.querySelector('#ghost-motion').setAttribute('aria-pressed',String(ambientMotion));startAmbient();requestRender();
