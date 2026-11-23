use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::app::App;
use crate::paths;

const CONTROL_COMMANDS: &[&str] = &[
    "status", "on", "off", "toggle", "clear", "demo", "quit",
    "style", "sound", "settings", "section", "toolbar", "welcome",
];

/// Render a JSON value the same way Python's `str()` renders parsed values.
/// The original control client prints `key=value` from a `json.loads` result,
/// so booleans are `True`/`False`, strings are unquoted, and `None` is possible.
fn py_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => if *value { "True" } else { "False" }.to_string(),
        Value::String(value) => value.clone(),
        Value::Null => "None".to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(py_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map.iter().map(|(k, v)| format!("'{k}': {}", py_value(v))).collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

pub fn send_command(command: &str) -> i32 {
    let path = paths::control_socket();
    if !path.exists() {
        eprintln!("tinyhandctl: TinyHand is not running ({})", path.display());
        return 1;
    }
    let mut client = match UnixStream::connect(&path) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("tinyhandctl: {e}");
            return 1;
        }
    };
    let _ = client.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = client.set_write_timeout(Some(Duration::from_secs(2)));
    if let Err(e) = client.write_all(command.as_bytes()) {
        eprintln!("tinyhandctl: {e}");
        return 1;
    }
    let mut data = Vec::new();
    if let Err(e) = client.read_to_end(&mut data) {
        eprintln!("tinyhandctl: {e}");
        return 1;
    }
    let text = String::from_utf8_lossy(&data);
    match serde_json::from_str::<Value>(text.trim()) {
        Ok(Value::Object(map)) if map.contains_key("error") => {
            eprintln!("{}", py_value(&map["error"]));
            1
        }
        Ok(Value::Object(map)) => {
            let parts: Vec<String> = map.iter().map(|(k, v)| format!("{k}={}", py_value(v))).collect();
            println!("{}", parts.join("  "));
            0
        }
        Ok(value) => {
            println!("{}", py_value(&value));
            0
        }
        Err(_) => {
            println!("{}", text.trim());
            0
        }
    }
}

fn url_command(url: &str) -> String {
    let lowered = url.to_ascii_lowercase();
    let Some(scheme_at) = lowered.find("tinyhand://") else {
        return url.to_string();
    };
    let rest = &url[scheme_at + "tinyhand://".len()..];
    let without_fragment = rest.split('#').next().unwrap_or("");
    let without_query = without_fragment.split('?').next().unwrap_or("");
    let (authority, path) = match without_query.find('/') {
        Some(index) => (&without_query[..index], &without_query[index + 1..]),
        None => (without_query, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("");
    let command = if host.is_empty() { "status".to_string() } else { host.to_ascii_lowercase() };
    let argument = path.trim_start_matches('/').split('/').next().unwrap_or("");
    format!("{command} {argument}").trim().to_string()
}

pub fn main() -> i32 {
    let args: Vec<String> = env::args().collect();
    let name = Path::new(args.first().map(String::as_str).unwrap_or("tinyhand"))
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("tinyhand");

    if let Some(first) = args.get(1) {
        if first.to_ascii_lowercase().starts_with("tinyhand://") {
            let url = url_command(first);
            let mut parts = url.splitn(2, ' ');
            let command = parts.next().unwrap_or("status").to_ascii_lowercase();
            let argument = parts.next().unwrap_or("").to_string();
            return send_command(&format!("{command} {argument}").trim().to_string());
        }
    }
    if name == "tinyhandctl" || args.get(1).is_some_and(|a| CONTROL_COMMANDS.contains(&a.as_str())) {
        let command = if args.len() > 1 { args[1..].join(" ") } else { "status".into() };
        return send_command(&command);
    }
    if env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("tinyhand: WAYLAND_DISPLAY is unset — start it inside a Wayland session");
        return 1;
    }
    let hidden = args.iter().any(|a| a == "--hidden");
    let settings = args.iter().any(|a| a == "--settings");
    let mut app = App::new();
    if settings && paths::control_socket().exists() {
        return send_command("settings");
    }
    app.apply_startup_flags(hidden, settings);
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn url_command_follows_python_urlparse_semantics() {
        assert_eq!(url_command("tinyhand://style/coral?x=1"), "style coral");
        assert_eq!(url_command("tinyhand://settings#foo"), "settings");
        assert_eq!(url_command("tinyhand://status:9/"), "status");
        assert_eq!(url_command("tinyhand://user@toggle"), "toggle");
        assert_eq!(url_command("tinyhand://STYLE/Coral"), "style Coral");
    }

    #[test]
    fn py_value_follows_python_str_repr() {
        assert_eq!(py_value(&json!(true)), "True");
        assert_eq!(py_value(&json!(false)), "False");
        assert_eq!(py_value(&json!("lavender")), "lavender");
        assert_eq!(py_value(&json!([1, 2])), "[1, 2]");
    }
}
