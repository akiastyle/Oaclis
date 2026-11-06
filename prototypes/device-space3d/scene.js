// scene.js v1.0.0
import * as THREE from 'three';
import {OrbitControls} from './vendor/OrbitControls.js';
import {RoundedBoxGeometry} from './vendor/RoundedBoxGeometry.js';
import {RoomEnvironment} from './vendor/RoomEnvironment.js';

const host=document.querySelector('#scene'),panel=document.querySelector('#panel'),labels=document.querySelector('#labels');
const roles=[{id:'sync',name:'Sincronizzazione',icon:'sync',color:'#94d9ca'},{id:'send',name:'Invio',icon:'send',color:'#d4b68e'},{id:'receive',name:'Ricezione',icon:'receive',color:'#b6c8db'},{id:'console',name:'Console remota',icon:'square-stop',color:'#b7b4d9'}];
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
const grid=new THREE.GridHelper(26,26,'#294149','#20343b');grid.position.y=-.015;grid.material.transparent=true;grid.material.opacity=.6;scene.add(grid);
const pickables=[],linkMeshes=[],flowDots=[];let selection=null,moveMode=false,drag=null,pointerDown=null,noticeTimer,flowFrame=0,renderFrame=0,frames=0;
const raycaster=new THREE.Raycaster(),pointer=new THREE.Vector2(),floor=new THREE.Plane(new THREE.Vector3(0,1,0),0),projected=new THREE.Vector3();
const geometry=new RoundedBoxGeometry(2.2,2.2,2.2,4,.19),sphereGeometry=new THREE.SphereGeometry(.18,24,16);

