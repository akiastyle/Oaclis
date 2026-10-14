// web.rs v1.20.992
//! Browser UI adapter. Transfer decisions remain in the existing core modules.
use crate::{
    devices::{self, Device},
    environment, invalid,
    rules::{self, Rule},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};
use tiny_http::{Header, Method, Request, Response, Server};

fn association(value: &Value, devices: &[Device]) -> io::Result<(String, Value)> {
    let source = value["source"]
        .as_str()
        .ok_or_else(|| invalid("Sorgente richiesta"))?;
    let target = value["target"]
        .as_str()
        .ok_or_else(|| invalid("Destinazione richiesta"))?;
    let operation = value["operation"]
        .as_str()
        .ok_or_else(|| invalid("Direzione richiesta"))?;
    let id = value["entry"]["id"]
        .as_str()
        .ok_or_else(|| invalid("Chat richiesta"))?;
    let name = value["entry"]["name"]
        .as_str()
        .ok_or_else(|| invalid("Titolo richiesto"))?;
    let known = |id: &str| id == "master" || devices.iter().any(|d| d.id == id);
    if !known(source)
        || !known(target)
        || source == target
        || (source != "master" && target != "master")
        || !devices::identifier(id)
        || !matches!(operation, "copy" | "send" | "sync")
        || name.chars().count() > 320
        || name.chars().any(char::is_control)
    {
        return Err(invalid("Associazione chat/dispositivi non valida"));
    }
    let label = value["label"].as_str().unwrap_or("");
    if label.chars().count() > 160 || label.chars().any(char::is_control) {
        return Err(invalid("Nome collegamento non valido"));
    }
    let scopes: environment::Scopes = serde_json::from_value(
        value
            .get("scopes")
            .cloned()
            .unwrap_or_else(|| json!({"chat":true,"paths":true,"apps":true})),
    )
    .map_err(io::Error::other)?;
    let mut pair = [source, target];
    pair.sort();
    Ok((
        format!("{}:{}:{}", id, pair[0], pair[1]),
        json!({"entry":{"id":id,"name":name},"source":source,"target":target,"operation":operation,"label":label,"scopes":scopes,"transfer_pending":value["transfer_pending"] == true}),
    ))
}

const CATALOG: &str = include_str!("../tools/chat_catalog.py");
fn catalog(root: &Path) -> io::Result<Vec<Value>> {
    let output = std::process::Command::new("python3")
        .args(["-c", CATALOG])
        .arg(root)
        .output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("Impossibile leggere il catalogo Codex: {}", detail.lines().last().unwrap_or("errore catalogo"))));
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    serde_json::from_value(value["chats"].clone()).map_err(io::Error::other)
}
// Each device analyzes its own archives serially; unchanged failures are not retried.
fn background_analysis(codex: PathBuf, config: PathBuf, active: Arc<Mutex<std::collections::HashSet<String>>>) {
    thread::spawn(move || loop {
        if let Ok(chats) = catalog(&codex) {
            for chat in chats {
                let Some(id) = chat["id"].as_str() else { continue; };
                let Ok(key) = environment::key("master", id) else { continue; };
                let revision = environment::revision(&chat, None);
                let cached = environment::database(&config, &key, "analysis-get", None).unwrap_or(Value::Null);
                let auth_revision = fs::metadata(codex.join("auth.json"))
                    .and_then(|metadata| metadata.modified())
                    .map(|time| format!("{time:?}"))
                    .unwrap_or_default();
                if matches!(cached["status"].as_str(), Some("ready" | "failed")) { continue; }
                {
                    let mut running = active.lock().unwrap();
                    if running.len() >= 2 || !running.insert(key.clone()) { continue; }
                }
                let _ = environment::database(&config, &key, "analysis-save", Some(&json!({"status":"pending", "revision":revision})));
                let result = match environment::analyze(None, chat["path"].as_str().unwrap_or(""), id) {
                    Ok(manifest) => {
                        if let Err(error) = crate::preparation::capture(&config,id,Path::new(chat["path"].as_str().unwrap_or("")),&manifest) { eprintln!("Oaclis: dimensioni sorgenti: {error}"); }
                        json!({"status":"ready", "revision":revision, "manifest":manifest, "chat_bytes":chat["bytes"], "chat_modified_ns":chat["modified_ns"]})
                    },
                    Err(error) => json!({"status":"failed", "revision":revision, "error":error.to_string(), "auth_revision":auth_revision}),
                };
                let _ = environment::database(&config, &key, "analysis-save", Some(&result));
                active.lock().unwrap().remove(&key);
            }
        }
        thread::sleep(std::time::Duration::from_secs(60));
    });
}
fn remote_catalog(device: &Device) -> io::Result<Value> {
    let command = format!("python3 -c {}", crate::ssh::quote(CATALOG));
    serde_json::from_slice(&crate::ssh::run(device, &command)?).map_err(io::Error::other)
}
fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).unwrap()
}
fn error_status(error: &io::Error) -> u16 {
    match error.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => 400,
        _ => 503,
    }
}
fn answer(request: Request, status: u16, body: String, content_type: &str) {
    answer_bytes(request, status, body.into_bytes(), content_type);
}
fn answer_bytes(request: Request, status: u16, body: Vec<u8>, content_type: &str) {
    let response = Response::from_data(body).with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
    let _ = request.respond(response);
}
fn local_host(host: &str) -> bool {
    let Some((name, port)) = host.rsplit_once(':') else { return false; };
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
        && port.parse::<u16>().is_ok_and(|port| port > 0)
}
fn trusted(request: &Request) -> bool {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Host"))
        .is_some_and(|h| local_host(h.value.as_str()))
}
fn body(request: &mut Request) -> io::Result<Value> {
    let mut data = Vec::new();
    request.as_reader().take(65537).read_to_end(&mut data)?;
    if data.len() > 65536 {
        return Err(invalid("Richiesta troppo grande"));
    }
    serde_json::from_slice(&data).map_err(|error| invalid(error.to_string()))
}
fn ssh_settings(home: &Path, config: &Path, operation: &str, value: &Value) -> io::Result<Value> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("python3").args(["-c", include_str!("../tools/ssh_settings.py")])
        .arg(home).arg(config).arg(operation).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?)?;
    let output = child.wait_with_output()?;
    let result: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    if !output.status.success() { return Err(invalid(result["error"].as_str().unwrap_or("Configurazione SSH non riuscita"))); }
    Ok(result)
}

