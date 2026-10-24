// main.rs v1.2.183
use oaclis::{
    chat::Chat,
    compare,
    devices::{self, Device},
    paths::ProjectPaths,
    rules::{self, Rule},
    sync::{self, Kind},
    transfer,
};
use std::env;
use std::error::Error;
use std::io;
use std::path::Path;

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [command] if command == "ssh-gateway" => oaclis::ssh_gateway::run()?,
        [command] if command == "file-request" => oaclis::ssh_gateway::files()?,
        [command] if command == "environment-request" => oaclis::environment::forward()?,
        [command] if command == "prepare-awake" => oaclis::keep_awake::prepare(None)?,
        [command, option, user] if command == "prepare-awake" && option == "--user" => oaclis::keep_awake::prepare(Some(user.to_str().ok_or("Utente non valido")?))?,
        [command] if command == "keep-awake-hold" => oaclis::keep_awake::run()?,
        [command, system, container, wsl] if command == "save-scenario" => {
            let system = system.to_str().ok_or("Sistema non valido")?;
            if !["yes", "no"].iter().any(|v| container == v) || !["yes", "no"].iter().any(|v| wsl == v) { return Err("Scenario non valido".into()); }
            oaclis::bootstrap_settings::configure(system, container == "yes", wsl == "yes")?;
        }
        [command] if command == "prepare-service" => oaclis::service_setup::prepare()?,
        [command] if command == "ui-launch-path" => println!("{}", oaclis::open_ui::launch_path()?.display()),
        [command] if command == "open-ui" => oaclis::open_ui::open(),
        [command, path, uid, gid] if command == "prepare-path-root" => {
            oaclis::path_access::create_privileged(Path::new(path),uid.to_str().ok_or("UID non valido")?.parse()?,gid.to_str().ok_or("GID non valido")?.parse()?)?;
        }
        [command] if command == "state-request" => {
            let home=env::var_os("HOME").ok_or("HOME non definita")?;
            oaclis::state_transport::forward(Path::new(&home))?;
        }
        [command] if command == "setup" => {
            let home = env::var_os("HOME").ok_or("HOME non definita")?;
            oaclis::setup::configure(Path::new(&home))?;
        }
        [command] if command == "serve" => {
            let home = env::var_os("HOME").ok_or("HOME non definita")?;
            oaclis::web::serve(home.into())?;
        }
        [command, id] if command == "prepare" => {
            let home = env::var_os("HOME").ok_or("HOME non definita")?;
            println!("{}", serde_json::to_string_pretty(&oaclis::preparation::from_saved(Path::new(&home),id.to_str().ok_or("ID non UTF-8")?)?)?);
        }
        [command, file, id, name, alias] if command == "device-set" => {
            let mut devices = devices::load(Path::new(file))?;
            devices::upsert(
                &mut devices,
                Device {
                    id: id.to_str().ok_or("ID non UTF-8")?.into(),
                    name: name.to_str().ok_or("Nome non UTF-8")?.into(),
                    ssh_alias: alias.to_str().ok_or("Alias non UTF-8")?.into(),
                },
            )?;
            devices::save(Path::new(file), &devices)?;
            println!("Dispositivo salvato");
        }
        [command, file] if command == "device-list" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&devices::load(Path::new(file))?)?
            );
        }
        [command, file, id, source, destination] if command == "send-file" => {
            let devices = devices::load(Path::new(file))?;
            let device = devices
                .iter()
                .find(|device| Some(device.id.as_str()) == id.to_str())
                .ok_or("Dispositivo non trovato")?;
            let receipt = transfer::send_file_with_progress(
                device,
                Path::new(source),
                Path::new(destination),
                |event| eprintln!("{}", serde_json::to_string(&event).unwrap()),
            )?;
            println!("{}", serde_json::to_string(&receipt)?);
        }
        [command, file, id, source, destination, checkpoint]
            if command == "sync-file" || command == "sync-chat" =>
        {
            let devices = devices::load(Path::new(file))?;
            let device = devices
                .iter()
                .find(|device| Some(device.id.as_str()) == id.to_str())
                .ok_or("Dispositivo non trovato")?;
            let kind = if command == "sync-chat" {
                Kind::Chat
            } else {
                Kind::File
            };
            let outcome = sync::synchronize(
                device,
                Path::new(source),
                Path::new(destination),
                Path::new(checkpoint),
                kind,
                |event| eprintln!("{}", serde_json::to_string(&event).unwrap()),
            )?;
            println!("{}", serde_json::to_string(&outcome)?);
        }
        [command, file, definition] if command == "rule-set" => {
            let mut entries = rules::load(Path::new(file))?;
            let rule: Rule = serde_json::from_reader(std::fs::File::open(definition)?)?;
            rules::upsert(&mut entries, rule)?;
            rules::save(Path::new(file), &entries)?;
            println!("Regola salvata");
        }
        [command, file] if command == "rule-list" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&rules::load(Path::new(file))?)?
            );
        }
        [command, config, file, id, state] if command == "rule-run" => {
            let entries = rules::load(Path::new(file))?;
            let rule = entries
                .iter()
                .find(|r| Some(r.id.as_str()) == id.to_str())
                .ok_or("Regola non trovata")?;
            let devices = devices::load(Path::new(config))?;
            let device = devices
                .iter()
                .find(|d| d.id == rule.device_id)
                .ok_or("Dispositivo della regola non trovato")?;
            let results = rules::execute(device, rule, Path::new(state), |event| {
                eprintln!("{}", serde_json::to_string(&event).unwrap())
            })?;
            println!("{}", serde_json::to_string(&results)?);
        }
        [command, path] if command == "inspect" => {
            let chat = Chat::read(Path::new(path))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "id": chat.id, "history_mode": chat.history_mode,
                    "project_paths": chat.project_paths, "parent_id": chat.parent_id,
                    "records": chat.record_count()
                }))?
            );
        }
        [command, source, destination] if command == "compare-chat" => {
            let source = Chat::read(Path::new(source))?;
            let destination = match Chat::read(Path::new(destination)) {
                Ok(chat) => Some(chat),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            println!("{:?}", compare::chats(&source, destination.as_ref()));
        }
        [command, source, destination] if command == "compare-file" => {
            println!(
                "{:?}",
                compare::files(Path::new(source), Path::new(destination))?
            );
        }
        [command, source_root, destination_root, path] if command == "map-path" => {
            let mapping = ProjectPaths::new(source_root, destination_root)?;
            println!("{}", mapping.map(Path::new(path))?.display());
        }
        [] => println!(
            "Oaclis\nprepare-awake [--user UTENTE]\nsave-scenario SISTEMA CONTAINER WSL\nprepare-service\nopen-ui\nsetup (ricerca master e configurazione ruolo)\nserve (UI locale sulla porta 23232)\nprepare CHAT_ID (dimensioni sorgenti dai percorsi salvati)\ninspect CHAT\ncompare-chat ORIGINE DESTINAZIONE\ncompare-file ORIGINE DESTINAZIONE\nmap-path CARTELLA_ORIGINE CARTELLA_DESTINAZIONE PERCORSO\ndevice-set CONFIG ID NOME ALIAS_SSH\ndevice-list CONFIG\nsend-file CONFIG ID ORIGINE DESTINAZIONE\nsync-file CONFIG ID LOCALE REMOTO CHECKPOINT\nsync-chat CONFIG ID LOCALE REMOTO CHECKPOINT\nrule-set REGOLE DEFINIZIONE_JSON\nrule-list REGOLE\nrule-run DISPOSITIVI REGOLE ID STATO"
        ),
        _ => return Err("Comando non valido; eseguire oaclis senza argomenti per l'elenco".into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Oaclis: {error}");
        std::process::exit(1);
    }
}
