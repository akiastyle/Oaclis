# environment_setup.py v1.6.211
"""Native destination preparation. No commands supplied by the language model."""
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

# Non-interactive SSH does not load the shell's Rust/Node environment.
_home = pathlib.Path.home()
_bins = [_home / ".cargo/bin", _home / ".local/bin"]
_node = _home / ".nvm/versions/node"
if _node.is_dir():
    _bins += [p / "bin" for p in sorted(_node.iterdir(), key=lambda p: p.stat().st_mtime_ns, reverse=True)]
os.environ["PATH"] = os.pathsep.join(str(p) for p in _bins if p.is_dir()) + os.pathsep + os.environ.get("PATH", "/usr/local/bin:/usr/bin:/bin")

ALLOWED_PACKAGES = {"cargo", "rustc", "nodejs", "npm", "python3", "python3-venv", "python3-pip", "php-cli", "composer", "ruby", "bundler", "golang-go", "docker.io", "docker-compose-v2", "apache2", "nginx", "libapache2-mod-php", "rsync", "openssh-client"}
COMMANDS = {"cargo": "cargo", "rustc": "rustc", "nodejs": "node", "npm": "npm", "python3": "python3", "php-cli": "php", "composer": "composer", "ruby": "ruby", "bundler": "bundle", "golang-go": "go", "docker.io": "docker", "apache2": "apache2ctl", "nginx": "nginx", "rsync":"rsync", "openssh-client":"ssh"}
CONFIG_DIRS = ("/etc/apache2/sites-available", "/etc/nginx/sites-available", "/etc/nginx/conf.d")


def run(args, cwd=None):
    process = subprocess.run(args, cwd=cwd, text=True, capture_output=True, timeout=600)
    if process.returncode:
        raise ValueError("Preparazione non riuscita: " + args[0] + " · " + process.stderr[-1200:].strip())


def safe_directory(path):
    path = pathlib.Path(path)
    if not path.is_absolute() or ".." in path.parts or any(p.is_symlink() for p in (path, *path.parents)):
        raise ValueError("Cartella destinataria non valida")
    if not path.is_dir():
        raise ValueError("Cartella del progetto assente sul destinatario: attiva anche le cartelle")
    return path


def prepare(value):
    packages = value["packages"]
    if not all(p in ALLOWED_PACKAGES for p in packages):
        raise ValueError("Pacchetto non supportato")
    missing = []
    for package in packages:
        if package in COMMANDS and shutil.which(COMMANDS[package]):
            continue
        installed = subprocess.run(["dpkg-query", "-W", "-f=${Status}", package], capture_output=True, text=True)
        if installed.returncode or installed.stdout != "install ok installed":
            missing.append(package)
    if missing:
        if os.geteuid() != 0:
            raise ValueError("Servono privilegi di amministrazione sul destinatario")
        run(["env", "DEBIAN_FRONTEND=noninteractive", "apt-get", "update"])
        run(["env", "DEBIAN_FRONTEND=noninteractive", "apt-get", "install", "-y", "--no-install-recommends", *missing])
    return {"installed": missing}


def site_names(path, service):
    text = path.read_text(errors="replace")
    pattern = r"(?im)^\s*(?:ServerName|ServerAlias)\s+([^\n#]+)" if service == "apache2" else r"(?im)^\s*server_name\s+([^;]+);"
    names = set(name.lower() for match in re.findall(pattern,text) for name in match.split())
    if not names and re.search(r"(?i)<VirtualHost\s",text):
        names.add("__default__")
    return names


def check_site_collision(path, service):
    folder = pathlib.Path("/etc/apache2/sites-enabled" if service == "apache2" else "/etc/nginx/sites-enabled")
    names = site_names(path,service)
    if folder.is_dir():
        for existing in folder.iterdir():
            if existing.is_file() and existing.resolve() != path.resolve() and names.intersection(site_names(existing,service)):
                raise ValueError("Nome del sito già attivo sul destinatario; configurazioni conservate")


