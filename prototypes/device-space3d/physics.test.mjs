// physics.test.mjs v1.0.2
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
