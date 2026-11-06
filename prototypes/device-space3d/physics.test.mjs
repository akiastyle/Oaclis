// physics.test.mjs v1.1.23
import assert from 'node:assert/strict';
import {createBody,releaseBody,stepBody} from './physics.js';
const body=createBody();body.height=4;releaseBody(body);let previousImpact=0,firstRebound=0;
for(let i=0;i<1200&&body.awake;i++){
 stepBody(body,1/120);assert.ok(body.height>=0);assert.ok(body.scale>.85&&body.scale<1.1);
 if(body.impacts===1)firstRebound=Math.max(firstRebound,body.height);
 if(body.impacts>previousImpact){assert.ok(body.velocity>=0);previousImpact=body.impacts;}
}
assert.ok(firstRebound>1.2&&firstRebound<2);assert.ok(body.impacts>=2);assert.equal(body.height,0);assert.equal(body.scale,1);assert.equal(body.awake,false);
const held=createBody();held.height=3;held.held=true;held.awake=true;stepBody(held,1);assert.equal(held.height,3);
console.log('PASS gravity, diminishing rebounds, floor contact, soft deformation, held body and complete sleep');

const {stepWorld,moveBody}=await import('./physics.js');
function assertSolid(bodies){
 for(let i=0;i<bodies.length;i++)for(let j=i+1;j<bodies.length;j++){
  const a=bodies[i],b=bodies[j],gap=(2.2/Math.sqrt(a.scale)+2.2/Math.sqrt(b.scale))/2;
  const overlap=Math.abs(a.x-b.x)<gap-.0001&&Math.abs(a.z-b.z)<gap-.0001&&Math.min(a.height+2.2*a.scale,b.height+2.2*b.scale)-Math.max(a.height,b.height)>.0001;
  assert.equal(overlap,false,'Solid cubes must never interpenetrate');
 }
}
const base=createBody(),upper=createBody();upper.height=6;releaseBody(upper);const pair=[base,upper];
for(let i=0;i<1800&&pair.some(b=>b.awake);i++){stepWorld(pair,1/120);assertSolid(pair);}
assert.equal(base.height,0);assert.equal(upper.awake,false);assert.ok(Math.abs(upper.height-2.2)<.001);assert.ok(upper.impacts>=2);
const third=createBody();third.height=9;releaseBody(third);const stack=[...pair,third];
for(let i=0;i<1800&&stack.some(b=>b.awake);i++){stepWorld(stack,1/120);assertSolid(stack);}
assert.equal(third.awake,false);assert.ok(Math.abs(third.height-4.4)<.001);
const wall=createBody(),mover=createBody(-4,0);mover.held=true;moveBody(mover,{x:4,y:0,z:0},[wall,mover]);assertSolid([wall,mover]);assert.ok(mover.x<=-2.2+.001,'Fast drag must not tunnel through another cube');
moveBody(mover,{x:-4,y:3,z:0},[wall,mover]);moveBody(mover,{x:0,y:3,z:0},[wall,mover]);moveBody(mover,{x:0,y:0,z:0},[wall,mover]);assertSolid([wall,mover]);assert.ok(Math.abs(mover.height-2.2)<.001,'Dragging downward stops on the other cube');
base.held=true;moveBody(base,{x:6,y:0,z:0},stack);base.held=false;
for(let i=0;i<1800&&stack.some(b=>b.awake);i++){stepWorld(stack,1/120);assertSolid(stack);}
assert.ok(Math.abs(upper.height)<.001,'Unsupported upper cube must fall after its base moves');assert.ok(Math.abs(third.height-2.2)<.001);
console.log('PASS cube contacts, rebound and three-cube stacking; solid swept dragging; support removal wakes the stack');
