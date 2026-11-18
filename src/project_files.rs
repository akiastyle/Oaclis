// project_files.rs v1.0.0
//! On-demand, read-only project directory listings for the shared console.
use crate::invalid;
use serde_json::{Value,json};
use std::{fs,io::{self,Read},path::{Component,Path}};

fn project_url(registry:&Path,root:&Path)->Option<String> {
    let mut html=String::new();fs::File::open(registry).ok()?.take(262144).read_to_string(&mut html).ok()?;
    let rows=regex::Regex::new(r#"<tr>.*?<code>([^<]+)</code>.*?</td><td>(.*?)</td></tr>"#).unwrap();
    let links=regex::Regex::new(r#"href="([^"]+)""#).unwrap();
    rows.captures_iter(&html).filter_map(|row|{
        let path=fs::canonicalize(&row[1]).ok()?;
        if !root.starts_with(&path){return None;}
        let link=links.captures(&row[2])?[1].to_owned();
        let url=if link.starts_with('/')&&!link.starts_with("//"){format!("http://localhost{link}")}else if link.starts_with("http://")||link.starts_with("https://"){link}else{return None;};
        Some((path.components().count(),url))
    }).max_by_key(|(depth,_)|*depth).map(|(_,url)|url)
}
pub(crate) fn listing(root:&Path,relative:&str,registry:&Path)->io::Result<Value> {
    if relative.len()>4096||relative.contains('\0')||Path::new(relative).components().any(|part|!matches!(part,Component::Normal(_))){return Err(invalid("Percorso progetto non valido"));}
    let root=fs::canonicalize(root)?;
    let path=fs::canonicalize(root.join(relative))?;
    if !path.starts_with(&root)||!path.is_dir(){return Err(invalid("Cartella esterna al progetto"));}
    let mut entries=Vec::new();
    for entry in fs::read_dir(&path)?.take(2001) {
        let entry=entry?;let kind=entry.file_type()?;
        entries.push(json!({"name":entry.file_name().to_string_lossy(),"directory":kind.is_dir()}));
    }
    entries.sort_by(|a,b|b["directory"].as_bool().cmp(&a["directory"].as_bool()).then_with(||a["name"].as_str().unwrap().to_lowercase().cmp(&b["name"].as_str().unwrap().to_lowercase())));
    let mut selected=Vec::new();let mut bytes=0;
    for entry in &entries {bytes+=serde_json::to_vec(entry)?.len();if selected.len()==200||bytes>20000{break;}selected.push(entry.clone());}
    Ok(json!({"project":{"root":root,"path":relative,"url":project_url(registry,&root),"more":selected.len()<entries.len(),"entries":selected}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listing_is_sorted_bounded_and_cannot_escape() {
        let dir=std::env::temp_dir().join(format!("oaclis-project-files-{}",std::process::id()));fs::create_dir_all(dir.join("project/sub")).unwrap();
        fs::write(dir.join("project/z.txt"),"test").unwrap();fs::write(dir.join("project/A.txt"),"test").unwrap();
        let registry=dir.join("index.html");fs::write(&registry,format!(r#"<tr><td><code>{}</code></td><td><a href="/demo/">Demo</a></td></tr>"#,dir.join("project").display())).unwrap();
        let value=listing(&dir.join("project"),"",&registry).unwrap();
        assert_eq!(value["project"]["url"],"http://localhost/demo/");assert_eq!(value["project"]["entries"][0]["name"],"sub");assert_eq!(value["project"]["entries"][1]["name"],"A.txt");
        assert!(listing(&dir.join("project"),"../",&registry).is_err());assert!(listing(&dir.join("project"),"/tmp",&registry).is_err());
        std::os::unix::fs::symlink(&dir,dir.join("project/escape")).unwrap();assert!(listing(&dir.join("project"),"escape",&registry).is_err());
        assert_eq!(listing(&dir.join("project"),"sub",&registry).unwrap()["project"]["entries"],json!([]));
        for n in 0..210 {fs::write(dir.join(format!("project/{n}.txt")),"").unwrap();}
        let value=listing(&dir.join("project"),"",&registry).unwrap();assert_eq!(value["project"]["entries"].as_array().unwrap().len(),200);assert_eq!(value["project"]["more"],true);
        fs::remove_dir_all(dir).unwrap();
    }
}
