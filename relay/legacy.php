<?php
// legacy.php v1.0.12
// Old invitations keep their URL fragment when the browser follows this redirect.
if ($_SERVER['REQUEST_METHOD']==='GET') {
    header('Cache-Control: no-store');
    header('Location: https://oaclis.krprojects.it/',true,302);
    exit;
}
http_response_code(410);
