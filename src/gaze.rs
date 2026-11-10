// gaze.rs v1.0.0
//! Read-only Windows display discovery for the gaze setup panel.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io, path::Path};

#[derive(Debug, Deserialize, Serialize)]
struct Monitor {
    id: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    primary: bool,
}

fn layout(monitors: Vec<Monitor>) -> io::Result<Value> {
    if monitors.iter().any(|m| m.width <= 0 || m.height <= 0) {
        return Err(crate::invalid("Dimensioni monitor non valide"));
    }
    let desktop = if monitors.is_empty() { Value::Null } else {
        let x = monitors.iter().map(|m| i64::from(m.x)).min().unwrap();
        let y = monitors.iter().map(|m| i64::from(m.y)).min().unwrap();
        let right = monitors.iter().map(|m| i64::from(m.x) + i64::from(m.width)).max().unwrap();
        let bottom = monitors.iter().map(|m| i64::from(m.y) + i64::from(m.height)).max().unwrap();
        json!({"x":x,"y":y,"width":right-x,"height":bottom-y})
    };
    Ok(json!({"supported":true,"monitors":monitors,"desktop":desktop}))
}

pub(crate) fn monitors() -> io::Result<Value> {
    if !Path::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe").is_file() {
        return Ok(json!({"supported":false,"monitors":[],"desktop":null}));
    }
    let output = crate::ssh_settings::text(&mut crate::keep_awake::windows(include_str!("../scripts/gaze-monitors.ps1")), 8)?;
    let monitors = serde_json::from_str(output.trim_start_matches('\u{feff}')).map_err(io::Error::other)?;
    layout(monitors)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen(x:i32,y:i32,width:i32,height:i32)->Monitor { Monitor{id:format!("{x}:{y}"),x,y,width,height,primary:x==0&&y==0} }
    #[test]
    fn keeps_real_negative_positions_and_mixed_resolutions() {
        let value=layout(vec![screen(0,0,1920,1080),screen(0,-1080,1920,1080),screen(1920,-522,1920,1080)]).unwrap();
        assert_eq!(value["desktop"],json!({"x":0,"y":-1080,"width":3840,"height":2160}));
        assert_eq!(value["monitors"][2]["y"],-522);
        let value=layout(vec![screen(-2560,0,2560,1440),screen(0,0,1920,1080)]).unwrap();
        assert_eq!(value["desktop"],json!({"x":-2560,"y":0,"width":4480,"height":1440}));
    }
    #[test]
    fn empty_or_invalid_displays_do_not_create_a_fake_desktop() {
        assert!(layout(vec![]).unwrap()["desktop"].is_null());
        assert!(layout(vec![screen(0,0,0,1080)]).is_err());
    }
}
