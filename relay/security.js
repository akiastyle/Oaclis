// security.js v1.8.58
'use strict';
const OaclisMobileSecurity=(()=>{
 let database,keyCache,clock;
 const bytes=new TextEncoder();
 function hex(value){return [...new Uint8Array(value)].map(byte=>byte.toString(16).padStart(2,'0')).join('');}
 function failure(status=503){return Object.assign(new Error(OaclisSettings.text(status===403?'Accesso non autorizzato.':'Collegamento temporaneamente non disponibile. Riprova.')),{status});}
 function db(){
  if(!database)database=new Promise((resolve,reject)=>{
   const request=indexedDB.open('oaclis-device',1);
   request.onupgradeneeded=()=>request.result.createObjectStore('keys');
   request.onsuccess=()=>resolve(request.result);
   request.onerror=request.onblocked=()=>reject(failure());
  }).catch(error=>{database=null;throw error;});
  return database;
 }
 async function deviceKey(code,create=false){
  if(keyCache?.code===code)return keyCache.promise;
  const promise=(async()=>{
   const database=await db();
   const saved=await new Promise((resolve,reject)=>{const request=database.transaction('keys').objectStore('keys').get('current');request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(failure());});
   if(saved?.code===code&&saved.privateKey?.type==='private'&&saved.privateKey.extractable===false)return saved;
   if(!create)return null;
   const pair=await crypto.subtle.generateKey({name:'ECDSA',namedCurve:'P-256'},false,['sign','verify']);
   const entry={code,privateKey:pair.privateKey,publicKey:hex(await crypto.subtle.exportKey('raw',pair.publicKey)),device:hex(await crypto.subtle.digest('SHA-256',bytes.encode(code)))};
   await new Promise((resolve,reject)=>{const transaction=database.transaction('keys','readwrite');transaction.objectStore('keys').put(entry,'current');transaction.oncomplete=resolve;transaction.onerror=transaction.onabort=()=>reject(failure());});
   return entry;
  })();
  keyCache={code,promise};
  try{const key=await promise;if(!key)keyCache=null;return key;}catch(error){keyCache=null;throw error;}
 }
 async function pairing(secret,code,name){
  const key=await deviceKey(code,true);
  const payload=JSON.stringify(['oaclis-pair-v1',key.device,name,key.publicKey]);
  const proofKey=await crypto.subtle.importKey('raw',bytes.encode(secret),{name:'HMAC',hash:'SHA-256'},false,['sign']);
  return {invite_hash:hex(await crypto.subtle.digest('SHA-256',bytes.encode(secret))),payload,
   proof:hex(await crypto.subtle.sign('HMAC',proofKey,bytes.encode(payload))),
   key_proof:hex(await crypto.subtle.sign({name:'ECDSA',hash:'SHA-256'},key.privateKey,bytes.encode(payload)))};
 }
 function update(security){
  if(security&&/^[a-f0-9]{64}$/.test(security.epoch)&&Number.isSafeInteger(security.time)&&security.time>0)clock={...security,received:Date.now()};
 }
 async function sign(code,{chat_id,operation,text='',key='',speech=false,speech_cursor=null}){
  const identity=await deviceKey(code);
  if(!identity)throw failure(403);
  if(!clock)throw failure();
  const id=hex(crypto.getRandomValues(new Uint8Array(16)));
  const payload=JSON.stringify({purpose:'oaclis-console-v1',epoch:clock.epoch,device:identity.device,id,
   issued:clock.time+Math.floor((Date.now()-clock.received)/1000),chat_id,action:operation,text,key,speech,speech_cursor});
  const signature=hex(await crypto.subtle.sign({name:'ECDSA',hash:'SHA-256'},identity.privateKey,bytes.encode(payload)));
  return {payload,signature};
 }
 async function hasKey(code){return !!(await deviceKey(code));}
 return Object.freeze({pairing,update,sign,hasKey});
})();