pub(crate) fn start_transfer(v: Value, codex: &Path, config: &Path, job: &Arc<Mutex<Value>>) -> io::Result<Value> {
    let ds = devices::load(config.join("devices.json"))?;
    let rule = if let Some(id) = v["rule_id"].as_str() {
        Some(
            rules::load(config.join("rules.json"))?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or_else(|| invalid("Regola non trovata"))?,
        )
    } else {
        None
    };
    let incoming = v["source_device_id"]
        .as_str()
        .is_some_and(|id| id != "master");
    if let Some(operation) = v["operation"].as_str()
        && operation != "copy"
        && operation != "sync"
        && operation != "send"
    {
        return Err(invalid("Operazione non valida"));
    }
    if incoming && v["device_id"] != "master" {
        return Err(invalid("Il destinatario deve essere questo dispositivo"));
    }
    let synchronize = v["operation"] == "sync" || v["operation"] == "send";
    let direction = if v["operation"] == "send" {
        if incoming {
            crate::sync::Direction::Receive
        } else {
            crate::sync::Direction::Send
        }
    } else {
        crate::sync::Direction::Both
    };
    let device_id = rule
        .as_ref()
        .map(|r| r.device_id.as_str())
        .or_else(|| {
            if incoming {
                v["source_device_id"].as_str()
            } else {
                v["device_id"].as_str()
            }
        })
        .ok_or_else(|| invalid("Dispositivo richiesto"))?;
    let device = ds
        .into_iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| invalid("Dispositivo non trovato"))?;
    let chat = if rule.is_none() {
        Some(
            (if incoming {
                serde_json::from_value::<Vec<Value>>(
                    remote_catalog(&device)?["chats"].clone(),
                )
                .map_err(io::Error::other)?
            } else {
                catalog(codex)?
            })
            .into_iter()
            .find(|c| c["id"] == v["chat_id"])
            .ok_or_else(|| invalid("Chat non trovata"))?,
        )
    } else {
        None
    };
    let scopes: environment::Scopes = v
        .get("scopes")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_default();
    if rule.is_none() && !scopes.any() {
        return Err(invalid("Seleziona almeno una categoria"));
    }
    let manifest = if let Some(chat) = &chat {
        let source = v["source_device_id"].as_str().unwrap_or("master");
        let key = environment::key(source, chat["id"].as_str().unwrap())?;
        let cached = environment::database(&config, &key, "analysis-get", None)?;
        if cached["status"] != "ready" {
            return Err(invalid("Attendi il completamento dell’analisi"));
        }
        if cached["revision"]
            != environment::revision(
                chat,
                if incoming { Some(&device) } else { None },
            )
        {
            return Err(invalid("La chat è aggiornata: ripeti l’analisi"));
        }
        let manifest: environment::Manifest =
            serde_json::from_value(cached["manifest"].clone())
                .map_err(io::Error::other)?;
        manifest.validate()?;
        if !environment::fresh(
            if incoming { Some(&device) } else { None },
            &manifest,
        )? {
            return Err(invalid("I requisiti sono aggiornati: ripeti l’analisi"));
        }
        Some(manifest)
    } else {
        None
    };
    let remote = if let Some(chat) = &chat
        && scopes.chat
    {
        let target = if incoming {
            json!({"root":codex,"chats":catalog(codex)?})
        } else {
            remote_catalog(&device)?
        };
        let existing = target["chats"]
            .as_array()
            .and_then(|chats| chats.iter().find(|c| c["id"] == chat["id"]))
            .and_then(|c| c["path"].as_str());
        let p =
            if let Some(existing) = existing {
                PathBuf::from(existing)
            } else {
                let relative = Path::new(
                    chat["relative_path"]
                        .as_str()
                        .ok_or_else(|| invalid("Percorso chat mancante"))?,
                );
                if !relative.starts_with("sessions")
                    || !relative
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
                {
                    return Err(invalid("Percorso chat non valido"));
                }
                PathBuf::from(target["root"].as_str().ok_or_else(|| {
                    invalid("Cartella Codex destinataria mancante")
                })?)
                .join(relative)
            };
        crate::paths::validate_absolute(&p)?;
        Some(p)
    } else {
        None
    };
    let mut lock = job.lock().unwrap();
    if lock["status"] == "running" {
        return Err(invalid("Un trasferimento è già in corso"));
    }
    *lock = json!({"status":"running","message":"Preparazione","progress":null,"chat_id":v["chat_id"],"source_device_id":v["source_device_id"].as_str().unwrap_or("master"),"scopes":scopes,"device_id":v["device_id"],
        "title":chat.as_ref().map(|c|c["name"].clone()).unwrap_or_else(||json!(rule.as_ref().unwrap().name)),
        "source":if incoming {device.name.as_str()}else{"Questo dispositivo"},
        "destination":if incoming {"Questo dispositivo"}else{device.name.as_str()}});
    let transfer_key = chat.as_ref().map(|c| environment::key(v["source_device_id"].as_str().unwrap_or("master"), c["id"].as_str().unwrap())).transpose()?;
    if let Some(key) = &transfer_key { environment::database(&config, key, "transfer-save", Some(&lock))?; }
    drop(lock);
    let transfer_config = config.to_path_buf();
    let job = job.clone();
    let state = config.join("state");
    thread::spawn(move || {
        let result = (|| -> io::Result<Value> {
            if let Some(rule) = rule {
                let results = rules::execute(
                    &device,
                    &rule,
                    &state.join(&rule.id),
                    |event| job.lock().unwrap()["progress"] = json!(event),
                )?;
                Ok(json!(results))
            } else {
                let chat = chat.unwrap();
                fs::create_dir_all(&state)?;
                let environment_results = environment::execute(
                    &device,
                    &manifest.unwrap(),
                    scopes,
                    incoming,
                    v["operation"].as_str().unwrap_or("copy"),
                    &state.join(format!(
                        "environment-{}-{}",
                        device.id,
                        chat["id"].as_str().unwrap()
                    )),
                    |progress| job.lock().unwrap()["progress"] = progress,
                )?;
                if !scopes.chat {
                    return Ok(json!({"environment":environment_results}));
                }
                let source = Path::new(chat["path"].as_str().unwrap());
                if incoming || synchronize {
                    let destination = remote.unwrap();
                    let (local, remote) = if incoming {
                        (destination, source.to_path_buf())
                    } else {
                        (source.to_path_buf(), destination)
                    };
                    if incoming && !synchronize && local.try_exists()? {
                        return Err(invalid(
                            "La copia locale esiste già; nessuna sovrascrittura",
                        ));
                    }
                    if incoming {
                        crate::rules::ensure_local(local.parent().ok_or_else(
                            || invalid("Cartella destinataria mancante"),
                        )?)?;
                    }
                    let checkpoint = state.join(format!(
                        "chat-{}-{}.json",
                        device.id,
                        chat["id"].as_str().unwrap()
                    ));
                    let outcome = crate::sync::synchronize_with_direction(
                        &device,
                        &local,
                        &remote,
                        &checkpoint,
                        crate::sync::Kind::Chat,
                        direction,
                        |event| {
                            job.lock().unwrap()["progress"] = json!({"event":event})
                        },
                    )?;
                    return Ok(
                        json!({"chat":outcome,"environment":environment_results}),
                    );
                }
                crate::chat::Chat::read(source)?;
                rules::ensure_remote(
                    &device,
                    remote
                        .as_ref()
                        .unwrap()
                        .parent()
                        .ok_or_else(|| invalid("Cartella destinataria mancante"))?,
                )?;
                let receipt = crate::transfer::send_file_with_progress(
                    &device,
                    source,
                    remote.unwrap(),
                    |event| {
                        job.lock().unwrap()["progress"] = json!({"event":event})
                    },
                )?;
                Ok(json!({"chat":receipt,"environment":environment_results}))
            }
        })();
        let mut lock = job.lock().unwrap();
        match result {
            Ok(results) => {
                lock["status"] = json!("completed");
                lock["results"] = results;
            }
            Err(e) => {
                lock["status"] = json!("failed");
                lock["message"] = json!(e.to_string());
            }
        }
        if let Some(key) = &transfer_key { let _ = environment::database(&transfer_config, key, "transfer-save", Some(&lock)); }
    });
    Ok(json!({"ok":true}))
}

