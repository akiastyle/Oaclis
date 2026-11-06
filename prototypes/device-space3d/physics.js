// physics.js v1.0.0
export function createBody(){return {height:0,velocity:0,scale:1,scaleVelocity:0,awake:false,held:false,impacts:0};}
export function releaseBody(body){body.held=false;body.velocity=0;body.awake=true;}
export function stepBody(body,elapsed){
 if(!body.awake||body.held)return false;
 let remaining=Math.min(elapsed,.08);
 while(remaining>0){
  const dt=Math.min(remaining,1/120);remaining-=dt;
  body.velocity-=18*dt;body.height+=body.velocity*dt;
  if(body.height<0){body.height=0;if(body.velocity<-.45){body.scaleVelocity-=Math.min(2.8,-body.velocity*.22);body.velocity*=-.38;body.impacts++;}else body.velocity=0;}
  body.scaleVelocity+=((1-body.scale)*190-body.scaleVelocity*21)*dt;body.scale+=body.scaleVelocity*dt;
 }
 if(body.height===0&&body.velocity===0&&Math.abs(body.scale-1)<.0008&&Math.abs(body.scaleVelocity)<.004){body.scale=1;body.scaleVelocity=0;body.awake=false;}
 return body.awake;
}
