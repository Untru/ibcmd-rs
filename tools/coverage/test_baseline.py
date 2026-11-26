"""A same-class set must never silently redirect another set's baseline."""
import json,pathlib,tempfile,unittest
import update_baseline

class BaselineIdentity(unittest.TestCase):
    def test_duplicate_set_identifiers_are_refused(self):
        with tempfile.TemporaryDirectory() as td:
            root=pathlib.Path(td)/"coverage";root.mkdir()
            for name in ['one','two']:
                path=root/name;path.mkdir();(path/'manifest.json').write_text(json.dumps({'set':'same-name'}),encoding='utf-8')
            original=update_baseline.ROOT
            corpus_original=update_baseline.CORPUS_ROOT
            (root.parent/"corpus-index.json").write_text("{}",encoding="utf-8")
            try:
                update_baseline.ROOT=root
                update_baseline.CORPUS_ROOT=root
                with self.assertRaises(SystemExit):update_baseline.set_dirs()
            finally:
                update_baseline.ROOT=original
                update_baseline.CORPUS_ROOT=corpus_original

if __name__=='__main__':unittest.main()
