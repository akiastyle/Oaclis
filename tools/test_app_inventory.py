# test_app_inventory.py v1.4.55
"""Read-only dependency checks: files are evidence, not executable installers."""
import importlib.util
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('inventory',Path(__file__).with_name('app_inventory.py'))
inventory=importlib.util.module_from_spec(spec);spec.loader.exec_module(inventory)

class InventoryTests(unittest.TestCase):
    def test_only_project_dependencies_and_actual_source_versions_are_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'app';root.mkdir()
            marker=root/'executed'
            (root/'server.py').write_text('import json\nimport sqlite3\nimport spacy\nfrom pathlib import Path\nPath('+repr(str(marker))+').touch()\n# ChatGPT Windows Android browser Codex\n')
            (root/'check-browser.py').write_text('import playwright\n')
            py=({'spacy':('spacy','3.8.16')},{'spacy':('spacy','3.8.16')})
            with patch.object(inventory,'python_distributions',return_value=py),patch.object(inventory.shutil,'which',return_value='/usr/bin/python3'),patch.object(inventory.subprocess,'run',return_value=types.SimpleNamespace(stdout='Python 3.12.3\n',stderr='')):
                source=inventory.source({'paths':[str(root)],'configs':[],'plugins':[]})
            self.assertEqual([(r['name'],r['version']) for r in source['requirements']],[('python3','3.12.3'),('spacy','3.8.16')])
            self.assertFalse(marker.exists())
            self.assertEqual(source['paths'],[str(root)])

    def test_node_versions_come_from_installed_package_files_and_never_run_scripts(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'app';root.mkdir()
            (root/'package.json').write_text(json.dumps({'dependencies':{'library-one':'^2'},'scripts':{'postinstall':'do not execute'}}))
            lib=root/'node_modules/library-one';lib.mkdir(parents=True)
            (lib/'package.json').write_text('{"version":"2.4.1"}')
            with patch.object(inventory,'python_distributions',return_value=({},{})),patch.object(inventory.shutil,'which',return_value=None):
                result=inventory.source({'paths':[str(root)],'configs':[],'plugins':[]})
            self.assertEqual([(r['name'],r['version']) for r in result['requirements']],[('library-one','2.4.1')])
            with self.assertRaisesRegex(ValueError,'Libreria'):
                inventory.installed({'kind':'node','name':'../private'},[root],({},{}))

    def test_local_service_is_identified_from_the_actual_source_listener(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'app';root.mkdir()
            (root/'whatsapp.py').write_text("URL='http://127.0.0.1:2785/api/sessions'\n")
            service={'name':'openwa','version':'0.1.6','source_root':str(root.parent/'external-openwa'),'runtime':'node'}
            with patch.object(inventory,'python_distributions',return_value=({},{})),patch.object(inventory.shutil,'which',return_value=None),patch.object(inventory,'service_info',return_value=service):
                source=inventory.source({'paths':[str(root)],'configs':[],'plugins':[]})
            requirement=source['requirements'][0]
            self.assertEqual((requirement['name'],requirement['version'],requirement['port']),('openwa','0.1.6',2785))
            self.assertIn(service['source_root'],source['paths'])

    def test_generic_and_windows_roots_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(inventory.safe_root(str(Path(directory)/'not-created')),Path(directory)/'not-created')
        for path in ('/var/www/html','/mnt/c/Users/test','/etc/ssh'):
            with self.assertRaises(ValueError):inventory.safe_root(path)

if __name__=='__main__':unittest.main()