fn serve_requests(server: &Server, handler: impl Fn(Request) + Sync) {
    thread::scope(|scope| {
        for _ in 0..4 {
            let handler = &handler;
            scope.spawn(move || {
                while let Ok(request) = server.recv() { handler(request); }
            });
        }
    });
}

pub fn serve(home: PathBuf) -> io::Result<()> {
    let paused = std::env::var("OACLIS_PAUSED").as_deref() == Ok("1");
    let config = home.join(".config/oaclis");
    fs::create_dir_all(&config)?;
    environment::database(&config, "", "migrate", None)?;
    crate::app_inventory::recover_interrupted(&config)?;
    let codex = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let role = match fs::read_to_string(config.join("role")) {
        Ok(value) => value.trim().to_owned(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => "master".into(),
        Err(error) => return Err(error),
    };
    if !matches!(role.as_str(), "master" | "client") { return Err(invalid("Ruolo Oaclis non valido")); }
    let brand_icon = if role == "master" { "oaclis-crown" } else { "oaclis-whale" };
    let mut random = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
    let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let server = Server::http("0.0.0.0:23232").map_err(io::Error::other)?;
    let peers = crate::peers::start(home.clone());
    let saved_transfers = environment::database(&config, "", "transfer-list", None)?;
    if let Some(records) = saved_transfers.as_object() {
        for (key, record) in records {
            if !paused && record["status"] == "running" {
                let mut interrupted = record.clone();
                interrupted["status"] = json!("failed");
                interrupted["message"] = json!("Trasferimento interrotto dal riavvio di Oaclis");
                environment::database(&config, key, "transfer-save", Some(&interrupted))?;
            }
        }
    }
    let job = Arc::new(Mutex::new(json!({"status":"idle"})));
    let connections = Arc::new(Mutex::new(json!({})));
    let analyses = Arc::new(Mutex::new(std::collections::HashSet::<String>::new()));
    if !paused {
        background_analysis(codex.clone(), config.clone(), analyses.clone());
        crate::shared_settings::start(home.clone());
        crate::automation::start(codex.clone(), config.clone(), job.clone(), connections.clone());
    }
    println!("Oaclis UI: http://localhost:23232");
    let mutations = Mutex::new(());
    serve_requests(&server, |mut request| {
        if request.method() == &Method::Get && request.url() == "/peer/hello" {
            if !request.remote_addr().is_some_and(|address| crate::peers::local_address(address.ip())) {
                answer(request, 403, "Rete locale richiesta".into(), "text/plain");
                return;
            }
            let packet = crate::peers::hello(&peers);
            answer(request, if packet.is_null() { 503 } else { 200 }, packet.to_string(), "application/json");
            return;
        }
        if !request.remote_addr().is_some_and(|address| address.ip().is_loopback()) || !trusted(&request) {
            answer(request, 403, "Host non consentito".into(), "text/plain");
            return;
        }
        let url = request.url().to_owned();
        if request.method() == &Method::Get {
            if url == "/assets/oaclis-brand.ttf" {
                answer_bytes(request, 200, include_bytes!("../ui/assets/oaclis-brand.ttf").to_vec(), "font/ttf");
                return;
            }
            if url == "/assets/oaclis-crown.png" {
                answer_bytes(request, 200, include_bytes!("../ui/assets/oaclis-crown.png").to_vec(), "image/png");
                return;
            }
            if url == "/assets/oaclis-whale.png" {
                answer_bytes(request, 200, include_bytes!("../ui/assets/oaclis-whale.png").to_vec(), "image/png");
                return;
            }
            if url == "/favicon.ico" {
                answer(request, 200, (if role == "master" { include_str!("../ui/assets/oaclis-crown.svg") } else { include_str!("../ui/assets/oaclis-whale.svg") }).into(), "image/svg+xml");
                return;
            }
            let asset = match url.as_str() {
                "/assets/oaclis-crown.svg" => Some((include_str!("../ui/assets/oaclis-crown.svg").into(), "image/svg+xml")),
                "/assets/oaclis-whale.svg" => Some((include_str!("../ui/assets/oaclis-whale.svg").into(), "image/svg+xml")),
                "/" => Some((
                    include_str!("../ui/index.html").replace("__TOKEN__", &token).replace("__BRAND_ICON__", brand_icon),
                    "text/html; charset=utf-8",
                )),
                "/style.css" => Some((
                    include_str!("../ui/style.css").into(),
                    "text/css; charset=utf-8",
                )),
                "/badge-motion.css" => Some((include_str!("../ui/badge-motion.css").into(), "text/css; charset=utf-8")),
                "/badge-motion.js" => Some((include_str!("../ui/badge-motion.js").into(), "text/javascript; charset=utf-8")),
                "/app.js" => Some((
                    include_str!("../ui/app.js").into(),
                    "text/javascript; charset=utf-8",
                )),
                _ => None,
            };
            if let Some((body, mime)) = asset {
                answer(request, 200, body, mime);
                return;
            }
        }
        let authorized = request
            .headers()
            .iter()
            .any(|h| h.field.equiv("X-Oaclis-Token") && h.value.as_str() == token);
        if !authorized {
            answer(
                request,
                403,
                "Sessione locale richiesta".into(),
                "text/plain",
            );
            return;
        }
        // Keep settings writes serialized while slow catalogs leave other workers available.
        let _mutation = if request.method() == &Method::Post && !matches!(url.as_str(), "/api/catalog" | "/api/analysis" | "/api/check" | "/api/terminal") {
            Some(mutations.lock().unwrap())
        } else { None };
        let result = (|| -> io::Result<Value> {
            match (request.method(), url.as_str()) {
                (&Method::Get, "/api/peers") => Ok(crate::peers::snapshot(&peers)),
                (&Method::Post, "/api/peers/approve") => crate::peers::approve(&peers, &home, &body(&mut request)?),
                (&Method::Post, "/api/peers/network") => crate::peers::network(&peers),
                (&Method::Post, "/api/peers/restore") => crate::peers::restore(&peers, &home),
                (&Method::Post, "/api/terminal") => {
                    let v = body(&mut request)?;
                    let id = v["device_id"].as_str().ok_or_else(|| invalid("Dispositivo richiesto"))?;
                    let device = if id == "master" { None } else {
                        Some(devices::load(config.join("devices.json"))?.into_iter()
                            .find(|d| d.id == id).ok_or_else(|| invalid("Dispositivo non trovato"))?)
                    };
                    crate::terminal::request(device.as_ref(), &v)
                }
                (&Method::Get, "/api/state") => Ok(
                    json!({"paused":paused,"positions":crate::shared_settings::positions(&config)?,"link_positions":crate::shared_settings::read_positions(&config,"link-positions.json")?,"role":role,"devices":devices::load(config.join("devices.json"))?,"rules":rules::load(config.join("rules.json"))?,"chats":catalog(&codex)?,"chat_settings":environment::database(&config,"","preferences-get",None)?["preferences"],"ssh_settings":ssh_settings(&home,&config,"list",&json!(devices::load(config.join("devices.json"))?))?,"codex_root":codex,"appearance":crate::appearance::load(config.join("appearance.json"))?,"job":*job.lock().unwrap(),"connections":*connections.lock().unwrap(),"links":environment::database(&config,"","links-get",None)?}),
                ),
                (&Method::Get, "/api/status") => {
                    let snapshot = environment::database(&config,"","status-read",None)?;
                    Ok(json!({"paused":paused,"job":*job.lock().unwrap(),"analyses":snapshot["analyses"],"app_checks":snapshot["app_checks"],"transfers":snapshot["transfers"],"preparations":snapshot["preparations"],"connections":*connections.lock().unwrap(),"discovery":crate::peers::snapshot(&peers),
                        "shared":{"devices":devices::load(config.join("devices.json"))?,"appearance":crate::appearance::load(config.join("appearance.json"))?,"links":snapshot["links"],"chat_settings":snapshot["preferences"],"positions":crate::shared_settings::positions(&config)?,"link_positions":crate::shared_settings::read_positions(&config,"link-positions.json")?}}))
                }
                (&Method::Post, "/api/link-position") => {
                    let value = body(&mut request)?;
                    crate::shared_settings::write_positions(&config, "link-position", &value)?;
                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/positions") => {
                    let value = body(&mut request)?;
                    crate::shared_settings::save_positions(&config, &value)?;
                    Ok(json!({"ok":true}))
                }
                (&Method::Get, "/api/links") => {
                    environment::database(&config, "", "links-get", None)
                }
                (&Method::Post, "/api/links/delete") => {
                    let value = body(&mut request)?;
                    let (key, _) =
                        association(&value, &devices::load(config.join("devices.json"))?)?;
                    environment::database(&config, &key, "links-delete", None)
                }
                (&Method::Post, "/api/links") => {
                    let value = body(&mut request)?;
                    let (key, value) =
                        association(&value, &devices::load(config.join("devices.json"))?)?;
                    environment::database(&config, &key, "links-save", Some(&value))
                }
                (&Method::Post, "/api/chat/visibility") => {
                    let value = body(&mut request)?;
                    let id = value["id"].as_str().ok_or_else(|| invalid("Chat richiesta"))?;
                    if !devices::identifier(id) || !value["hidden"].is_boolean() { return Err(invalid("Visibilità non valida")); }
                    environment::database(&config, id, "chat-visibility", Some(&value))
                }
                (&Method::Post, "/api/chat/hide") | (&Method::Post, "/api/device/delete") => {
                    let value = body(&mut request)?;
                    let id = value["id"].as_str().ok_or_else(|| invalid("Oggetto richiesto"))?;
                    if !devices::identifier(id) { return Err(invalid("Oggetto non valido")); }
                    if url == "/api/chat/hide" {
                        environment::database(&config, id, "chat-hide", None)
                    } else {
                        if id == "master" { return Err(invalid("Il dispositivo locale non può essere eliminato")); }
                        let mut ds = devices::load(config.join("devices.json"))?;
                        let device = ds.iter().find(|d| d.id == id).ok_or_else(|| invalid("Dispositivo non trovato"))?.clone();
                        crate::peers::forget(&peers, &home, id)?;
                        let mut rs = rules::load(config.join("rules.json"))?;
                        rs.retain(|r| r.device_id != id);
                        ds.retain(|d| d.id != id);
                        ssh_settings(&home, &config, "delete", &json!({"id":id,"ssh_alias":device.ssh_alias}))?;
                        rules::save(config.join("rules.json"), &rs)?;
                        devices::save(config.join("devices.json"), &ds)?;
                        environment::database(&config, id, "device-forget", None)
                    }
                }
                (&Method::Post, "/api/appearance") => {
                    let v = body(&mut request)?;
                    let key = v["key"]
                        .as_str()
                        .ok_or_else(|| invalid("Oggetto richiesto"))?;
                    let value: crate::appearance::Appearance =
                        serde_json::from_value(v["value"].clone()).map_err(io::Error::other)?;
                    let mut entries = crate::appearance::load(config.join("appearance.json"))?;
                    entries.insert(key.into(), value);
                    crate::appearance::save(config.join("appearance.json"), &entries)?;
                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/app-check") => {
                    let value=body(&mut request)?;
                    if paused && value["force"] != true {
                        let key = environment::key(value["source_device_id"].as_str().unwrap_or("master"), value["chat_id"].as_str().ok_or_else(||invalid("Chat richiesta"))?)?;
                        return environment::database(&config,&key,"inventory-get",None);
                    }
                    crate::app_inventory::request(&config,&value)
                }
                (&Method::Post, "/api/catalog") => {
                    let value = body(&mut request)?;
                    if value["device_id"] == "master" {
                        return Ok(json!({"root":codex,"chats":catalog(&codex)?}));
                    }
                    let device = devices::load(config.join("devices.json"))?
                        .into_iter()
                        .find(|d| d.id == value["device_id"])
                        .ok_or_else(|| invalid("Dispositivo non trovato"))?;
                    let result = remote_catalog(&device)?;
                    if let Some(chats) = result["chats"].as_array() {
                        for chat in chats {
                            if chat["analysis"].is_object() {
                                let mut record = chat["analysis"].clone();
                                record["revision"] = json!(environment::revision(chat, Some(&device)));
                                environment::database(&config, &environment::key(&device.id, chat["id"].as_str().unwrap())?, "analysis-save", Some(&record))?;
                            }
                        }
                    }
                    Ok(result)
                }
                (&Method::Post, "/api/scopes") | (&Method::Post, "/api/analysis") => {
                    let v = body(&mut request)?;
                    let key = environment::key(
                        v["source_device_id"].as_str().unwrap_or("master"),
                        v["chat_id"]
                            .as_str()
                            .ok_or_else(|| invalid("Chat richiesta"))?,
                    )?;
                    if url == "/api/scopes" {
                        let scopes: environment::Scopes =
                            serde_json::from_value(v["scopes"].clone())
                                .map_err(io::Error::other)?;
                        environment::database(
                            &config,
                            v["chat_id"].as_str().unwrap(),
                            "chat-scopes",
                            Some(&json!(scopes)),
                        )
                    } else {
                        let record = environment::database(&config, &key, "analysis-get", None)?;
                        if !paused && record["status"] == "pending" && !analyses.lock().unwrap().contains(&key)
                        {
                            environment::database(
                                &config,
                                &key,
                                "analysis-save",
                                Some(
                                    &json!({"status":"failed","error":"Analisi interrotta: riprova"}),
                                ),
                            )
                        } else {
                            Ok(record)
                        }
                    }
                }
                (&Method::Post, "/api/analyze") => {
                    let v = body(&mut request)?;
                    let source = v["source_device_id"].as_str().unwrap_or("master");
                    let chat_id = v["chat_id"]
                        .as_str()
                        .ok_or_else(|| invalid("Chat richiesta"))?;
                    let key = environment::key(source, chat_id)?;
                    if paused && v["force"] != true { return environment::database(&config,&key,"analysis-get",None); }
                    let device = if source == "master" {
                        None
                    } else {
                        Some(
                            devices::load(config.join("devices.json"))?
                                .into_iter()
                                .find(|d| d.id == source)
                                .ok_or_else(|| invalid("Dispositivo non trovato"))?,
                        )
                    };
                    let list = if let Some(device) = &device {
                        remote_catalog(device)?["chats"].clone()
                    } else {
                        json!(catalog(&codex)?)
                    };
                    let chat = list
                        .as_array()
                        .and_then(|cs| cs.iter().find(|c| c["id"] == chat_id))
                        .ok_or_else(|| invalid("Chat non trovata"))?;
                    let revision = environment::revision(chat, device.as_ref());
                    let cached = environment::database(&config, &key, "analysis-get", None)?;
                    if analyses.lock().unwrap().contains(&key) {
                        return Ok(cached);
                    }
                    if matches!(cached["status"].as_str(), Some("ready" | "failed")) && v["force"] != true {
                        return Ok(cached);
                    }
                    if analyses.lock().unwrap().len() >= 2 {
                        return Err(invalid("Analisi già in corso; riprova tra poco"));
                    }
                    let pending = environment::database(
                        &config,
                        &key,
                        "analysis-save",
                        Some(&json!({"status":"pending","revision":revision})),
                    )?;
                    analyses.lock().unwrap().insert(key.clone());
                    let active = analyses.clone();
                    let config = config.clone();
                    let chat_id = chat_id.to_owned();
                    let path = chat["path"]
                        .as_str()
                        .ok_or_else(|| invalid("Archivio chat mancante"))?
                        .to_owned();
                    thread::spawn(move || {
                        let result = match environment::analyze(device.as_ref(), &path, &chat_id) {
                            Ok(manifest) => {
                                if device.is_none() {
                                    if let Err(error) = crate::preparation::capture(&config,&chat_id,Path::new(&path),&manifest) { eprintln!("Oaclis: dimensioni sorgenti: {error}"); }
                                }
                                json!({"status":"ready","revision":revision,"manifest":manifest})
                            }
                            Err(error) => {
                                json!({"status":"failed","revision":revision,"error":error.to_string()})
                            }
                        };
                        if let Err(error) =
                            environment::database(&config, &key, "analysis-save", Some(&result))
                        {
                            eprintln!("Oaclis: salvataggio analisi non riuscito: {error}");
                        }
                        active.lock().unwrap().remove(&key);
                    });
                    Ok(pending)
                }
                (&Method::Post, "/api/check") => {
                    if paused { return Ok(connections.lock().unwrap().clone()); }
                    for device in devices::load(config.join("devices.json"))? {
                        let mut lock = connections.lock().unwrap();
                        if lock[&device.id] == "checking" {
                            continue;
                        }
                        lock[&device.id] = json!("checking");
                        drop(lock);
                        let connections = connections.clone();
                        thread::spawn(move || {
                            let online = crate::ssh::run(&device, "true").is_ok();
                            connections.lock().unwrap()[&device.id] =
                                json!(if online { "online" } else { "offline" });
                        });
                    }
                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/device") => {
                    let mut value = body(&mut request)?;
                    let appearance = value.as_object_mut().ok_or_else(|| invalid("Dispositivo non valido"))?.remove("appearance")
                        .map(serde_json::from_value::<crate::appearance::Appearance>).transpose().map_err(io::Error::other)?;
                    if let Some(value) = &appearance { value.validate()?; }
                    let settings = value.as_object_mut().ok_or_else(|| invalid("Dispositivo non valido"))?.remove("ssh_settings");
                    if settings.is_some() && value.get("ssh_alias").is_none() {
                        value["ssh_alias"] = json!(format!("oaclis-{}", value["id"].as_str().ok_or_else(|| invalid("ID richiesto"))?));
                    }
                    let d: Device = serde_json::from_value(value).map_err(io::Error::other)?;
                    let mut ds = devices::load(config.join("devices.json"))?;
                    devices::upsert(&mut ds, d.clone())?;
                    if let Some(settings) = settings {
                        ssh_settings(&home, &config, "save", &json!({"id":d.id,"ssh_alias":d.ssh_alias,"settings":settings}))?;
                    }
                    devices::save(config.join("devices.json"), &ds)?;
                    if let Some(value) = appearance {
                        let mut entries = crate::appearance::load(config.join("appearance.json"))?;
                        entries.insert(format!("device:{}", d.id), value);
                        crate::appearance::save(config.join("appearance.json"), &entries)?;
                    }

                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/rule") => {
                    let r: Rule =
                        serde_json::from_value(body(&mut request)?).map_err(io::Error::other)?;
                    if !devices::load(config.join("devices.json"))?
                        .iter()
                        .any(|d| d.id == r.device_id)
                    {
                        return Err(invalid("Dispositivo non trovato"));
                    }
                    let mut rs = rules::load(config.join("rules.json"))?;
                    rules::upsert(&mut rs, r)?;
                    rules::save(config.join("rules.json"), &rs)?;
                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/run") => {
                    if paused { return Err(invalid("Sincronizzazioni in pausa")); }
                    let v = body(&mut request)?;
                    start_transfer(v, &codex, &config, &job)
                }
                _ => Err(invalid("Operazione non disponibile")),
            }
        })();
        match result {
            Ok(v) => answer(request, 200, v.to_string(), "application/json"),
            Err(e) => answer(
                request,
                error_status(&e),
                json!({"error":e.to_string()}).to_string(),
                "application/json",
            ),
        }
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_request_does_not_block_a_local_request() {
        use std::{net::TcpStream, io::{Read, Write}, sync::mpsc, time::Duration};
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let address = server.server_addr().to_ip().unwrap();
        let (started, wait_started) = mpsc::channel();
        let (release, wait_release) = mpsc::channel();
        let wait_release = Mutex::new(wait_release);
        let worker_server = server.clone();
        let workers = thread::spawn(move || serve_requests(&worker_server, |request| {
            if request.url() == "/slow" {
                started.send(()).unwrap();
                wait_release.lock().unwrap().recv_timeout(Duration::from_secs(5)).unwrap();
            }
            answer(request, 200, "available".into(), "text/plain");
        }));
        let mut slow = TcpStream::connect(address).unwrap();
        slow.write_all(b"GET /slow HTTP/1.0\r\n\r\n").unwrap();
        wait_started.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut fast = TcpStream::connect(address).unwrap();
        fast.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        fast.write_all(b"GET /status HTTP/1.0\r\n\r\n").unwrap();
        let mut response = String::new();
        let result = fast.read_to_string(&mut response);
        release.send(()).unwrap();
        for _ in 0..4 { server.unblock(); }
        workers.join().unwrap();
        result.unwrap();
        assert!(response.contains("available"));
    }
    #[test]
    fn operational_failures_are_not_bad_requests() {
        assert_eq!(error_status(&invalid("Richiesta non valida")), 400);
        assert_eq!(error_status(&io::Error::new(io::ErrorKind::ConnectionAborted, "SSH non disponibile")), 503);
        assert_eq!(error_status(&io::Error::other("database is locked")), 503);
    }
    #[test]
    fn accepts_local_tunnel_ports_only() {
        for host in ["localhost:23232", "localhost:23233", "127.0.0.1:23234", "[::1]:23233"] {
            assert!(local_host(host));
        }
        for host in ["evil.example:23233", "localhost.evil.example:23233", "192.168.1.154:23232", "localhost:0", "localhost:65536", "localhost:no", "localhost"] {
            assert!(!local_host(host));
        }
    }
    #[test]
    fn validates_associations_and_preserves_identity_across_direction_changes() {
        let devices = [Device {
            id: "client".into(),
            name: "Client".into(),
            ssh_alias: "client-ssh".into(),
        }];
        let mut value = json!({"source":"master","target":"client","operation":"send","entry":{"id":"chat-1","name":"Chat","path":"ignored"}});
        let (key, saved) = association(&value, &devices).unwrap();
        assert!(saved["entry"].get("path").is_none());
        assert_eq!(saved["scopes"]["chat"], json!(true));
        value["label"] = json!("Work");
        value["scopes"] = json!({"chat":true,"paths":false,"apps":true});
        assert_eq!(
            association(&value, &devices).unwrap().1["scopes"]["paths"],
            json!(false)
        );
        value["scopes"]["apps"] = json!("invalid");
        assert!(association(&value, &devices).is_err());
        value["scopes"]["apps"] = json!(true);
        value["source"] = json!("client");
        value["target"] = json!("master");
        assert_eq!(key, association(&value, &devices).unwrap().0);
        value["target"] = json!("missing");
        assert!(association(&value, &devices).is_err());
        value["target"] = json!("client");
        assert!(association(&value, &devices).is_err());
    }
    #[test]
    fn catalog_skips_links_and_uses_latest_title() {
        let root = std::env::temp_dir().join(format!("oaclis-catalog-{}", std::process::id()));
        fs::create_dir_all(root.join("sessions")).unwrap();
        fs::write(
            root.join("session_index.jsonl"),
            "{\"id\":\"01a0568c-cf6c-7d62-bb6d-12c20b4d9b16\",\"thread_name\":\"Titolo\"}\n",
        )
        .unwrap();
        fs::write(
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b16.jsonl",
            ),
            "secret content",
        )
        .unwrap();
        let entries = catalog(&root).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "Titolo");
        assert!(!entries[0].to_string().contains("secret content"));
        std::os::unix::fs::symlink(
            root.join("session_index.jsonl"),
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b17.jsonl",
            ),
        )
        .unwrap();
        fs::write(root.join("session_index.jsonl"), "").unwrap();
        let unindexed = catalog(&root).unwrap();
        assert_eq!(unindexed.len(), 1);
        assert_eq!(unindexed[0]["name"], "Senza titolo");
        let script = r#"import sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    db.execute('CREATE TABLE threads(id TEXT,name TEXT,title TEXT,source TEXT)')
    db.execute('INSERT INTO threads VALUES(?,?,?,?)',('01a0568c-cf6c-7d62-bb6d-12c20b4d9b16','Titolo Codex','Primo messaggio','cli'))
    db.execute('INSERT INTO threads VALUES(?,?,?,?)',('01a0568c-cf6c-7d62-bb6d-12c20b4d9b18',None,'','{"subagent":{}}'))"#;
        assert!(
            std::process::Command::new("python3")
                .args(["-c", script])
                .arg(root.join("state_5.sqlite"))
                .status()
                .unwrap()
                .success()
        );
        fs::write(
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b18.jsonl",
            ),
            "internal",
        )
        .unwrap();
        let titled = catalog(&root).unwrap();
        assert_eq!(titled.len(), 1);
        assert_eq!(titled[0]["name"], "Titolo Codex");
        assert!(std::process::Command::new("python3").args(["-c","import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); db.execute(\"UPDATE threads SET name='Titolo aggiornato' WHERE source='cli'\"); db.commit()"]).arg(root.join("state_5.sqlite")).status().unwrap().success());
        assert_eq!(catalog(&root).unwrap()[0]["name"], "Titolo aggiornato");
        fs::remove_dir_all(root).unwrap();
    }
}
