// web.rs v2.26.1661
//! Browser UI adapter. Transfer decisions remain in the existing core modules.
use crate::{
    devices::{self, Device},
    environment, invalid,
    rules::{self, Rule},
    catalog::{local as catalog, remote as remote_catalog},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
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
    let legacy_pause = std::env::var("OACLIS_PAUSED").as_deref() == Ok("1");
    let paused = std::env::var("OACLIS_PAUSE_TRANSFERS").map_or(legacy_pause, |value| value == "1");
    let checks_paused = std::env::var("OACLIS_PAUSE_CHECKS").map_or(legacy_pause, |value| value == "1");
    let settings_paused = std::env::var("OACLIS_PAUSE_SETTINGS").as_deref() == Ok("1");
    let config = home.join(".config/oaclis");
    fs::create_dir_all(&config)?;
    environment::database(&config, "", "migrate", None)?;
    crate::app_inventory::recover_interrupted(&config)?;
    crate::link_flow::recover(&config)?;
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
    let runtime=Arc::new(Mutex::new(crate::device_runtime::Runtime::new(&config)));
    crate::state_transport::start(config.clone(),codex.clone(),runtime.clone(),job.clone(),analyses.clone(),role.clone())?;
    if !settings_paused { crate::shared_settings::start(home.clone()); }
    if !checks_paused {
        crate::initial_inventory::start(codex.clone(), config.clone(), analyses.clone());
    }
    crate::automation::start(codex.clone(), config.clone(), job.clone(), connections.clone(), peers.clone(), !paused);
    println!("Oaclis UI: http://localhost:23232");
    let mobile = crate::mobile::start(config.clone(), role == "master");
    let configuration_edits = Mutex::new(());
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
                    include_str!("../ui/index.html").replace("__TOKEN__", &token).replace("__BRAND_ICON__", brand_icon).replace("__APP_VERSION__", env!("OACLIS_APP_VERSION")),
                    "text/html; charset=utf-8",
                )),
                "/style.css" => Some((
                    include_str!("../ui/style.css").into(),
                    "text/css; charset=utf-8",
                )),
                "/badge-motion.css" => Some((include_str!("../ui/badge-motion.css").into(), "text/css; charset=utf-8")),
                "/badge-motion.js" => Some((include_str!("../ui/badge-motion.js").into(), "text/javascript; charset=utf-8")),
                "/panels.js" => Some((include_str!("../ui/panels.js").into(), "text/javascript; charset=utf-8")),
                "/map.js" => Some((include_str!("../ui/map.js").into(), "text/javascript; charset=utf-8")),
                "/console.js" => Some((include_str!("../ui/console.js").into(), "text/javascript; charset=utf-8")),
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
        // Only legacy config-file edits need a route lock; state_store serializes metadata.
        let _configuration_edit = if request.method() == &Method::Post && matches!(url.as_str(), "/api/device" | "/api/device/delete" | "/api/rule") {
            Some(configuration_edits.lock().unwrap())
        } else { None };
        if request.method() == &Method::Post { crate::state_transport::log(format!("HTTP POST {url}")); }
        let result = (|| -> io::Result<Value> {
            match (request.method(), url.as_str()) {
                (&Method::Get, "/api/mobile/qr") => {
                    if role != "master" { return Err(invalid("Comando disponibile dal master")); }
                    crate::mobile::access_qr()
                }
                (&Method::Post, "/api/mobile/invite") => {
                    if role != "master" { return Err(invalid("Invito disponibile dal master")); }
                    crate::mobile::invite(&config)
                }
                (&Method::Get, "/api/mobile/pending") => {
                    if role != "master" { return Err(invalid("Conferma disponibile dal master")); }
                    crate::mobile::pending(&config)
                }
                (&Method::Post, "/api/mobile/approve") => {
                    if role != "master" { return Err(invalid("Conferma disponibile dal master")); }
                    crate::mobile::approve(&config, &body(&mut request)?, &mobile)
                }
                (&Method::Post, "/api/mobile/delete") => {
                    if role != "master" { return Err(invalid("Comando disponibile dal master")); }
                    crate::mobile::remove(&config, &body(&mut request)?, &mobile)
                }
                (&Method::Post, "/api/mobile/chats") => {
                    if role != "master" { return Err(invalid("Comando disponibile dal master")); }
                    crate::mobile::chat_access(&config, &codex, &body(&mut request)?, &mobile)
                }
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
                    json!({"mobile_devices":mobile.snapshot(),"device_runtime":crate::device_runtime::snapshot(&config,&runtime)?,"build":crate::device_runtime::build(),"paused":paused,"checks_paused":checks_paused,"settings_paused":settings_paused,"positions":crate::shared_settings::positions(&config)?,"link_positions":crate::shared_settings::read_positions(&config,"link-positions.json")?,"role":role,"devices":devices::load(config.join("devices.json"))?,"rules":rules::load(config.join("rules.json"))?,"chats":catalog(&codex)?,"chat_settings":environment::database(&config,"","preferences-get",None)?["preferences"],"ssh_settings":crate::ssh_settings::settings(&home,&config,"list",&json!(devices::load(config.join("devices.json"))?))?,"codex_root":codex,"appearance":crate::appearance::load(config.join("appearance.json"))?,"job":*job.lock().unwrap(),"connections":*connections.lock().unwrap(),"links":environment::database(&config,"","links-get",None)?}),
                ),
                (&Method::Post, "/api/device/control") => {
                    let value = body(&mut request)?;
                    let id = value["device_id"].as_str().ok_or_else(||invalid("Dispositivo richiesto"))?;
                    let action = value["action"].as_str().ok_or_else(||invalid("Comando richiesto"))?;
                    if id != "master" { return crate::device_runtime::remote(&config,id,action); }
                    crate::device_runtime::control(&runtime,&job,&analyses,&role,action,value["revision"].as_str().unwrap_or(""))
                }
                (&Method::Post, "/api/path/authorize") => crate::path_access::authorize(body(&mut request)?),
                (&Method::Get, "/api/log") => Ok(crate::state_transport::logs()),
                (&Method::Get, "/api/status") => {
                    let snapshot = environment::database(&config,"","status-read",None)?;
                    Ok(json!({"mobile_devices":mobile.snapshot(),"device_runtime":crate::device_runtime::snapshot(&config,&runtime)?,"build":crate::device_runtime::build(),"paused":paused,"checks_paused":checks_paused,"settings_paused":settings_paused,"job":*job.lock().unwrap(),"analyses":snapshot["analyses"],"app_checks":snapshot["app_checks"],"transfers":snapshot["transfers"],"preparations":snapshot["preparations"],"connections":*connections.lock().unwrap(),"discovery":crate::peers::snapshot(&peers),
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
                (&Method::Post, "/api/links/play") => {
                    if paused { return Err(invalid("Trasferimenti in pausa")); }
                    let value=body(&mut request)?;
                    let (key,_)=association(&value,&devices::load(config.join("devices.json"))?)?;
                    let links=environment::database(&config,"","links-get",None)?;
                    let link=links.as_array().and_then(|all|all.iter().find(|link|crate::link_flow::key(link).is_ok_and(|k|k==key))).ok_or_else(||invalid("Collegamento non trovato"))?;
                    if link["operation"] != "sync" && link["flow"]["initialized"]==true { return Err(invalid("Trasferimento monodirezionale già completato")); }
                    if link["flow"]["phase"]=="conflict" {return Err(invalid("Le chat divergono: scegliere quale copia mantenere"));}
                    if link["flow"]["phase"]=="transferring" {return Err(invalid("Trasferimento già in corso"));}
                    if link["flow"]["phase"]!="ready" {return Err(invalid("Nessuna sincronizzazione pronta: attendi il confronto"));}
                    let mut flow=link["flow"].clone();flow["requested"]=json!(true);
                    crate::link_flow::save(&config,link,flow)?;
                    environment::database(&config,"","links-get",None)
                }
                (&Method::Post, "/api/links") => {
                    let value = body(&mut request)?;
                    let (key, mut value) =
                        association(&value, &devices::load(config.join("devices.json"))?)?;
                    let links=environment::database(&config,"","links-get",None)?;
                    value["flow"]=links.as_array().and_then(|all|all.iter().find(|link|crate::link_flow::key(link).is_ok_and(|k|k==key))).map(|link|link["flow"].clone())
                        .unwrap_or_else(||crate::link_flow::initial(crate::peers::snapshot(&peers)["local_id"].as_str().unwrap_or("")));
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
                        crate::ssh_settings::settings(&home, &config, "delete", &json!({"id":id,"ssh_alias":device.ssh_alias}))?;
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
                    crate::appearance::save(config.join("appearance.json"), &std::collections::BTreeMap::from([(key.into(), value)]))?;
                    Ok(json!({"ok":true}))
                }
                (&Method::Post, "/api/app-check") => {
                    let value=body(&mut request)?;
                    if checks_paused && value["force"] != true {
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
                        if !checks_paused && record["status"] == "pending" && !analyses.lock().unwrap().contains(&key)
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
                    let value=body(&mut request)?;
                    crate::initial_inventory::request(&config,&codex,&analyses,checks_paused,value)
                }
                (&Method::Post, "/api/check") => {
                    if checks_paused { return Ok(connections.lock().unwrap().clone()); }
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
                        crate::ssh_settings::settings(&home, &config, "save", &json!({"id":d.id,"ssh_alias":d.ssh_alias,"settings":settings}))?;
                    }
                    devices::save(config.join("devices.json"), &ds)?;
                    if let Some(value) = appearance {
                        crate::appearance::save(config.join("appearance.json"), &std::collections::BTreeMap::from([(format!("device:{}", d.id), value)]))?;
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
                    crate::transfer_job::start(v, &codex, &config, &job)
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

}
