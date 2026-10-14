// test_activity.cjs v1.1.22
const fs=require('fs'),vm=require('vm'),assert=require('assert');
const s=fs.readFileSync('ui/panels.js','utf8');
const c=vm.createContext({data:{analyses:{},transfers:{},job:{status:'idle'}},Date});
vm.runInContext(s.slice(s.indexOf('function activityState'),s.indexOf('function activityIndicator')),c);
const state=(scope)=>vm.runInContext(`activityState({id:'chat'},'master','${scope}').state`,c);
assert.equal(state('all'),'idle');
c.data.analyses['master:chat']={status:'pending'};assert.equal(state('paths'),'busy');assert.equal(state('chat'),'idle');
c.data.analyses['master:chat']={status:'ready'};assert.equal(state('paths'),'idle');
c.data.job={status:'running',chat_id:'chat',source_device_id:'master',scopes:{chat:true,paths:false,apps:false}};
assert.equal(state('chat'),'busy');assert.equal(state('apps'),'idle');assert.equal(state('all'),'busy');
c.data.job={status:'idle'};c.data.transfers['master:chat']={status:'failed',chat_id:'chat',source_device_id:'master',scopes:{chat:true}};
assert.equal(state('chat'),'idle');assert.equal(state('all'),'idle');
c.data.transfers['master:chat'].status='completed';assert.notEqual(state('chat'),'busy');
console.log('Activity states: OK');
