// sw.js v1.0.14
const AUDIO_CACHE='oaclis-feedback-v2',AUDIO_URL=new URL('/feedback.mp3',self.location.origin).href;
self.addEventListener('install',event=>event.waitUntil(caches.open(AUDIO_CACHE).then(cache=>cache.add(new Request(AUDIO_URL,{cache:'reload'}))).then(()=>self.skipWaiting())));
self.addEventListener('activate',event=>event.waitUntil(caches.delete('oaclis-feedback-v1').then(()=>self.clients.claim())));
self.addEventListener('fetch',event=>event.respondWith(event.request.method==='GET'&&event.request.url===AUDIO_URL?caches.open(AUDIO_CACHE).then(cache=>cache.match(AUDIO_URL)).then(cached=>cached||fetch(event.request)):fetch(event.request)));
