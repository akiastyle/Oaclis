// flat-view.js v1.0.3
import * as THREE from 'three';
export function createFlatView({root,camera,devices,roles,connections,onOpen,onMove,onDismiss,afterRender}){
 const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),ray=new THREE.Raycaster(),pointer=new THREE.Vector2(),floor=new THREE.Plane(new THREE.Vector3(0,1,0),0);let gesture=null;
 svg.classList.add('flat-links');root.append(svg);
 function project(point){const v=point.clone().project(camera);return {x:(v.x+1)*innerWidth/2,y:(1-v.y)*innerHeight/2};}
 function ground(event){pointer.set(event.clientX/innerWidth*2-1,1-event.clientY/innerHeight*2);ray.setFromCamera(pointer,camera);return ray.ray.intersectPlane(floor,new THREE.Vector3());}
 for(const device of devices){
  const node=device.flatNode=document.createElement('div');node.className='flat-node';node.style.setProperty('--device-color',device.color);
  const button=document.createElement('button');button.type='button';button.className='flat-device';button.dataset.flatDevice=device.id;button.innerHTML=OaclisIcon(device.icon)+'<span><strong>'+device.name+'</strong><small>'+device.type.split(' · ')[0]+'</small></span>';button.onclick=event=>{if(event.detail===0)onOpen(device);};device.cube.userData.flatElement=button;node.append(button);
  button.onkeydown=event=>{const direction={ArrowLeft:[-.25,0],ArrowRight:[.25,0],ArrowUp:[0,-.25],ArrowDown:[0,.25]}[event.key];if(direction){event.preventDefault();onMove(device,{x:device.body.x+direction[0],z:device.body.z+direction[1]});render();}};
  roles.forEach((role,index)=>{const port=document.createElement('button');port.type='button';port.className='flat-port';port.dataset.index=index;port.style.color=role.color;port.title=role.name;port.setAttribute('aria-label',device.name+' · '+role.name);port.innerHTML=OaclisIcon(role.icon);port.onclick=()=>onOpen(device,role);device.ports[role.id].userData.flatElement=port;node.append(port);});root.append(node);
 }
 for(const connection of connections){
  if(connection.role==='console'&&connection.chats.some(({chat})=>chat.active)){for(const [id,role]of [[connection.from,connection.role],[connection.to,connection.targetRole]]){const port=devices.find(d=>d.id===id).ports[role].userData.flatElement;port.classList.add('chat-active');port.style.removeProperty('color');}}
  connection.flatPath=document.createElementNS(ns,'path');connection.flatPath.style.stroke=connection.material.color.getStyle();svg.append(connection.flatPath);
  for(const {chat,ball}of connection.chats){const button=document.createElement('button');button.type='button';button.className='flat-chat';button.classList.toggle('chat-active',Boolean(chat.active));button.title=chat.name+(chat.active?' · Sessione aperta':'');button.setAttribute('aria-label',chat.name);button.innerHTML=OaclisIcon(chat.icon);button.onclick=()=>onOpen(ball.userData.device,null,chat,ball);ball.userData.flatElement=button;root.append(button);}
 }
 function portPoint(device,role){const box=device.ports[role].userData.flatElement.getBoundingClientRect(),index=roles.findIndex(r=>r.id===role),right=index===1||index===2,bottom=index>=2;return {x:box.x+box.width/2,y:box.y+box.height/2,nx:right?1:-1,ny:bottom?1:-1};}
 function render(){
  camera.updateMatrixWorld(true);svg.setAttribute('viewBox','0 0 '+innerWidth+' '+innerHeight);
  for(const device of devices){const point=project(new THREE.Vector3(device.body.x,0,device.body.z));device.flatNode.style.left=point.x+'px';device.flatNode.style.top=point.y+'px';}
  for(const connection of connections){
   const a=portPoint(devices.find(d=>d.id===connection.from),connection.role),b=portPoint(devices.find(d=>d.id===connection.to),connection.targetRole),reach=Math.min(140,Math.hypot(a.x-b.x,a.y-b.y)*.32);
   connection.flatPath.setAttribute('d',`M${a.x} ${a.y}C${a.x+a.nx*reach} ${a.y+a.ny*reach*.35} ${b.x+b.nx*reach} ${b.y+b.ny*reach*.35} ${b.x} ${b.y}`);
   const length=connection.flatPath.getTotalLength();connection.chats.forEach(({ball},index)=>{const point=connection.flatPath.getPointAtLength(length*(index+1)/(connection.chats.length+1));ball.userData.flatElement.style.left=point.x+'px';ball.userData.flatElement.style.top=point.y+'px';});
  }
  afterRender();
 }
 root.addEventListener('pointerdown',event=>{
  if(event.button!==0)return;const button=event.target.closest('[data-flat-device]');if(!button&&event.target.closest('button'))return;
  const device=button?devices.find(d=>d.id===button.dataset.flatDevice):null,at=ground(event);if(!at)return;
  event.preventDefault();button?.focus({preventScroll:true});gesture={device,startX:event.clientX,startY:event.clientY,at,x:device?.body.x,z:device?.body.z,camera:camera.position.clone(),moved:false};root.setPointerCapture(event.pointerId);
 });
 root.addEventListener('pointermove',event=>{
  if(!gesture)return;gesture.moved||=Math.hypot(event.clientX-gesture.startX,event.clientY-gesture.startY)>=4;
  if(gesture.device){const point=ground(event);if(point)onMove(gesture.device,{x:gesture.x+point.x-gesture.at.x,z:gesture.z+point.z-gesture.at.z});}
  else{const unit=2*(camera.position.y-.5)*Math.tan(THREE.MathUtils.degToRad(camera.fov/2))/innerHeight;camera.position.x=gesture.camera.x-(event.clientX-gesture.startX)*unit;camera.position.z=gesture.camera.z-(event.clientY-gesture.startY)*unit;}
  render();
 });
 root.addEventListener('pointerup',event=>{if(!gesture)return;const {device,moved}=gesture;gesture=null;if(root.hasPointerCapture(event.pointerId))root.releasePointerCapture(event.pointerId);if(!moved){if(device)onOpen(device);else onDismiss();}});
 root.addEventListener('pointercancel',()=>{gesture=null;});
 root.addEventListener('wheel',event=>{event.preventDefault();camera.position.y=.5+THREE.MathUtils.clamp((camera.position.y-.5)*Math.exp(event.deltaY*.001),14,80);render();},{passive:false});
 function flow(dots,t){
  for(const item of dots){if(!item.flat){item.flat=document.createElementNS(ns,'circle');item.flat.classList.add('flat-flow');item.flat.setAttribute('r','3.5');svg.append(item.flat);}item.flat.style.display=t<=1?'':'none';if(t<=1){const path=item.connection.flatPath,p=path.getPointAtLength(path.getTotalLength()*(item.reverse?1-t:t));item.flat.setAttribute('cx',p.x);item.flat.setAttribute('cy',p.y);}}
 }
 return {render,flow};
}