async function iconTexture(name,color='#e9f3f3'){
 const svg='<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 24 24" fill="none" stroke="'+color+'" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round">'+OaclisIcons[name]+'</svg>';
 const image=new Image();image.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(svg);await image.decode();const texture=new THREE.Texture(image);texture.colorSpace=THREE.SRGBColorSpace;texture.needsUpdate=true;return texture;
}
const iconMaps=new Map(await Promise.all([...new Set([...devices.map(d=>d.icon),...roles.map(r=>r.icon)])].map(async name=>[name,await iconTexture(name)])));
for(const device of devices){
 const group=device.group=new THREE.Group();group.position.set(device.x,0,device.z);scene.add(group);
 const cube=device.cube=new THREE.Mesh(geometry,new THREE.MeshPhysicalMaterial({color:device.color,roughness:.3,metalness:.32,clearcoat:.65,clearcoatRoughness:.3,envMapIntensity:.9}));cube.position.y=1.18;cube.castShadow=true;cube.receiveShadow=true;cube.userData={device};group.add(cube);pickables.push(cube);
 for(const [position,rotation]of [[[0,0,1.108],[0,0,0]],[[0,0,-1.108],[0,Math.PI,0]],[[1.108,0,0],[0,Math.PI/2,0]],[[-1.108,0,0],[0,-Math.PI/2,0]],[[0,1.108,0],[-Math.PI/2,0,0]],[[0,-1.108,0],[Math.PI/2,0,0]]]){
  const mark=new THREE.Mesh(new THREE.PlaneGeometry(.92,.92),new THREE.MeshStandardMaterial({map:iconMaps.get(device.icon),transparent:true,depthWrite:false,roughness:.6,metalness:0,emissive:'#a7cecc',emissiveIntensity:.15}));mark.position.set(...position);mark.rotation.set(...rotation);cube.add(mark);
 }
 if(device.id==='rog'){
  const shape=new THREE.Shape();shape.moveTo(-.37,0);shape.lineTo(-.45,.36);shape.lineTo(-.18,.22);shape.lineTo(0,.47);shape.lineTo(.18,.22);shape.lineTo(.45,.36);shape.lineTo(.37,0);shape.closePath();
  const crown=new THREE.Mesh(new THREE.ExtrudeGeometry(shape,{depth:.075,bevelEnabled:true,bevelSegments:2,steps:1,bevelSize:.025,bevelThickness:.025}),new THREE.MeshStandardMaterial({color:'#d5bd84',metalness:.8,roughness:.28}));crown.position.set(-.03,2.48,.02);crown.rotation.z=.12;group.add(crown);
 }
 const ring=new THREE.Mesh(new THREE.RingGeometry(1.65,1.665,64),new THREE.MeshBasicMaterial({color:device.color,transparent:true,opacity:.65,side:THREE.DoubleSide}));ring.rotation.x=-Math.PI/2;ring.position.y=.005;group.add(ring);
 device.ports={};for(const role of roles){
  const ball=new THREE.Mesh(sphereGeometry,new THREE.MeshPhysicalMaterial({color:role.color,metalness:.25,roughness:.22,clearcoat:.8,emissive:role.color,emissiveIntensity:.22}));ball.castShadow=true;ball.userData={device,role};group.add(ball);device.ports[role.id]=ball;pickables.push(ball);
  const symbol=new THREE.Sprite(new THREE.SpriteMaterial({map:iconMaps.get(role.icon),transparent:true,depthWrite:false}));symbol.scale.set(.19,.19,1);symbol.position.y=.26;ball.add(symbol);
 }
 const label=device.label=document.createElement('button');label.type='button';label.className='device-label';label.dataset.device=device.id;label.setAttribute('aria-expanded','false');label.innerHTML='<span class="status-dot"'+(device.online?'':' style="background:#89969e"')+'></span><strong>'+device.name+'</strong><small>'+device.type+'</small>';label.onclick=()=>openPanel(device);labels.append(label);
}
for(const connection of connections){
 const role=roles.find(r=>r.id===connection.role);connection.material=new THREE.MeshStandardMaterial({color:role.color,metalness:.3,roughness:.45,transparent:true,opacity:.65,emissive:role.color,emissiveIntensity:.12});
 const mesh=connection.mesh=new THREE.Mesh(new THREE.BufferGeometry(),connection.material);scene.add(mesh);linkMeshes.push(mesh);
 for(const reverse of connection.both?[false,true]:[false]){const dot=new THREE.Mesh(new THREE.SphereGeometry(.075,12,8),new THREE.MeshBasicMaterial({color:'#e4fff4'}));dot.visible=false;scene.add(dot);flowDots.push({connection,dot,reverse});}
}
function updateLinks(){
 for(const device of devices){
  const neighbors=connections.filter(c=>c.from===device.id||c.to===device.id).map(c=>devices.find(d=>d.id===(c.from===device.id?c.to:c.from)));
  const direction=neighbors.reduce((v,peer)=>v.add(peer.group.position.clone().sub(device.group.position).normalize()),new THREE.Vector3());const angle=Math.atan2(direction.z,direction.x);
  roles.forEach((role,index)=>{const a=angle+index*Math.PI/2;device.ports[role.id].position.set(Math.cos(a)*1.82,.88,Math.sin(a)*1.82);});
 }
 scene.updateMatrixWorld(true);
 for(const connection of connections){
  const from=devices.find(d=>d.id===connection.from),to=devices.find(d=>d.id===connection.to),a=from.ports[connection.role].getWorldPosition(new THREE.Vector3()),b=to.ports[connection.targetRole].getWorldPosition(new THREE.Vector3());
  const distance=a.distanceTo(b),reach=Math.min(distance*.32,3),normalA=a.clone().sub(from.group.position).setY(0).normalize(),normalB=b.clone().sub(to.group.position).setY(0).normalize();
  const c=a.clone().addScaledVector(normalA,reach),d=b.clone().addScaledVector(normalB,reach);c.y+=Math.min(4,distance*.3);d.y+=Math.min(4,distance*.3);
  connection.curve=new THREE.CubicBezierCurve3(a,c,d,b);connection.mesh.geometry.dispose();connection.mesh.geometry=new THREE.TubeGeometry(connection.curve,64,.018,6,false);
 }
 requestRender();
}
function screenPoint(point){projected.copy(point).project(camera);return {x:(projected.x*.5+.5)*innerWidth,y:(-.5*projected.y+.5)*innerHeight,visible:projected.z<1};}
function render(){
 renderFrame=0;frames++;renderer.render(scene,camera);
 for(const device of devices){const p=screenPoint(device.group.position.clone().add(new THREE.Vector3(0,.1,1.85)));device.label.style.left=p.x+'px';device.label.style.top=p.y+'px';device.label.hidden=!p.visible;}
 if(selection){
  const object=selection.role?selection.device.ports[selection.role.id]:selection.device.cube,p=screenPoint(object.getWorldPosition(new THREE.Vector3())),box=panel.getBoundingClientRect();
  const x=p.x<box.left?box.left:p.x>box.right?box.right:Math.max(box.left+18,Math.min(box.right-18,p.x)),y=Math.max(box.top+20,Math.min(box.bottom-20,p.y));
  document.querySelector('#tether path').setAttribute('d',p.visible?'M'+p.x+' '+p.y+'Q'+(p.x+x)/2+' '+(p.y+y)/2+' '+x+' '+y:'');
 }
}
function requestRender(){if(!renderFrame)renderFrame=requestAnimationFrame(render);}
function notice(text){clearTimeout(noticeTimer);const box=document.querySelector('#notice');box.textContent=text;box.hidden=false;noticeTimer=setTimeout(()=>{box.hidden=true;},2400);}
function openPanel(device,role=null){
 selection={device,role};panel.hidden=false;devices.forEach(d=>d.label.setAttribute('aria-expanded',String(d===device)));
 document.querySelector('#panel-kind').textContent=role?device.name+' · collegamento':device.type;
 document.querySelector('#panel-title').textContent=role?role.name:device.name;
 document.querySelector('#panel-body').innerHTML='<div class="summary"><span>Stato dimostrativo</span><strong class="state">'+(device.online?'Collegato':'Non raggiungibile')+'</strong></div>'+device.chats.map(chat=>'<div class="chat-row">'+OaclisIcon(chat.icon)+'<div><strong>'+chat.name+'</strong><small>Chat '+chat.number+'</small></div><button type="button" aria-label="Apri console '+chat.name+'" title="Apri console">'+OaclisIcon('terminal')+'</button></div>').join('')+(role?'':'<div class="port-legend">'+roles.map(r=>'<button type="button" data-role="'+r.id+'">'+OaclisIcon(r.icon)+'<span>'+r.name+'</span></button>').join('')+'</div>')+'<p class="panel-note">Bozza con dati statici. Le console sono simulate.</p>';
 panel.querySelectorAll('[data-role]').forEach(button=>button.onclick=()=>openPanel(device,roles.find(role=>role.id===button.dataset.role)));
 panel.querySelectorAll('.chat-row button').forEach(button=>button.onclick=()=>notice('Console dimostrativa · nessuna sessione reale aperta'));
 requestRender();
}
function closePanel(){selection=null;panel.hidden=true;document.querySelector('#tether path').setAttribute('d','');devices.forEach(d=>d.label.setAttribute('aria-expanded','false'));requestRender();}
function resetCamera(top=false){controls.target.set(-.2,.5,0);camera.position.set(...(top?[0,27,.01]:[16,17,23]));if(innerWidth<600)camera.position.multiplyScalar(1.8);scene.fog.near=innerWidth<600?55:30;scene.fog.far=innerWidth<600?105:65;controls.update();requestRender();}
function pick(event){const box=renderer.domElement.getBoundingClientRect();pointer.set((event.clientX-box.left)/box.width*2-1,-(event.clientY-box.top)/box.height*2+1);raycaster.setFromCamera(pointer,camera);return raycaster.intersectObjects(pickables,false)[0]?.object.userData;}
renderer.domElement.addEventListener('pointerdown',event=>{
 const hit=pick(event);pointerDown={x:event.clientX,y:event.clientY};
 if(moveMode&&hit&&!hit.role){const point=raycaster.ray.intersectPlane(floor,new THREE.Vector3());if(point){drag={device:hit.device,offset:hit.device.group.position.clone().sub(point)};controls.enabled=false;renderer.domElement.setPointerCapture(event.pointerId);}}
},true);
renderer.domElement.addEventListener('pointermove',event=>{
 if(drag){pick(event);const point=raycaster.ray.intersectPlane(floor,new THREE.Vector3());if(point){point.add(drag.offset);drag.device.group.position.set(THREE.MathUtils.clamp(point.x,-11.5,11.5),0,THREE.MathUtils.clamp(point.z,-8,8));updateLinks();}return;}
 host.style.cursor=pick(event)?(moveMode?'move':'pointer'):'grab';
});
renderer.domElement.addEventListener('pointerup',event=>{
 if(drag){drag=null;controls.enabled=true;if(renderer.domElement.hasPointerCapture(event.pointerId))renderer.domElement.releasePointerCapture(event.pointerId);pointerDown=null;return;}
 if(pointerDown&&Math.hypot(event.clientX-pointerDown.x,event.clientY-pointerDown.y)<5){const hit=pick(event);if(hit)openPanel(hit.device,hit.role);else closePanel();}pointerDown=null;
});
renderer.domElement.addEventListener('pointercancel',()=>{drag=null;controls.enabled=true;pointerDown=null;});
controls.addEventListener('change',requestRender);
document.querySelector('#close-panel').onclick=closePanel;
document.querySelector('#reset').onclick=()=>resetCamera();document.querySelector('#top').onclick=()=>resetCamera(true);
document.querySelector('#move').onclick=()=>{moveMode=!moveMode;document.querySelector('#move').setAttribute('aria-pressed',String(moveMode));notice(moveMode?'Trascina un cubo per spostarlo sul piano':'Trascina lo spazio per ruotare la vista');};
function stopFlow(){cancelAnimationFrame(flowFrame);flowFrame=0;flowDots.forEach(({dot})=>{dot.visible=false;});document.querySelector('#flow').removeAttribute('aria-busy');requestRender();}
document.querySelector('#flow').onclick=()=>{
 if(flowFrame){stopFlow();return;}
 const started=performance.now();document.querySelector('#flow').setAttribute('aria-busy','true');
 const animate=now=>{const elapsed=now-started;if(document.hidden||elapsed>=5300){stopFlow();return;}const t=(elapsed%2150)/1150;
  flowDots.forEach(({connection,dot,reverse})=>{dot.visible=t<=1;if(t<=1)dot.position.copy(connection.curve.getPointAt(reverse?1-t:t));});requestRender();flowFrame=requestAnimationFrame(animate);
 };flowFrame=requestAnimationFrame(animate);
};
document.addEventListener('visibilitychange',()=>{if(document.hidden)stopFlow();else requestRender();});
window.addEventListener('keydown',event=>{if(event.key==='Escape')closePanel();});
window.addEventListener('resize',()=>{camera.aspect=innerWidth/innerHeight;camera.updateProjectionMatrix();renderer.setSize(innerWidth,innerHeight);requestRender();});
window.Oaclis3DDemo={devices,connections,roles,renderer,camera,controls,openPanel,updateLinks,screenPoint,get frames(){return frames;}};
resetCamera();updateLinks();document.querySelector('#loading').hidden=true;requestRender();
