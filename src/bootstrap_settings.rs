// bootstrap_settings.rs v1.3.141
use crate::{invalid, staging};
use serde_json::{Value, json};
use std::{env, fs, io, path::{Path, PathBuf}};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

pub fn save(directory: &Path, mut environment: Value, tools: Value) -> io::Result<PathBuf> {
    if !matches!(environment["os"].as_str(), Some("linux" | "macos" | "windows"))
        || !(environment["host_os"].is_null() || matches!(environment["host_os"].as_str(), Some("linux" | "macos" | "windows"))) {
        return Err(invalid("Sistema non valido"));
    }
    if directory.symlink_metadata().is_ok_and(|m| !m.is_dir()) {
        return Err(invalid("La cartella settings non deve essere un link simbolico"));
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(directory)?;
    let path = directory.join("settings.json");
    let mut saved: Value = match crate::open_regular(&path) {
        Ok((file, _)) => {
            use std::io::Read;
            let mut text = String::new();
            std::io::BufReader::new(file).read_to_string(&mut text)?;
            serde_json::from_str(text.trim_start_matches('\u{feff}'))?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => json!({}),
        Err(error) => return Err(error),
    };
    if !saved.is_object() || saved.get("schema_version").is_some_and(|v| v != 1) {
        return Err(invalid("Formato settings non supportato: file conservato"));
    }
    let previous = saved.clone();
    if saved.get("environment").is_some_and(|v| !v.is_object()) {
        return Err(invalid("Scenario salvato non valido: file conservato"));
    }
    if environment["container"] == true && environment["host_os"].is_null()
        && ["os", "container", "wsl_kernel"].iter().all(|key| saved["environment"][key] == environment[key]) {
        environment["host_os"] = saved["environment"]["host_os"].clone();
    }
    let object = saved.as_object_mut().unwrap();
    object.entry("schema_version").or_insert(json!(1));
    object.entry("control_port").or_insert(json!(23232));
    object.insert("environment".into(), environment);
    let existing = object.entry("tools").or_insert(json!({})).as_object_mut()
        .ok_or_else(|| invalid("Impostazioni tools non valide: file conservato"))?;
    existing.extend(tools.as_object().ok_or_else(|| invalid("Tools non validi"))?.clone());
    if saved != previous { staging::save_json(&path, &saved)?; }
    Ok(path)
}

pub fn configure(system: &str, container: bool, wsl: bool) -> io::Result<()> {
    let home = env::var_os("HOME").ok_or_else(|| invalid("HOME non definita"))?;
    let directory = env::var_os("OACLIS_SETTINGS_DIR").filter(|s| !s.is_empty())
        .map(PathBuf::from).unwrap_or_else(|| Path::new(&home).join(".config/oaclis"));
    let system = match system { "Linux" => "linux", "Darwin" => "macos", _ => return Err(invalid("Sistema non valido")) };
    let host = env::var("OACLIS_HOST_OS").ok().filter(|s| !s.is_empty())
        .or_else(|| if wsl { Some("windows".into()) } else if !container { Some(system.into()) } else { None });
    let distribution = env::var("OACLIS_WSL_DISTRO").ok().filter(|s| !s.is_empty())
        .or_else(|| env::var("WSL_DISTRO_NAME").ok().filter(|s| !s.is_empty()));
    let environment = json!({"os":system,"host_os":host,"container":container,"wsl_kernel":wsl,
        "runtime":if container {"container"} else if wsl {"wsl"} else {"native"},"wsl_distribution":distribution});
    let mut tools = json!({});
    for name in ["node", "npm", "codex"] {
        let path = env::split_paths(&env::var_os("PATH").unwrap_or_default()).map(|p| p.join(name)).find(|p| {
            p.metadata().is_ok_and(|m| {
                #[cfg(unix)]
                { m.is_file() && m.permissions().mode() & 0o111 != 0 }
                #[cfg(not(unix))]
                { m.is_file() }
            })
        });
        tools[name] = json!({"detected":path.is_some(),"path":path});
    }
    println!("Settings: {}", save(&directory, environment, tools)?.display());
    Ok(())
}
