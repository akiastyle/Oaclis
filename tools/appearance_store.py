# appearance_store.py v1.2.159
"""Oaclis storage adapter; chat metadata lives in the appearance record."""
import json
import os
import sqlite3
import sys
from pathlib import Path
if 'unified_ensure' not in globals():
    from chat_records import *

path, operation = Path(sys.argv[1]), sys.argv[2]
key = sys.argv[3] if len(sys.argv) > 3 else ''
config = path.parent
os.umask(0o077)
READS = {'analysis-list', 'inventory-list', 'inventory-get', 'transfer-list', 'links-get', 'preferences-get', 'status-read', 'preparation-get', 'load', 'analysis-get'}

def values(db, table):
    exists = db.execute("SELECT 1 FROM sqlite_master WHERE name=?", (table,)).fetchone()
    return {key:json.loads(raw) for key,raw in db.execute('SELECT key,value FROM ' + table + ' WHERE value IS NOT NULL ORDER BY key')} if exists else {}

def read_data(db):
    columns = {row[1] for row in db.execute('PRAGMA table_info(appearance)')}
    if 'sync' in columns:
        inventories, preparations = unified_data(db, config)
        preferences = unified_preferences(db)
    else:
        inventories, preparations = values(db, 'app_checks'), values(db, 'preparations')
        preferences = {}
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        visibility = dict(db.execute('SELECT id,hidden FROM chat_visibility')) if 'chat_visibility' in tables else {}
        if 'chat_settings' in tables:
            preferences = {chat:{'hidden':bool(hidden or visibility.get(chat,False)), **({'scopes':json.loads(scopes)} if scopes else {})} for chat,hidden,scopes in db.execute('SELECT id,hidden,scopes FROM chat_settings')}
    return {'preferences':preferences, 'links':list(values(db,'chat_links').values()), 'analyses':values(db,'analysis'), 'app_checks':inventories, 'preparations':preparations, 'transfers':values(db,'transfers')}

if operation in READS:
    data = {'preferences':{},'links':[],'analyses':{},'app_checks':{},'preparations':{},'transfers':{}}
    with sqlite3.connect(path.absolute().as_uri() + '?mode=ro',uri=True,timeout=10) if path.is_file() else __import__('contextlib').nullcontext(None) as db:
        if db:
            db.execute('BEGIN')
            if operation == 'load':
                rows = values(db,'appearance')
                columns = {row[1] for row in db.execute('PRAGMA table_info(appearance)')}
                if 'sync' in columns:
                    removed = {key for key,raw in db.execute('SELECT key,sync FROM appearance') if json.loads(raw).get('removed')}
                    rows = {key:value for key,value in rows.items() if key not in removed}
                print(json.dumps(rows));sys.exit(0)
            data = read_data(db)
        if operation in ('status-read','preferences-get'):
            result = data if operation == 'status-read' else {name:data[name] for name in ('preferences','links')}
        elif operation == 'analysis-get':
            result = data['analyses'].get(key, {'status':'missing'})
            scopes = data['preferences'].get(key.split(':',1)[-1],{}).get('scopes')
            if scopes is None and db and 'scopes' in {row[1] for row in db.execute('PRAGMA table_info(analysis)')}:
                row = db.execute('SELECT scopes FROM analysis WHERE key=?',(key,)).fetchone()
                scopes = json.loads(row[0]) if row and row[0] else None
            result['scopes'] = scopes or dict.fromkeys(MODULES,True)
        elif operation in ('inventory-get','preparation-get'):

            result = data['app_checks' if operation=='inventory-get' else 'preparations'].get(key,{'status':'missing'})
        else:
            field = {'inventory-list':'app_checks','analysis-list':'analyses','transfer-list':'transfers','links-get':'links','load':'appearance'}[operation]
            result = data.get(field,{})
        print(json.dumps(result))
    sys.exit(0)

