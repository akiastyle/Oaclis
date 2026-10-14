# app_inventory.py v1.9.195
"""Read project files and installed metadata. Never install or import project code."""
import ast
import functools
import importlib.metadata as metadata
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

home = Path.home()
bins = [home / '.cargo/bin', home / '.local/bin']
node = home / '.nvm/versions/node'
if node.is_dir():
    bins += [p / 'bin' for p in sorted(node.iterdir(), key=lambda p:p.stat().st_mtime_ns, reverse=True)]
os.environ['PATH'] = os.pathsep.join(str(p) for p in bins if p.is_dir()) + os.pathsep + os.environ.get('PATH','/usr/bin:/bin')
SKIP = {'.git','.ssh','.codex','.agents','.config','.cache','.venv','venv','node_modules','target','__pycache__','backups','primary','specialist','_imports','_experiments'}
COMMANDS = {'python3':'Python 3','node':'Node.js','npm':'npm','cargo':'Cargo','rustc':'Rust','php':'PHP','composer':'Composer','ruby':'Ruby','bundle':'Bundler','go':'Go','docker':'Docker','apache2ctl':'Apache','nginx':'Nginx','pdftoppm':'Poppler','pdftotext':'Poppler','ffmpeg':'FFmpeg','mutool':'MuPDF','potrace':'Potrace','resvg':'resvg','tesseract':'Tesseract','gs':'Ghostscript','sqlite3':'SQLite CLI','chromium':'Chromium'}

def safe_root(value):
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts or len(path.parts)<3 or path == home or path in (Path('/var/www'),Path('/var/www/html')) or re.match(r'^/mnt/[a-zA-Z](?:/|$)',str(path)):
        raise ValueError('Radice progetto non valida')
    if any(p.is_symlink() for p in (path,*path.parents)) or any(p in {'.git','.ssh','.codex','.agents','.config','.cache','.venv','venv','node_modules','target','__pycache__'} for p in path.parts):
        raise ValueError('Radice progetto non valida')
    if path.parts[1] in ('etc','usr','proc','sys','dev','run','bin','lib','boot'):
        raise ValueError('Radice progetto non valida')
    return path


def scan_files(root):
    count=0
    for directory, dirs, names in os.walk(root,followlinks=False):
        folder=Path(directory)
        dirs[:] = sorted(d for d in dirs if d not in SKIP and not (folder/d).is_symlink()) if len(folder.relative_to(root).parts)<5 else []
        for name in sorted(names):
            path=folder/name
            if name.startswith(('test_','test-','check_','check-')) or path.is_symlink():continue
            if path.suffix=='.py' or name in ('package.json','Cargo.toml','composer.json','Gemfile','go.mod','requirements.txt','pyproject.toml'):
                count+=1
                if count>2000:return
                if path.is_file() and path.stat().st_size<=131072:yield path


@functools.lru_cache(maxsize=64)
def files(root):
    return tuple(scan_files(root))


def python_distributions(roots):
    paths=[]
    for root in roots:
        for base in (root/'.venv',root/'venv',root.parent/'.venv',root.parent/('.venv-'+root.name)):
            if base.is_dir() and not base.is_symlink():
                paths += [str(p) for p in base.glob('lib/python*/site-packages') if p.is_dir() and not p.is_symlink()]
    distributions=list(metadata.distributions())+list(metadata.distributions(path=paths))
    versions={};modules={}
    for dist in distributions:
        name=dist.metadata.get('Name','')
        if not name:continue
        versions[name.lower().replace('_','-')]=(name,dist.version)
        top=dist.read_text('top_level.txt') or ''
        tops=top.splitlines() or [str(p).split('/')[0].removesuffix('.py') for p in (dist.files or [])] or [name.replace('-','_')]
        for module in tops:
            if re.fullmatch(r'[A-Za-z_]\w*',module):modules[module]=(name,dist.version)
    return versions,modules


def module_version(name):
    if not re.fullmatch(r'[A-Za-z_]\w*',name):return None
    try:spec=importlib.util.find_spec(name)
    except (ModuleNotFoundError,ValueError):return None
    if spec is None:return None
    if spec.origin and Path(spec.origin).suffix=='.py':
        try:
            for node in ast.walk(ast.parse(Path(spec.origin).read_text(errors='replace'))):
                if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='__version__' for t in node.targets) and isinstance(node.value,ast.Constant) and isinstance(node.value.value,str):return node.value.value
        except (OSError,SyntaxError):pass
    return ''


def service_info(port):
    # Linux exposes the actual listener's working directory; no service is guessed from a mention.
    if not Path('/proc').is_dir() or not shutil.which('ss'):return None
    result=subprocess.run(['ss','-ltnp','sport = :'+str(int(port))],capture_output=True,text=True,timeout=3)
    match=re.search(r'pid=(\d+)',result.stdout)
    if not match:return None
    process=Path('/proc')/match.group(1)
    try:
        cwd=(process/'cwd').resolve()
        args=(process/'cmdline').read_bytes().split(b'\0')
        for base in (cwd,*cwd.parents):
            package=base/'package.json'
            if package.is_file() and not package.is_symlink():
                data=json.loads(package.read_text())
                return {'name':str(data.get('name',base.name)),'version':str(data.get('version','')),'source_root':str(base),'runtime':'node' if args and b'node' in args[0] else ''}
    except (OSError,ValueError):pass
    return None


