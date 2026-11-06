// physics.js v1.6.60
export function createBody(x=0,z=0){return {x,z,support:null,height:0,velocity:0,scale:1,scaleVelocity:0,awake:false,held:false,impacts:0};}
export function releaseBody(body){body.held=false;body.velocity=0;body.awake=true;}
export function stepBody(body,elapsed,surface=0){
 if(!body.awake||body.held)return false;
 let remaining=Math.min(elapsed,.25);
 while(remaining>0){
  const dt=Math.min(remaining,1/120);remaining-=dt;
  body.velocity-=18*dt;body.height+=body.velocity*dt;
  if(body.height<surface){body.height=surface;if(body.velocity<-.45){body.scaleVelocity-=Math.min(2.8,-body.velocity*.22);body.velocity*=-.62;body.impacts++;}else body.velocity=0;}
  body.scaleVelocity+=((1-body.scale)*190-body.scaleVelocity*21)*dt;body.scale+=body.scaleVelocity*dt;
 }
 if(body.height===surface&&body.velocity===0&&Math.abs(body.scale-1)<.0008&&Math.abs(body.scaleVelocity)<.004){body.scale=1;body.scaleVelocity=0;body.awake=false;}
 return body.awake;
}

const SIZE=2.2,EPSILON=.00001;
function width(body){return SIZE/Math.sqrt(body.scale);}
function horizontalContact(a,b){return Math.abs(a.x-b.x)<(width(a)+width(b))/2-EPSILON&&Math.abs(a.z-b.z)<(width(a)+width(b))/2-EPSILON;}
function surfaceFor(body,bodies){
 let surface=0,support=null;
 for(const other of bodies){if(other===body||!horizontalContact(body,other))continue;const top=other.height+SIZE*other.scale;if(top>surface&&(top<=body.height+.002||body.support===other)){surface=top;support=other;}}
 return {surface,support};
}
function separateBody(body,bodies){
 for(let pass=0;pass<bodies.length;pass++){
  let changed=false;
  for(const other of bodies){
   if(other===body)continue;
   const gap=(width(body)+width(other))/2,dx=body.x-other.x,dz=body.z-other.z;
   const ox=gap-Math.abs(dx),oz=gap-Math.abs(dz),top=body.height+SIZE*body.scale,otherTop=other.height+SIZE*other.scale,oy=Math.min(top,otherTop)-Math.max(body.height,other.height);
   if(ox<=EPSILON||oz<=EPSILON||oy<=EPSILON)continue;
   const above=body.height+SIZE*body.scale/2>=other.height+SIZE*other.scale/2;
   if(oy<=Math.min(ox,oz)&&(above||other.height>=SIZE*body.scale)){
    body.height=above?otherTop:other.height-SIZE*body.scale;
    if(above)body.support=other;else if(body.velocity>0)body.velocity=0;
   }else if(ox<oz)body.x+=(dx>=0?1:-1)*ox;
   else body.z+=(dz>=0?1:-1)*oz;
   changed=true;
  }
  if(!changed)break;
 }
}
export function stepWorld(bodies,elapsed){
 let remaining=Math.min(elapsed,.25);
 while(remaining>0){
  const dt=Math.min(remaining,1/120);remaining-=dt;
  for(const body of [...bodies].sort((a,b)=>a.height-b.height)){
   if(body.held)continue;
   separateBody(body,bodies);const {surface,support}=surfaceFor(body,bodies);body.support=support;
   if(Math.abs(body.height-surface)>.001)body.awake=true;
   stepBody(body,dt,surface);
  }
  for(const body of bodies)if(!body.held)separateBody(body,bodies);
 }
 return bodies.some(body=>body.awake&&!body.held);
}
export function moveBody(body,target,bodies){
 const count=Math.max(1,Math.ceil(Math.hypot(target.x-body.x,target.y-body.height,target.z-body.z)/.12)),dx=(target.x-body.x)/count,dy=(target.y-body.height)/count,dz=(target.z-body.z)/count;
 body.support=null;
 for(let i=0;i<count;i++){body.x+=dx;body.height=Math.max(0,body.height+dy);body.z+=dz;separateBody(body,bodies);}
 for(const other of bodies)if(!other.held&&Math.abs(other.height-surfaceFor(other,bodies).surface)>.001)other.awake=true;
}
