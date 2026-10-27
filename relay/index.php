<?php
// index.php v1.5.397
declare(strict_types=1);
const ORIGIN = 'https://oaclis.krprojects.it';
const PRIVATE_DIR = __DIR__ . '/../../oaclis-auth-test-private';
header('Cache-Control: no-store');
header('X-Content-Type-Options: nosniff');
header('Referrer-Policy: no-referrer');
header("Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; manifest-src 'self'; img-src 'self' data:; worker-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
header('Permissions-Policy: publickey-credentials-create=(self), publickey-credentials-get=(self), screen-wake-lock=(self), microphone=(self)');
if (($_SERVER['HTTP_HOST'] ?? '') !== 'oaclis.krprojects.it') { http_response_code(404); exit; }
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    header('Content-Type: application/json');
    if (($_SERVER['HTTP_ORIGIN'] ?? '') !== ORIGIN || !str_starts_with($_SERVER['CONTENT_TYPE'] ?? '', 'application/json') || (int)($_SERVER['CONTENT_LENGTH'] ?? 0) > 131072) { http_response_code(403); exit; }
    umask(0077);
    session_name('__Host-Oaclis');
    session_save_path(PRIVATE_DIR . '/sessions');
    ini_set('session.use_strict_mode', '1');
    ini_set('session.gc_maxlifetime', '7200');
    session_set_cookie_params(['lifetime'=>0, 'path'=>'/', 'secure'=>true, 'httponly'=>true, 'samesite'=>'Strict']);
    $sessionCookie=$_COOKIE[session_name()]??'';
    if (is_string($sessionCookie) && preg_match('/^[A-Za-z0-9,-]{16,128}$/D',$sessionCookie)) session_start();
    $config = require PRIVATE_DIR . '/config.php';
    spl_autoload_register(function ($class) {
        $prefix = 'lbuchs\\WebAuthn\\';
        if (str_starts_with($class, $prefix)) require PRIVATE_DIR . '/library/src/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
    });
    function respond(array $value, int $code=200): never { http_response_code($code); echo json_encode($value, JSON_THROW_ON_ERROR); exit; }
    function readState(): string {
        $path=PRIVATE_DIR.'/state.json';
        for ($attempt=0;$attempt<5;$attempt++) {
            clearstatcache(true,$path);
            $raw=@file_get_contents($path);
            if (is_string($raw) && $raw!=='') return $raw;
            if ($attempt<4) usleep(20000);
        }
        throw new RuntimeException('Relay state temporarily unreadable');
    }
    function decode(string $s): string {
        if (strlen($s)>18000 || !preg_match('/^[A-Za-z0-9_-]+$/D', $s)) throw new RuntimeException();
        $v=base64_decode(strtr($s,'-_','+/'), true);
        if ($v===false) throw new RuntimeException();
        return $v;
    }
    function checkClient(string $encoded): string {
        $raw=decode($encoded); $c=json_decode($raw,true,32,JSON_THROW_ON_ERROR);
        if (($c['origin']??'')!==ORIGIN || ($c['crossOrigin']??false)!==false) throw new RuntimeException();
        return $raw;
    }
    function takeChallenge(string $kind, string $device): string {
        $c=$_SESSION['challenge']??[]; unset($_SESSION['challenge']);
        if (($c['kind']??'')!==$kind || ($c['device']??'')!==$device || ($c['until']??0)<time()) throw new RuntimeException();
        return base64_decode($c['bytes'], true);
    }
    try {
        $body=file_get_contents('php://input',false,null,0,131073);
        if ($body===false || strlen($body)>131072) respond([],413);
        $input=json_decode($body,true,32,JSON_THROW_ON_ERROR);
        $action=$input['action']??'';
        $device=$_SERVER['HTTP_X_DEVICE_CODE']??'';
        $deviceId=preg_match('/^[a-f0-9]{64}$/D',$device)?hash('sha256',$device):'';
        $admin=$_SERVER['HTTP_X_ADMIN_KEY']??'';
        $isAdmin=hash_equals($config['admin_hash'],hash('sha256',$admin));
        $adminActions=['admin-list','chat-access','set-chat-access','invite','approve','revoke','console-exchange'];
        $authenticated=$deviceId!=='' && ($_SESSION['auth']['device']??'')===$deviceId && ($_SESSION['auth']['until']??0)>=time();
        if (!$isAdmin && !$authenticated) {
            // Fixed shards bound disk usage; use the connection address, never forwarded headers.
            $bucket=substr(hash('sha256',$_SERVER['REMOTE_ADDR']??''),0,2);
            $limit=fopen(PRIVATE_DIR.'/rate-'.$bucket,'c+');
            if (!$limit || !flock($limit,LOCK_EX|LOCK_NB)) { header('Retry-After: 1'); http_response_code(429); exit; }
            $window=intdiv(time(),60);
            $usage=explode(' ',trim(stream_get_contents($limit,64)));
            $count=($usage[0]??'')===(string)$window?(int)($usage[1]??0):0;
            if ($count>=60) { flock($limit,LOCK_UN); fclose($limit); header('Retry-After: '.(60-time()%60)); http_response_code(429); exit; }
            rewind($limit); ftruncate($limit,0);
            if (fwrite($limit,$window.' '.($count+1))===false) throw new RuntimeException();
            flock($limit,LOCK_UN); fclose($limit);
        }
        if (!$isAdmin && in_array($action,$adminActions,true)) { http_response_code(403); exit; }
        $db=fopen(PRIVATE_DIR.'/state.lock','c+');
        if (!$db || !flock($db,LOCK_EX)) throw new RuntimeException();
        $raw=readState();
        $state=$raw===''?['invitations'=>[], 'devices'=>[]]:json_decode($raw,true,64,JSON_THROW_ON_ERROR);
        $record=$state['devices'][$deviceId]??null;
        $result=[];
        $web=new \lbuchs\WebAuthn\WebAuthn('Oaclis','oaclis.krprojects.it',['none'],true);
        switch ($action) {
        case 'admin-list':
            if (!$isAdmin) respond([],403);
            if (array_key_exists('app_version',$input)) {
                if (!is_string($input['app_version']) || !preg_match('/^v[0-9]{1,10}\.[0-9]{1,10}\.[0-9]{1,10}$/D',$input['app_version'])) throw new RuntimeException();
                $state['app_version']=$input['app_version'];
            }
            $result['devices']=array_values(array_map(function($id,$r){return ['id'=>$id,'name'=>$r['name'],'status'=>$r['status'],'created'=>$r['created'],'lastAccess'=>$r['lastAccess']??null,'active'=>$r['status']==='approved' && ($r['lastSeen']??0)>=time()-30,'pairing'=>$r['status']==='pending'?($r['pairing']??null):null];},array_keys($state['devices']),$state['devices']));
            break;
        case 'chat-access': case 'set-chat-access':
            if (!$isAdmin) respond([],403);
            $target=$input['id']??'';
            if (!is_string($target) || ($state['devices'][$target]['status']??'')!=='approved') respond([],403);
            if ($action==='chat-access') { $result=['chats'=>$state['devices'][$target]['chats']??[]]; break; }
            $chats=$input['chats']??null;
            if (!is_array($chats) || !array_is_list($chats) || count($chats)>500) throw new RuntimeException();
            $allowed=[];
            foreach ($chats as $chat) {
                if (!is_array($chat) || !is_string($chat['id']??null) || !preg_match('/^[A-Za-z0-9][A-Za-z0-9_.-]{0,159}$/D',$chat['id']) || !is_string($chat['name']??null) || trim($chat['name'])==='' || mb_strlen($chat['name'])>320 || preg_match('/[\x00-\x1f\x7f]/u',$chat['name']) || isset($allowed[$chat['id']])) throw new RuntimeException();
                $icon=$chat['icon']??''; $color=$chat['color']??'';
                if (!is_string($icon) || strlen($icon)>48000 || ($icon!=='' && !preg_match('/^(?:[a-z]+|data:image\/png;base64,iVBORw0KGgo[A-Za-z0-9+\/=]*)$/D',$icon)) || !is_string($color) || ($color!=='' && !preg_match('/^#[a-f0-9]{6}$/iD',$color))) throw new RuntimeException();
                $allowed[$chat['id']]=['id'=>$chat['id'],'name'=>$chat['name'],'icon'=>$icon,'color'=>$color];
            }
            $state['devices'][$target]['chats']=array_values($allowed);
            if (isset($state['devices'][$target]['console']) && !isset($allowed[$state['devices'][$target]['console']['chat_id']])) unset($state['devices'][$target]['console']);
            $result=['ok'=>true];
            break;
        case 'invite':
            if (!$isAdmin) respond([],403);
            $state['invitations']=array_filter($state['invitations'],fn($r)=>$r['until']>time());
            if (count($state['invitations'])>=20) throw new RuntimeException();
            $hash=$input['invite_hash']??'';
            if (!is_string($hash) || !preg_match('/^[a-f0-9]{64}$/D',$hash)) throw new RuntimeException();
            $state['invitations'][$hash]=['until'=>time()+600];
            $result=['ok'=>true];
            break;
        case 'approve': case 'revoke':
            if (!$isAdmin) respond([],403);
            if ($action==='revoke' && ($input['reset_all']??false)===true) {
                $state['devices']=[]; $state['invitations']=[];
                if (session_status()===PHP_SESSION_ACTIVE) session_destroy();
                foreach (glob(PRIVATE_DIR.'/sessions/sess_*')?:[] as $session) {
                    if (is_file($session) && !unlink($session)) throw new RuntimeException();
                }
                $result=['ok'=>true]; break;
            }
            $id=$input['id']??'';
            if (!isset($state['devices'][$id])) throw new RuntimeException();
            if ($action==='revoke' && ($input['forget']??false)===true) {
                unset($state['devices'][$id]); $result=['ok'=>true]; break;
            }
            if ($action==='approve' && $state['devices'][$id]['status']!=='pending') throw new RuntimeException();
            $state['devices'][$id]['status']=$action==='approve'?'approved':'revoked';
            if ($action==='revoke') $state['devices'][$id]['lastSeen']=0;
            if ($action==='revoke') unset($state['devices'][$id]['console']);
            $result=['ok'=>true];
            break;
        case 'register-options':
            $invite=$input['invite']??'';
            if (!is_string($invite) || !preg_match('/^[a-f0-9]{64}$/D',$invite)) respond([],403);
            if (!$deviceId || $record || ($state['invitations'][$invite]['until']??0)<time() || count($state['devices'])>=100) respond([],403);
            $name=trim((string)($input['name']??'Telefono'));
            if ($name==='' || mb_strlen($name)>60) throw new RuntimeException();
            $pairing=$input['pairing']??null;
            if (!is_array($pairing) || ($pairing['invite_hash']??null)!==$invite || !is_string($pairing['payload']??null) || strlen($pairing['payload'])>2048 || !is_string($pairing['proof']??null) || !preg_match('/^[a-f0-9]{64}$/D',$pairing['proof']) || !is_string($pairing['key_proof']??null) || !preg_match('/^[a-f0-9]{128}$/D',$pairing['key_proof'])) respond([],403);
            $binding=json_decode($pairing['payload'],true,8,JSON_THROW_ON_ERROR);
            if (!is_array($binding) || !array_is_list($binding) || count($binding)!==4 || $binding[0]!=='oaclis-pair-v1' || $binding[1]!==$deviceId || $binding[2]!==$name || !is_string($binding[3]) || !preg_match('/^04[a-f0-9]{128}$/D',$binding[3])) respond([],403);
            if (session_status()!==PHP_SESSION_ACTIVE && !session_start()) throw new RuntimeException(error_get_last()['message']??'Unable to open session');
            $args=$web->getCreateArgs(hex2bin($deviceId),$name,$name,60,true,true,false);
            $_SESSION['challenge']=['kind'=>'create','device'=>$deviceId,'until'=>time()+120,'bytes'=>base64_encode($web->getChallenge()->getBinaryString()),'invite'=>$invite,'name'=>$name,'pairing'=>$pairing];
            $result=$args;
            break;
        case 'register':
            $pending=$_SESSION['challenge']??[];
            $challenge=takeChallenge('create',$deviceId);
            $invite=$pending['invite']??'';
            if (!$deviceId || $record || ($state['invitations'][$invite]['until']??0)<time()) respond([],403);
            $data=$web->processCreate(checkClient($input['clientDataJSON']??''),decode($input['attestationObject']??''),$challenge,true,true);
            if (!hash_equals($data->credentialId,decode($input['id']??''))) throw new RuntimeException();
            $state['devices'][$deviceId]=['name'=>$pending['name'],'status'=>'pending','created'=>gmdate('c'),'credential'=>base64_encode($data->credentialId),'key'=>$data->credentialPublicKey,'counter'=>$data->signatureCounter??0,'pairing'=>$pending['pairing']];
            unset($state['invitations'][$invite]);
            $result=['status'=>'pending'];
            break;
        case 'status':
            if (!$record || $record['status']==='revoked') respond([],403);
            $result=['status'=>$record['status'],'name'=>$record['name']];
            break;
        case 'login-options':
            if (!$record || $record['status']!=='approved') respond([],403);
            if (session_status()!==PHP_SESSION_ACTIVE && !session_start()) throw new RuntimeException(error_get_last()['message']??'Unable to open session');
            $args=$web->getGetArgs([base64_decode($record['credential'],true)],60,true,true,true,true,true,true);
            $_SESSION['challenge']=['kind'=>'get','device'=>$deviceId,'until'=>time()+120,'bytes'=>base64_encode($web->getChallenge()->getBinaryString())];
            $result=$args;
            break;
        case 'login':
            $challenge=takeChallenge('get',$deviceId);
            if (!$record || $record['status']!=='approved' || !hash_equals($record['credential'],base64_encode(decode($input['id']??'')))) respond([],403);
            if (!empty($input['userHandle']) && !hash_equals(hex2bin($deviceId),decode($input['userHandle']))) throw new RuntimeException();
            $web->processGet(checkClient($input['clientDataJSON']??''),decode($input['authenticatorData']??''),decode($input['signature']??''),$record['key'],$challenge,$record['counter'],true,true);
            $state['devices'][$deviceId]['counter']=$web->getSignatureCounter()??0;
            $state['devices'][$deviceId]['lastAccess']=gmdate('c');
            $state['devices'][$deviceId]['lastSeen']=time();
            session_regenerate_id(true);
            $_SESSION['auth']=['device'=>$deviceId,'until'=>time()+7200];
            $result=['ok'=>true];
            break;
        case 'console': case 'console-result':
            if (!$record || $record['status']!=='approved' || ($_SESSION['auth']['device']??'')!==$deviceId || ($_SESSION['auth']['until']??0)<time()) respond([],403);
            if (($record['lastSeen']??0)<time()-10) $state['devices'][$deviceId]['lastSeen']=time();
            $chat=$input['chat_id']??'';
            if (!is_string($chat) || !in_array($chat,array_column($record['chats']??[],'id'),true)) respond([],403);
            if ($action==='console-result') {
                $job=$state['devices'][$deviceId]['console']??[];
                if (($job['id']??'')!==($input['request_id']??'') || ($job['chat_id']??'')!==$chat) respond([],403);
                // Wait without holding the session or state lock: the master must publish the result.
                $authUntil=$_SESSION['auth']['until'];
                session_write_close();
                $deadline=microtime(true)+2;
                while (!isset($job['result']) && microtime(true)<$deadline) {
                    flock($db,LOCK_UN);
                    usleep(100000);
                    if (!flock($db,LOCK_EX)) throw new RuntimeException();
                    $raw=readState();
                    $state=json_decode($raw,true,64,JSON_THROW_ON_ERROR);
                    $record=$state['devices'][$deviceId]??null;
                    $job=$record['console']??[];
                    if (!$record || $record['status']!=='approved' || $authUntil<time() ||
                        !in_array($chat,array_column($record['chats']??[],'id'),true) ||
                        ($job['id']??'')!==($input['request_id']??'') || ($job['chat_id']??'')!==$chat) respond([],403);
                }
                if (($record['lastSeen']??0)<time()-10) $state['devices'][$deviceId]['lastSeen']=time();
                $result=isset($job['result'])?$job['result']:['pending'=>true];
                break;
            }
            $operation=$input['operation']??''; $text=$input['text']??''; $key=$input['key']??'';
            if (!in_array($operation,['open','read','input'],true) || !is_string($text) || mb_strlen($text)>8000 || str_contains($text,"\0") || !in_array($key,['','Enter','Escape','Tab','Up','Down','Left','Right','BSpace','C-c'],true)) throw new RuntimeException();
            $signed=$input['signed']??null;
            if (!is_array($signed) || !is_string($signed['payload']??null) || strlen($signed['payload'])>65536 || !is_string($signed['signature']??null) || !preg_match('/^[a-f0-9]{128}$/D',$signed['signature'])) respond([],403);
            $packet=json_decode($signed['payload'],true,16,JSON_THROW_ON_ERROR);
            if (!is_array($packet) || ($packet['purpose']??'')!=='oaclis-console-v1' || ($packet['device']??'')!==$deviceId || ($packet['chat_id']??'')!==$chat || ($packet['action']??'')!==$operation || ($packet['text']??null)!==$text || ($packet['key']??null)!==$key || !is_string($packet['id']??null) || !preg_match('/^[a-f0-9]{32}$/D',$packet['id'])) respond([],403);
            if (!is_bool($packet['speech']??false) || (($packet['speech_cursor']??null)!==null && (!is_int($packet['speech_cursor']) || $packet['speech_cursor']<0))) respond([],403);
            $previous=$record['console']??[];
            if (($previous['until']??0)>time() && !isset($previous['result'])) respond(['error'=>'Attendi il completamento della richiesta.'],409);
            $id=$packet['id'];
            $state['devices'][$deviceId]['console']=['id'=>$id,'chat_id'=>$chat,'until'=>min(time()+20,$_SESSION['auth']['until']),'value'=>['chat_id'=>$chat,'action'=>$operation,'text'=>$text,'key'=>$key],'signed'=>$signed];
            $result=['request_id'=>$id];
            break;
        case 'console-exchange':
            if (!$isAdmin) respond([],403);
            $security=$input['security']??null;
            if (!is_array($security) || !is_string($security['epoch']??null) || !preg_match('/^[a-f0-9]{64}$/D',$security['epoch']) || !is_int($security['time']??null)) throw new RuntimeException();
            $state['console_security']=['epoch'=>$security['epoch']];
            foreach (($input['results']??[]) as $answer) {
                $target=$answer['device']??''; $job=$state['devices'][$target]['console']??[];
                if (($job['id']??'')!==($answer['id']??null) || ($job['until']??0)<time() || ($state['devices'][$target]['status']??'')!=='approved' || !in_array($job['chat_id'],array_column($state['devices'][$target]['chats']??[],'id'),true)) continue;
                $screen=$answer['screen']??null;
                $state['devices'][$target]['console']['result']=is_string($screen)&&strlen($screen)<=60000?['screen'=>$screen]:['error'=>'Console non disponibile. Riprova.'];
                if (is_string($screen)&&strlen($screen)<=60000&&is_string($answer['screen_ansi']??null)&&strlen($answer['screen_ansi'])<=24000) $state['devices'][$target]['console']['result']['screen_ansi']=$answer['screen_ansi'];
                if (is_string($screen)&&is_array($answer['speech']??null)&&strlen(json_encode($answer['speech'],JSON_UNESCAPED_UNICODE))<=10000) $state['devices'][$target]['console']['result']['speech']=$answer['speech'];
            }
            $result=['commands'=>[], 'console_active'=>false];
            foreach ($state['devices'] as $id=>$phone) {
                $job=$phone['console']??[];
                if ($phone['status']==='approved' && ($job['until']??0)>=time()) $result['console_active']=true;
                if ($phone['status']==='approved' && ($job['until']??0)>=time() && !isset($job['result']) && in_array($job['chat_id'],array_column($phone['chats']??[],'id'),true)) {
                    if (!empty($job['claimed'])) continue;
                    $state['devices'][$id]['console']['claimed']=true;
                    $result['commands'][]=['device'=>$id,'id'=>$job['id'],'signed'=>$job['signed']??null];
                    break;
                }
            }
            break;
        case 'protected':
            if (!$record || $record['status']!=='approved' || ($_SESSION['auth']['device']??'')!==$deviceId || ($_SESSION['auth']['until']??0)<time()) respond([],403);
            if (($record['lastSeen']??0)<time()-10) $state['devices'][$deviceId]['lastSeen']=time();
            $result=['message'=>'Accesso autorizzato','name'=>$record['name'],'time'=>gmdate('c'),'chats'=>$record['chats']??[],'version'=>$state['app_version']??''];
            break;
        case 'logout':
            if ($record && ($_SESSION['auth']['device']??'')===$deviceId) $state['devices'][$deviceId]['lastSeen']=0;
            unset($_SESSION['auth'],$_SESSION['challenge'],$state['devices'][$deviceId]['console']);
            $result=['ok'=>true];
            break;
        default: respond([],404);
        }
        if (in_array($action,['protected','console','console-result'],true)) $result['security']=isset($state['console_security']['epoch'])?['epoch'=>$state['console_security']['epoch'],'time'=>time()]:null;
        if ($state!==($raw===''?['invitations'=>[], 'devices'=>[]]:json_decode($raw,true))) {
            $encoded=json_encode($state,JSON_THROW_ON_ERROR|JSON_PRETTY_PRINT);
            $temporary=tempnam(PRIVATE_DIR,'state-');
            if ($temporary===false) throw new RuntimeException();
            try {
                if (file_put_contents($temporary,$encoded)!==strlen($encoded) ||
                    !rename($temporary,PRIVATE_DIR.'/state.json')) throw new RuntimeException();
            } finally { if (is_file($temporary)) unlink($temporary); }
        }
        flock($db,LOCK_UN); fclose($db);
        echo json_encode($result,JSON_THROW_ON_ERROR|JSON_UNESCAPED_UNICODE|JSON_UNESCAPED_SLASHES);
    } catch (Throwable $e) { @file_put_contents(PRIVATE_DIR.'/last-error.json',json_encode(['action'=>is_string($action??null)?substr($action,0,32):'','reason'=>substr($e->getMessage(),0,240),'line'=>$e->getLine(),'time'=>gmdate('c')]),LOCK_EX); error_log('Oaclis auth test: '.$e->getMessage()); http_response_code(400); echo '{"error":"Verifica non riuscita. Riprova."}'; }
    exit;
}
if ($_SERVER['REQUEST_METHOD']!=='GET') {http_response_code(405);exit;}
?>
<!doctype html>
<html lang="it"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover,interactive-widget=resizes-content"><meta name="robots" content="noindex,nofollow"><title>Oaclis</title><meta name="theme-color" content="#111314"><link rel="manifest" href="/manifest.webmanifest"><link rel="icon" type="image/png" href="/icon-192.png"><link rel="apple-touch-icon" href="/icon-192.png"><link rel="stylesheet" href="/style.css?v=<?= hash_file('sha256', __DIR__.'/style.css') ?>"><script defer src="/locale.js?v=<?= hash_file('sha256', __DIR__.'/locale.js') ?>"></script><script defer src="/icons.js?v=<?= hash_file('sha256', __DIR__.'/icons.js') ?>"></script><script defer src="/voice.js?v=<?= hash_file('sha256', __DIR__.'/voice.js') ?>"></script><script defer src="/security.js?v=<?= hash_file('sha256', __DIR__.'/security.js') ?>"></script><script defer src="/terminal-format.js?v=<?= hash_file('sha256', __DIR__.'/terminal-format.js') ?>"></script><script defer src="/read-aloud.js?v=<?= hash_file('sha256', __DIR__.'/read-aloud.js') ?>"></script><script defer src="/app.js?v=<?= hash_file('sha256', __DIR__.'/app.js') ?>"></script></head>
<body><main id="app" hidden aria-live="polite"></main></body></html>