def installed(item, roots, py):
    kind,name=item['kind'],item['name']
    if kind=='command':
        if name not in COMMANDS:raise ValueError('Comando non supportato')
        command=shutil.which(name)
        if not command:return None
        option={'ffmpeg':'-version','pdftoppm':'-v','pdftotext':'-v','sqlite3':'-version','gs':'--version','apache2ctl':'-v','nginx':'-v'}.get(name,'--version')
        try:
            p=subprocess.run([command,option],capture_output=True,text=True,timeout=3)
            text=(p.stdout or p.stderr).splitlines()
            match=re.search(r'\d+(?:\.\d+){1,3}',text[0] if text else '')
            return match.group() if match else ''
        except (OSError,subprocess.SubprocessError):return ''
    if kind=='python':
        if not re.fullmatch(r'[A-Za-z0-9_.-]+',name):raise ValueError('Libreria non valida')
        version=py[0].get(name.lower().replace('_','-'),(None,None))[1]
        return version if version is not None else module_version(item.get('module',name.replace('-','_')))
    if kind=='node':
        if not re.fullmatch(r'(?:@[A-Za-z0-9_.-]+/)?[A-Za-z0-9_.-]+',name) or name in ('.','..'):raise ValueError('Libreria non valida')
        for root in roots:
            for path in [root/'node_modules'/name/'package.json',*(p.parent/'node_modules'/name/'package.json' for p in files(root) if p.name=='package.json')]:
                if path.is_file() and not path.is_symlink():
                    value=json.loads(path.read_text());return str(value.get('version',''))
        return None
    if kind=='service':
        port=int(item['port'])
        if not 0<port<65536:raise ValueError('Porta non valida')
        info=service_info(port)
        return info['version'] if info and info['name']==name else None
    raise ValueError('Tipo requisito non valido')


def source(value):
    roots=[safe_root(p) for p in value['paths']];py=python_distributions(roots);items={};ports={}
    def add(kind,name,evidence,label=None,**details):
        if (kind,name) in items:return
        item={'kind':kind,'name':name,'label':label or name,'evidence':str(evidence),**details}
        version=installed(item,roots,py)
        if version is not None:items[(kind,name)]=dict(item,version=version)
    for root in roots:
        local={p.stem for p in root.glob('*.py')} | {p.name for p in root.iterdir() if p.is_dir()}
        for path in files(root):
            if path.suffix=='.py':
                add('command','python3',path,COMMANDS['python3'])
                text=path.read_text(errors='replace')
                for port in re.findall(r'https?://(?:localhost|127\.0\.0\.1|\[::1\]):(\d+)',text):
                    if 0<int(port)<65536:ports[int(port)]=str(path)
                try:tree=ast.parse(text)
                except SyntaxError:continue
                for node in ast.walk(tree):
                    names=[a.name.split('.')[0] for a in node.names] if isinstance(node,ast.Import) else [node.module.split('.')[0]] if isinstance(node,ast.ImportFrom) and node.module and not node.level else []
                    local_names = local | {p.stem for p in path.parent.glob('*.py')}
                    for name in names:
                        if name in sys.stdlib_module_names or name in local_names:continue
                        package = py[1].get(name,(name,''))[0]
                        add('python',package,path,{'spacy':'spaCy','PIL':'Pillow'}.get(name,package),module=name)
                    if isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute) and isinstance(node.func.value,ast.Name) and node.func.value.id=='subprocess' and node.func.attr in ('run','Popen','check_output','check_call') and node.args and isinstance(node.args[0],(ast.List,ast.Tuple)):
                        args=node.args[0].elts
                        if args and isinstance(args[0],ast.Constant) and isinstance(args[0].value,str):
                            command=Path(args[0].value).name
                            if command in COMMANDS:add('command',command,path,COMMANDS[command])
                    if isinstance(node,ast.Constant) and isinstance(node.value,str) and re.fullmatch(r'(?:en|it)_core_(?:web|news)_[a-z]+',node.value):add('python',node.value,path)
            elif path.name=='package.json':
                add('command','node',path,COMMANDS['node']);add('command','npm',path,COMMANDS['npm'])
                package=json.loads(path.read_text());declared={**package.get('dependencies',{}),**package.get('devDependencies',{})}
                for name in declared:add('node',name,path)
            else:
                commands={'Cargo.toml':['cargo','rustc'],'composer.json':['php','composer'],'Gemfile':['ruby','bundle'],'go.mod':['go'],'requirements.txt':['python3'],'pyproject.toml':['python3']}.get(path.name,[])
                for name in commands:add('command',name,path,COMMANDS[name])
    for port,evidence in ports.items():
        info=service_info(port)
        if info:
            if not any(Path(info['source_root']).is_relative_to(root) for root in roots):
                items[('service',info['name'])]=dict(info,kind='service',label={'openwa':'OpenWA'}.get(info['name'],info['name']),port=port,evidence=evidence)
            if info['runtime']=='node':
                add('command','node',evidence,COMMANDS['node']);add('command','npm',evidence,COMMANDS['npm'])
    for config in value.get('configs',[]):
        name='apache2ctl' if config['service']=='apache2' else 'nginx'
        add('command',name,config['path'],COMMANDS[name])
    for plugin in value.get('plugins',[]):
        path=Path(plugin['path'])
        if path.is_dir():items[(plugin['kind'],plugin['name'])]={'kind':plugin['kind'],'name':plugin['name'],'label':plugin['name'],'version':'','evidence':str(path),'source_path':str(path)}
    paths=sorted({str(root) for root in roots} | {p['source_root'] for p in items.values() if p['kind']=='service'})
    paths=[p for p in paths if not any(p!=other and Path(other) in Path(p).parents for other in paths)]
    return {'paths':paths,'requirements':sorted(items.values(),key=lambda p:(p['kind'],p['name']))}


if __name__=='__main__':
    try:
        value=json.loads(sys.argv[2]);print(json.dumps(source(value)))
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        print(str(error),file=sys.stderr);sys.exit(1)
