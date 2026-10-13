"""Targeted checks: model boundary and destination commands, using isolated fixtures."""
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import types
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).parent / (name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


class EnvironmentTests(unittest.TestCase):
    def test_analysis_only_accepts_observed_projects(self):
        analyzer = module("environment_analysis")
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory) / "app"
            root.mkdir()
            (root / "package.json").write_text('{"name":"fixture"}')
            chat = pathlib.Path(directory) / "chat.jsonl"
            chat.write_text(json.dumps({"type":"session_meta","payload":{"id":"fixture","cwd":str(root)}}) + "\n" + json.dumps({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"Work on this project; password=private-value"}]}}) + "\n")
            def model(args, **kwargs):
                self.assertIn("--ephemeral", args)
                self.assertEqual(args[args.index("--sandbox")+1], "read-only")
                self.assertNotIn("private-value", kwargs["input"])
                pathlib.Path(args[args.index("--output-last-message")+1]).write_text(json.dumps({"paths":[str(root)],"plugins":[],"uncertainties":[]}))
                return types.SimpleNamespace(returncode=0)
            with patch.object(analyzer.subprocess,"run",side_effect=model):
                result = analyzer.analyze(chat,"fixture")
            self.assertEqual(result["paths"],[str(root)])
            self.assertEqual(result["packages"],["nodejs","npm"])
            setup = pathlib.Path(__file__).parent / "environment_setup.py"
            def fresh():
                return json.loads(subprocess.check_output(["python3",str(setup),"verify",json.dumps(result["fingerprints"])],text=True))["fresh"]
            self.assertTrue(fresh())
            (root/"package.json").write_text('{"name":"updated-fixture"}')
            self.assertFalse(fresh())
            (pathlib.Path(directory)/"link").symlink_to(root,target_is_directory=True)
            self.assertFalse(analyzer.project(pathlib.Path(directory)/"link"))
            self.assertFalse(analyzer.project(pathlib.Path.home()))
            with self.assertRaises(ValueError):
                analyzer.collect(chat,"different-chat")

    def test_inaccessible_historical_paths_do_not_block_analysis(self):
        analyzer = module("environment_analysis")
        blocked = pathlib.Path("/mnt/c/Users/Administrator/AppData/Local")
        original = pathlib.Path.stat
        def stat(path, *args, **kwargs):
            if path == blocked or blocked in path.parents:
                raise PermissionError(13, "Permission denied", str(path))
            return original(path, *args, **kwargs)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory) / "project"
            root.mkdir()
            chat = pathlib.Path(directory) / "chat.jsonl"
            chat.write_text(json.dumps({"type":"session_meta","payload":{"id":"fixture","cwd":str(root)}}) + "\n" + json.dumps({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":str(blocked)}]}}) + "\n")
            with patch.object(pathlib.Path, "stat", stat):
                self.assertFalse(analyzer.project(blocked))
                self.assertFalse(analyzer.regular(blocked / "package.json"))
                evidence = analyzer.collect(chat, "fixture")
            self.assertEqual([fact["path"] for fact in evidence["projects"]], [str(root)])
            self.assertIn(str(blocked), evidence["conversation"])

    def test_windows_paths_are_never_accessed(self):
        analyzer = module("environment_analysis")
        original = pathlib.Path.stat
        def stat(path, *args, **kwargs):
            if analyzer.windows_path(path):
                raise AssertionError("Windows filesystem must not be accessed")
            return original(path, *args, **kwargs)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory) / "project"
            root.mkdir()
            chat = pathlib.Path(directory) / "chat.jsonl"
            chat.write_text(json.dumps({"type":"session_meta","payload":{"id":"fixture","cwd":str(root)}}) + "\n" + json.dumps({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"/mnt/c/Users/Administrator/AppData/Local /mnt/d/project /mnt/Z/project"}]}}) + "\n")
            with patch.object(pathlib.Path, "stat", stat):
                self.assertFalse(analyzer.project(pathlib.Path("/mnt/c/project")))
                self.assertFalse(analyzer.regular(pathlib.Path("/mnt/d/package.json")))
                facts = analyzer.collect(chat, "fixture")["projects"]
            self.assertEqual([fact["path"] for fact in facts], [str(root)])
            self.assertFalse(analyzer.windows_path(pathlib.Path("/mnt/data/project")))

    def test_associations_persist_without_overwriting_other_chats(self):
        with tempfile.TemporaryDirectory() as directory:
            db = pathlib.Path(directory) / "appearance.sqlite"
            script = pathlib.Path(__file__).parent / "appearance_store.py"
            def call(operation, key="", value=None):
                return json.loads(subprocess.check_output(["python3", str(script), str(db), operation, key], input=json.dumps(value) if value is not None else "", text=True))
            self.assertEqual(call("links-get"), [])
            call("links-save", "one", {"operation":"send"})
            call("links-save", "two", {"operation":"sync"})
            self.assertEqual(call("links-save", "one", {"operation":"receive"}), [{"operation":"receive"}, {"operation":"sync"}])
            self.assertEqual(call("links-get"), [{"operation":"receive"}, {"operation":"sync"}])
            self.assertEqual(call("links-delete", "one"), [{"operation":"sync"}])
            self.assertEqual(call("links-get"), [{"operation":"sync"}])

    def test_packages_are_fixed_and_existing_tools_are_reused(self):
        setup = module("environment_setup")
        commands = []
        with patch.object(setup.shutil,"which",return_value="/usr/bin/node"):
            self.assertEqual(setup.prepare({"packages":["nodejs"]}),{"installed":[]})
        with self.assertRaises(ValueError):
            setup.prepare({"packages":["nginx; rm -rf /"]})
        with patch.object(setup.shutil,"which",return_value=None), patch.object(setup.subprocess,"run",return_value=types.SimpleNamespace(returncode=1,stdout="")), patch.object(setup,"run",side_effect=lambda args, **kwargs:commands.append(args)), patch.object(setup.os,"geteuid",return_value=0):
            result=setup.prepare({"packages":["nginx"]})
        self.assertEqual(result["installed"],["nginx"])
        self.assertEqual(commands[-1][-1],"nginx")
        with tempfile.TemporaryDirectory() as directory:
            root=pathlib.Path(directory)/"app";root.mkdir()
            (root/"package.json").write_text('{}');(root/"package-lock.json").write_text('{}')
            commands.clear()
            with patch.object(setup,"run",side_effect=lambda args, **kwargs:commands.append(args)):
                setup.finish({"paths":[str(root)],"plugins":[],"configs":[]})
            self.assertEqual(commands,[["npm","ci"]])

    def test_analysis_keeps_model_without_external_tools(self):
        analyzer = module("environment_analysis")
        with tempfile.TemporaryDirectory() as directory:
            (pathlib.Path(directory)/"config.toml").write_text('model = "configured-model"\nmodel_provider = "local"\nsandbox_mode = "danger-full-access"\n[model_providers.local]\nname = "Local"\nbase_url = "http://localhost:11434/v1"\n[hooks]\nenabled = true\n')
            with patch.dict(analyzer.os.environ,{"CODEX_HOME":directory}):
                args = analyzer.codex_model_config()
            self.assertIn('model="configured-model"',args)
            self.assertTrue(any("http://localhost:11434/v1" in arg for arg in args))
            self.assertFalse(any("hooks" in arg or "danger-full-access" in arg for arg in args))

    def test_analysis_error_is_actionable_and_redacts_credentials(self):
        analyzer = module("environment_analysis")
        result = types.SimpleNamespace(returncode=1, stderr="ERROR: 401 Unauthorized token=private-value")
        message = analyzer.analysis_failure(result)
        self.assertIn("codex login --device-auth", message)
        self.assertNotIn("private-value", message)
        result.stderr = "ERROR: model unavailable"
        self.assertIn("model unavailable", analyzer.analysis_failure(result))

    def test_analysis_database_preserves_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            db = pathlib.Path(directory)/"appearance.sqlite"
            script=pathlib.Path(__file__).parent/"appearance_store.py"
            def call(mode,value=None):
                output=subprocess.run(["python3",str(script),str(db),mode,"master:fixture"],input=json.dumps(value) if value is not None else None,text=True,capture_output=True,check=True)
                return json.loads(output.stdout)
            self.assertEqual(call("analysis-get")["scopes"],{"chat":True,"paths":True,"apps":True})
            call("analysis-scopes",{"chat":True,"paths":False,"apps":False})
            record=call("analysis-save",{"status":"ready","manifest":{"paths":[]}})
            self.assertFalse(record["scopes"]["apps"])
            output=subprocess.check_output(["python3",str(script),str(db),"load"],text=True)
            self.assertEqual(json.loads(output),{})


if __name__ == "__main__":
    unittest.main()