def finish(value):
    for directory in value["paths"]:
        root = safe_directory(directory)
        if (root / "package.json").is_file():
            if (root / "package-lock.json").is_file():
                run(["npm", "ci"], cwd=root)
            else:
                run(["npm", "install", "--package-lock=false"], cwd=root)
        if (root / "requirements.txt").is_file() or (root / "pyproject.toml").is_file():
            venv = root / ".venv"
            if any(p.is_symlink() for p in (venv, venv / "bin", venv / "pyvenv.cfg")):
                raise ValueError("Ambiente Python non valido")
            if not (venv / "pyvenv.cfg").exists():
                run(["python3", "-m", "venv", str(venv)], cwd=root)
            command = [str(venv / "bin/python"), "-m", "pip", "install"]
            command += ["-r", "requirements.txt"] if (root / "requirements.txt").is_file() else ["."]
            run(command, cwd=root)
        if (root / "Cargo.toml").is_file():
            run(["cargo", "fetch"], cwd=root)
        if (root / "composer.json").is_file():
            run(["composer", "install", "--no-interaction"], cwd=root)
        if (root / "Gemfile").is_file():
            run(["bundle", "install"], cwd=root)
        if (root / "go.mod").is_file():
            run(["go", "mod", "download"], cwd=root)
    plugins = value["plugins"]
    if plugins:
        base = pathlib.Path.home() / ".config/oaclis/plugins"
        safe_directory(base)
        items = []
        for plugin in plugins:
            name = plugin["name"]
            if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name):
                raise ValueError("Nome plugin non valido")
            root = safe_directory(base / name)
            if not (root / ".codex-plugin/plugin.json").is_file():
                raise ValueError("Manifest plugin assente")
            items.append({"name": name, "source": {"source": "local", "path": "./" + name}, "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"}})
        registry = base / ".agents/plugins"
        if registry.is_symlink() or registry.parent.is_symlink():
            raise ValueError("Registro plugin non valido")
        registry.mkdir(parents=True, exist_ok=True)
        manifest = registry / "marketplace.json"
        if manifest.is_symlink():
            raise ValueError("Manifest marketplace non valido")
        # Keep plugins brought by earlier transfers available in this LAN-only marketplace.
        if manifest.exists():
            previous = json.loads(manifest.read_text()).get("plugins", [])
            items.extend(item for item in previous if item.get("name") not in {p["name"] for p in items})
        manifest.write_text(json.dumps({"name": "oaclis-local", "plugins": items}))
        run(["codex", "plugin", "marketplace", "add", str(base), "--json"])
        for plugin in plugins:
            run(["codex", "plugin", "add", plugin["name"] + "@oaclis-local", "--json"])
    configs = value["configs"]
    apache, nginx, modules = [], [], set()
    for config in configs:
        path = pathlib.Path(config["path"])
        if config["service"] == "apache2":
            for name in config.get("modules",[]):
                if name not in {"rewrite","ssl","proxy","proxy_http","headers","expires"}:
                    raise ValueError("Modulo Apache non supportato")
                modules.add(name)
        if path.name == ".htaccess" and config["service"] == "apache2" and any(pathlib.Path(p) in path.parents for p in value["paths"]):
            safe_directory(path.parent)
            if path.is_symlink() or not path.is_file():
                raise ValueError("Configurazione locale Apache non valida")
            continue
        if str(path.parent) not in CONFIG_DIRS or path.is_symlink() or not path.is_file():
            raise ValueError("Configurazione web non valida")
        (apache if config["service"] == "apache2" else nginx).append(path)
    if apache or modules:
        enabled, enabled_modules = [], []
        try:
            for name in sorted(modules):
                if not (pathlib.Path("/etc/apache2/mods-enabled") / (name + ".load")).exists():
                    run(["a2enmod", name]); enabled_modules.append(name)
            for path in apache:
                check_site_collision(path,"apache2")
                link = pathlib.Path("/etc/apache2/sites-enabled") / path.name
                if link.exists() or link.is_symlink():
                    if not link.is_symlink() or link.resolve() != path.resolve():
                        raise ValueError("Sito Apache già associato a un’altra configurazione")
                    continue
                run(["a2ensite", path.name])
                enabled.append(path.name)
            run(["apache2ctl", "configtest"])
        except Exception:
            for name in enabled:
                subprocess.run(["a2dissite", name], capture_output=True)
            for name in reversed(enabled_modules):
                subprocess.run(["a2dismod", name], capture_output=True)
            raise
        run(["service", "apache2", "reload"])
    if nginx:
        created = []
        try:
            for path in nginx:
                check_site_collision(path,"nginx")
                if path.parent.name != "sites-available":
                    continue
                link = pathlib.Path("/etc/nginx/sites-enabled") / path.name
                if link.exists() or link.is_symlink():
                    if not link.is_symlink() or link.resolve() != path.resolve():
                        raise ValueError("Sito Nginx già associato a un’altra configurazione")
                    continue
                link.symlink_to(path)
                created.append(link)
            run(["nginx", "-t"])
        except Exception:
            for link in created:
                link.unlink()
            raise
        run(["service", "nginx", "reload"])
    return {"prepared": True}


if __name__ == "__main__":
    try:
        if sys.argv[1] == "identity":
            home = pathlib.Path.home()
            print(json.dumps({"home": str(home), "codex_root": str(pathlib.Path(os.environ.get("CODEX_HOME", home / ".codex")).absolute())}))
        elif sys.argv[1] == "verify":
            fresh = True
            for item in json.loads(sys.argv[2]):
                path = pathlib.Path(item["path"])
                if any(p.is_symlink() for p in (path,*path.parents)):
                    fresh = False; break
                stat = path.stat() if path.exists() else None
                if (stat.st_size if stat else None) != item["bytes"] or (str(stat.st_mtime_ns) if stat else None) != item["modified_ns"]:
                    fresh = False; break
            print(json.dumps({"fresh":fresh}))
        else:
            value = json.loads(sys.argv[2])
            print(json.dumps(prepare(value) if sys.argv[1] == "prepare" else finish(value)))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
