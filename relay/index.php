<?php
// index.php v1.4.229
declare(strict_types=1);
const ORIGIN = 'https://www.krprojects.it';
const PRIVATE_DIR = __DIR__ . '/../../oaclis-auth-test-private';
header('Cache-Control: no-store');
header('X-Content-Type-Options: nosniff');
header('Referrer-Policy: no-referrer');
header("Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'");
header('Permissions-Policy: publickey-credentials-create=(self), publickey-credentials-get=(self), screen-wake-lock=(self)');
if (($_SERVER['HTTP_HOST'] ?? '') !== 'www.krprojects.it') { http_response_code(404); exit; }
if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    header('Content-Type: application/json');
    if (($_SERVER['HTTP_ORIGIN'] ?? '') !== ORIGIN || !str_starts_with($_SERVER['CONTENT_TYPE'] ?? '', 'application/json') || (int)($_SERVER['CONTENT_LENGTH'] ?? 0) > 131072) { http_response_code(403); exit; }
    umask(0077);
    session_name('OaclisAuthTest');
    session_save_path(PRIVATE_DIR . '/sessions');
    ini_set('session.use_strict_mode', '1');
    ini_set('session.gc_maxlifetime', '7200');
    session_set_cookie_params(['lifetime'=>0, 'path'=>'/oaclis/', 'secure'=>true, 'httponly'=>true, 'samesite'=>'Strict']);
    session_start();
    $config = require PRIVATE_DIR . '/config.php';
    spl_autoload_register(function ($class) {
        $prefix = 'lbuchs\\WebAuthn\\';
        if (str_starts_with($class, $prefix)) require PRIVATE_DIR . '/library/src/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
    });
    function respond(array $value, int $code=200): never { http_response_code($code); echo json_encode($value, JSON_THROW_ON_ERROR); exit; }
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
        $input=json_decode(file_get_contents('php://input'),true,32,JSON_THROW_ON_ERROR);
        $action=$input['action']??'';
        $device=$_SERVER['HTTP_X_DEVICE_CODE']??'';
        $deviceId=preg_match('/^[a-f0-9]{64}$/D',$device)?hash('sha256',$device):'';
        $admin=$_SERVER['HTTP_X_ADMIN_KEY']??'';
        $isAdmin=hash_equals($config['admin_hash'],hash('sha256',$admin));
        $db=fopen(PRIVATE_DIR.'/state.json','c+');
        if (!$db || !flock($db,LOCK_EX)) throw new RuntimeException();
        $raw=stream_get_contents($db);
        $state=$raw===''?['invitations'=>[], 'devices'=>[]]:json_decode($raw,true,64,JSON_THROW_ON_ERROR);
        $record=$state['devices'][$deviceId]??null;
        $result=[];
        $web=new \lbuchs\WebAuthn\WebAuthn('Oaclis','www.krprojects.it',['none'],true);
        switch ($action) {
        case 'admin-list':
            if (!$isAdmin) respond([],403);
            $result['devices']=array_values(array_map(function($id,$r){return ['id'=>$id,'name'=>$r['name'],'status'=>$r['status'],'created'=>$r['created'],'lastAccess'=>$r['lastAccess']??null,'active'=>$r['status']==='approved' && ($r['lastSeen']??0)>=time()-30];},array_keys($state['devices']),$state['devices']));
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
                $allowed[$chat['id']]=['id'=>$chat['id'],'name'=>$chat['name']];
            }
            $state['devices'][$target]['chats']=array_values($allowed);
            if (isset($state['devices'][$target]['console']) && !isset($allowed[$state['devices'][$target]['console']['chat_id']])) unset($state['devices'][$target]['console']);
            $result=['ok'=>true];
            break;
        case 'invite':
            if (!$isAdmin) respond([],403);
            $state['invitations']=array_filter($state['invitations'],fn($r)=>$r['until']>time());
            if (count($state['invitations'])>=20) throw new RuntimeException();
            $token=bin2hex(random_bytes(32));
            $state['invitations'][hash('sha256',$token)]=['until'=>time()+600];
            $result=['url'=>ORIGIN.'/oaclis/auth#pair='.$token];
            break;
        case 'approve': case 'revoke':
            if (!$isAdmin) respond([],403);
            $id=$input['id']??'';
            if (!isset($state['devices'][$id])) throw new RuntimeException();
            if ($action==='approve' && $state['devices'][$id]['status']!=='pending') throw new RuntimeException();
            $state['devices'][$id]['status']=$action==='approve'?'approved':'revoked';
            if ($action==='revoke') $state['devices'][$id]['lastSeen']=0;
            if ($action==='revoke') unset($state['devices'][$id]['console']);
            $result=['ok'=>true];
            break;
        case 'register-options':
            $invite=hash('sha256',$input['invite']??'');
            if (!$deviceId || $record || ($state['invitations'][$invite]['until']??0)<time() || count($state['devices'])>=100) respond([],403);
            $name=trim((string)($input['name']??'Telefono'));
            if ($name==='' || mb_strlen($name)>60) throw new RuntimeException();
            $args=$web->getCreateArgs(hex2bin($deviceId),$name,$name,60,true,true,false);
            $_SESSION['challenge']=['kind'=>'create','device'=>$deviceId,'until'=>time()+120,'bytes'=>base64_encode($web->getChallenge()->getBinaryString()),'invite'=>$invite,'name'=>$name];
            $result=$args;
            break;
        case 'register':
            $pending=$_SESSION['challenge']??[];
            $challenge=takeChallenge('create',$deviceId);
            $invite=$pending['invite']??'';
            if (!$deviceId || $record || ($state['invitations'][$invite]['until']??0)<time()) respond([],403);
            $data=$web->processCreate(checkClient($input['clientDataJSON']??''),decode($input['attestationObject']??''),$challenge,true,true);
            if (!hash_equals($data->credentialId,decode($input['id']??''))) throw new RuntimeException();
            $state['devices'][$deviceId]=['name'=>$pending['name'],'status'=>'pending','created'=>gmdate('c'),'credential'=>base64_encode($data->credentialId),'key'=>$data->credentialPublicKey,'counter'=>$data->signatureCounter??0];
            unset($state['invitations'][$invite]);
            $result=['status'=>'pending'];
            break;
        case 'status':
            if (!$record || $record['status']==='revoked') respond([],403);
            $result=['status'=>$record['status'],'name'=>$record['name']];
            break;
        case 'login-options':
            if (!$record || $record['status']!=='approved') respond([],403);
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
                $result=isset($job['result'])?$job['result']:['pending'=>true];
                break;
            }
            $operation=$input['operation']??''; $text=$input['text']??''; $key=$input['key']??'';
            if (!in_array($operation,['open','read','input'],true) || !is_string($text) || mb_strlen($text)>8000 || str_contains($text,"\0") || !in_array($key,['','Enter','Escape','Tab','Up','Down','Left','Right','BSpace','C-c'],true)) throw new RuntimeException();
            $previous=$record['console']??[];
            if (($previous['until']??0)>time() && !isset($previous['result'])) respond(['error'=>'Attendi il completamento della richiesta.'],409);
            $id=bin2hex(random_bytes(16));
            $state['devices'][$deviceId]['console']=['id'=>$id,'chat_id'=>$chat,'until'=>min(time()+20,$_SESSION['auth']['until']),'value'=>['chat_id'=>$chat,'action'=>$operation,'text'=>$text,'key'=>$key]];
            $result=['request_id'=>$id];
            break;
        case 'console-exchange':
            if (!$isAdmin) respond([],403);
            foreach (($input['results']??[]) as $answer) {
                $target=$answer['device']??''; $job=$state['devices'][$target]['console']??[];
                if (($job['id']??'')!==($answer['id']??null) || ($job['until']??0)<time() || ($state['devices'][$target]['status']??'')!=='approved' || !in_array($job['chat_id'],array_column($state['devices'][$target]['chats']??[],'id'),true)) continue;
                $screen=$answer['screen']??null;
                $state['devices'][$target]['console']['result']=is_string($screen)&&strlen($screen)<=60000?['screen'=>$screen]:['error'=>'Console non disponibile. Riprova.'];
            }
            $result=['commands'=>[]];
            foreach ($state['devices'] as $id=>$phone) {
                $job=$phone['console']??[];
                if ($phone['status']==='approved' && ($job['until']??0)>=time() && !isset($job['result']) && in_array($job['chat_id'],array_column($phone['chats']??[],'id'),true)) {
                    if (!empty($job['claimed'])) continue;
                    $state['devices'][$id]['console']['claimed']=true;
                    $result['commands'][]=['device'=>$id,'id'=>$job['id'],'value'=>$job['value']];
                    break;
                }
            }
            break;
        case 'protected':
            if (!$record || $record['status']!=='approved' || ($_SESSION['auth']['device']??'')!==$deviceId || ($_SESSION['auth']['until']??0)<time()) respond([],403);
            if (($record['lastSeen']??0)<time()-10) $state['devices'][$deviceId]['lastSeen']=time();
            $result=['message'=>'Accesso autorizzato','name'=>$record['name'],'time'=>gmdate('c'),'chats'=>$record['chats']??[]];
            break;
        case 'logout':
            if ($record && ($_SESSION['auth']['device']??'')===$deviceId) $state['devices'][$deviceId]['lastSeen']=0;
            unset($_SESSION['auth'],$_SESSION['challenge'],$state['devices'][$deviceId]['console']);
            $result=['ok'=>true];
            break;
        default: respond([],404);
        }
        if ($state!==($raw===''?['invitations'=>[], 'devices'=>[]]:json_decode($raw,true))) {
            $encoded=json_encode($state,JSON_THROW_ON_ERROR|JSON_PRETTY_PRINT);
            rewind($db);
            if (fwrite($db,$encoded)!==strlen($encoded)) throw new RuntimeException();
            ftruncate($db,strlen($encoded)); fflush($db);
        }
        flock($db,LOCK_UN); fclose($db);
        echo json_encode($result,JSON_THROW_ON_ERROR|JSON_UNESCAPED_UNICODE|JSON_UNESCAPED_SLASHES);
    } catch (Throwable $e) { error_log('Oaclis auth test: '.$e->getMessage()); http_response_code(400); echo '{"error":"Verifica non riuscita. Riprova."}'; }
    exit;
}
if ($_SERVER['REQUEST_METHOD']!=='GET') {http_response_code(405);exit;}
?>
<!doctype html>
<html lang="it"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover,interactive-widget=resizes-content"><meta name="robots" content="noindex,nofollow"><title>Oaclis</title><link rel="stylesheet" href="/oaclis/style.css?v=<?= hash_file('sha256', __DIR__.'/style.css') ?>"><script defer src="/oaclis/app.js?v=<?= hash_file('sha256', __DIR__.'/app.js') ?>"></script></head>
<body><main id="app" hidden aria-live="polite"></main></body></html>
