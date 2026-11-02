// llama.rs v1.4.68
//! Read-only discovery through the existing AI-LAB SSH tunnel; no model is loaded.
use serde_json::{Value, json};
use std::{io::{self, Read}, path::Path, time::Duration};

fn discover(endpoint: &str, key: &str) -> io::Result<Value> {
    if key.is_empty() || key.len() > 4096 || key.chars().any(char::is_control) {
        return Err(crate::invalid("Chiave llama.cpp non valida"));
    }
    static CLIENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(|| ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .timeout_connect(Some(Duration::from_secs(3)))
        .max_redirects(0).build().into());
    let mut response = client.get(endpoint).header("Authorization", &format!("Bearer {key}"))
        .call().map_err(|_| io::Error::other("Collegamento llama.cpp non disponibile"))?;
    if response.status() != 200 { return Err(io::Error::other("Collegamento llama.cpp non disponibile")); }
    let mut bytes = Vec::new();
    response.body_mut().as_reader().take(1048577).read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 { return Err(crate::invalid("Elenco modelli troppo grande")); }
    let reply: Value = serde_json::from_slice(&bytes).map_err(|_| crate::invalid("Elenco modelli non valido"))?;
    let rows = reply["data"].as_array().ok_or_else(|| crate::invalid("Elenco modelli non valido"))?;
    let mut models = Vec::new();
    for row in rows {
        let id = row["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 512 && !id.chars().any(char::is_control))
            .ok_or_else(|| crate::invalid("Nome modello non valido"))?;
        let status = row["status"]["value"].as_str().unwrap_or("unknown");
        models.push(json!({"id":id,"loaded":status == "loaded"}));
    }
    Ok(json!({"engine":"llama.cpp","models":models}))
}

pub(crate) fn models(home: &Path) -> io::Result<Value> {
    let key = crate::engines::llama_key(home)?;
    // Reuse the installed tunnel. Never expose the Jetson listener or its key to browsers.
    discover("http://127.0.0.1:18085/v1/models", key.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_authenticates_and_keeps_server_configuration_private() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1/models", server.server_addr());
        let worker = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            assert_eq!(request.url(), "/v1/models");
            assert!(request.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == "Bearer fixture-key"));
            request.respond(tiny_http::Response::from_string(r#"{"data":[{"id":"Qwen3","status":{"value":"loaded","args":["private"],"preset":"private"}},{"id":"Bonsai","status":{"value":"unloaded"}}]}"#)).unwrap();
        });
        assert_eq!(discover(&endpoint, "fixture-key").unwrap(), json!({"engine":"llama.cpp","models":[{"id":"Qwen3","loaded":true},{"id":"Bonsai","loaded":false}]}));
        worker.join().unwrap();
        assert!(discover(&endpoint, "bad\nkey").is_err());
    }

    #[test]
    #[ignore = "Requires the configured AI-LAB tunnel; discovery only"]
    fn installed_jetson_catalog() {
        let reply = models(Path::new(&std::env::var("HOME").unwrap())).unwrap();
        assert!(reply["models"].as_array().unwrap().iter().any(|model| model["id"] == "Qwen3-8B-Q4_K_M"));
        println!("Jetson: {} models discovered", reply["models"].as_array().unwrap().len());
    }
}
