<?php
// legacy.php v1.0.8
// Old invitations keep their URL fragment when the browser follows this redirect.
if ($_SERVER['REQUEST_METHOD']==='GET') {
    header('Cache-Control: no-store');
    header('Location: https://www.krprojects.it/oaclis/',true,302);
    exit;
}
require __DIR__.'/../oaclis/index.php';