with sqlite3.connect(path,timeout=10) as db:
    db.execute('BEGIN IMMEDIATE')
    unified_ensure(db, config)
    if operation == 'migrate':
        result = {'ok':True}
    elif operation == 'chat-observed':
        observed=json.load(sys.stdin)
        source,chat=key.split(':',1)
        source=source_mapping(config).get(source,source)
        value=chat_object(db,chat)
        record=value.setdefault('chat',{}).setdefault('sources',{}).setdefault(source,{})
        record.update({name:observed[name] for name in ('bytes','modified_ns','sha256')})
        save_chat_object(db,chat,value)
        result=observed
    elif operation in ('inventory-save','preparation-save'):
        value = json.load(sys.stdin)
        (save_inventory if operation=='inventory-save' else save_preparation)(db,key,value,config)
        result = value
    elif operation in ('chat-visibility','chat-hide','chat-scopes','device-forget'):
        if operation.startswith('chat-'):
            value = chat_object(db,key)
            save_chat_object(db,key,value)
            if operation == 'chat-scopes':
                scopes = json.load(sys.stdin)
                if set(scopes)!=set(MODULES) or any(type(v)is not bool for v in scopes.values()):
                    raise ValueError('Moduli non validi')
                set_switches(db,key,scopes)
            else:
                hidden = True if operation=='chat-hide' else json.load(sys.stdin)['hidden']
                if type(hidden)is not bool: raise ValueError('Visibilità non valida')
                if operation=='chat-hide': value['removed']=1
                elif not hidden: value.pop('removed',None)
                save_chat_object(db,key,value)
                db.execute('UPDATE appearance SET hidden=? WHERE key=?',(int(hidden),'chat:'+key))
        links = values(db,'chat_links')
        for link_key,link in links.items():
            match = link['entry']['id']==key if operation.startswith('chat-') else key in (link['source'],link['target'])
            if not match: continue
            if operation in ('chat-hide','device-forget'): db.execute('DELETE FROM chat_links WHERE key=?',(link_key,))
            elif operation=='chat-scopes':
                link['scopes']=scopes;db.execute('UPDATE chat_links SET value=? WHERE key=?',(json.dumps(link),link_key))
        if operation=='device-forget': db.execute('DELETE FROM appearance WHERE key=?',('device:'+key,))
        if operation in ('chat-hide','device-forget'):
            for analysis_key in values(db,'analysis'):
                source,chat=analysis_key.split(':',1)
                if chat==key if operation=='chat-hide' else source==key: db.execute('DELETE FROM analysis WHERE key=?',(analysis_key,))
        data=read_data(db);result={name:data[name] for name in ('preferences','links')}
    elif operation.startswith('links-'):
        db.execute('CREATE TABLE IF NOT EXISTS chat_links (key TEXT PRIMARY KEY,value TEXT NOT NULL)')
        if operation=='links-flow':
            update=json.load(sys.stdin)
            row=db.execute('SELECT value FROM chat_links WHERE key=?',(key,)).fetchone()
            if row:
                saved=json.loads(row[0])
                if saved.get('flow',{}).get('generation') == update.get('generation'):
                    saved['flow']=update
                    saved['transfer_pending']=not bool(update.get('initialized'))
                    db.execute('UPDATE chat_links SET value=? WHERE key=?',(json.dumps(saved),key))
        elif operation=='links-save':
            value=json.load(sys.stdin);chat=value['entry']['id'];record=chat_object(db,chat)
            existing=db.execute('SELECT value FROM chat_links WHERE key=?',(key,)).fetchone()
            if existing:
                existing=json.loads(existing[0])
                if any(existing.get(name)!=value.get(name) for name in ('source','target','operation')):
                    raise ValueError('Elimina e ricrea il collegamento per cambiarne la direzione')
                if 'flow' in existing:value['flow']=existing['flow']
            if record.get('removed') and not db.execute('SELECT 1 FROM chat_links WHERE key=?',(key,)).fetchone(): raise ValueError('Chat rimossa da Oaclis')
            preference=unified_preferences(db).get(chat)
            if preference:value['scopes']=preference['scopes']
            if 'flow' in value:value['transfer_pending']=not bool(value['flow'].get('initialized'))
            db.execute('INSERT INTO chat_links VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',(key,json.dumps(value)))
        elif operation=='links-delete':db.execute('DELETE FROM chat_links WHERE key=?',(key,))
        result=list(values(db,'chat_links').values())
    elif operation=='transfer-save':
        db.execute('CREATE TABLE IF NOT EXISTS transfers (key TEXT PRIMARY KEY,value TEXT NOT NULL)')
        db.execute('INSERT INTO transfers VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',(key,json.dumps(json.load(sys.stdin))))
        result=values(db,'transfers')
    elif operation.startswith('analysis-'):
        db.execute('CREATE TABLE IF NOT EXISTS analysis (key TEXT PRIMARY KEY,value TEXT)')
        if operation=='analysis-save':
            db.execute('INSERT INTO analysis(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',(key,json.dumps(json.load(sys.stdin))))
            chat=key.split(':',1)[-1];record=chat_object(db,chat)
            for name in MODULES:record.setdefault(name,{}).setdefault('enabled',1)
            save_chat_object(db,chat,record)
        elif operation=='analysis-scopes':set_switches(db,key.split(':',1)[-1],json.load(sys.stdin))
        result=values(db,'analysis').get(key,{'status':'missing'})
        result['scopes']=unified_preferences(db).get(key.split(':',1)[-1],{}).get('scopes',dict.fromkeys(MODULES,True))
    elif operation=='save':
        entries=json.load(sys.stdin)
        db.executemany('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',[(key,json.dumps(value)) for key,value in entries.items()])
        result=None
    else:raise ValueError('Operazione DB non valida')
    if result is not None:print(json.dumps(result))
