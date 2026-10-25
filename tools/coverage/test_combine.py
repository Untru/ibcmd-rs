"""Root assets and non-default root properties must survive combination."""
import json,pathlib,tempfile,unittest
from combine import merge_root_files,declared_configuration_properties,configuration_base

class RootAssets(unittest.TestCase):
    def test_base_prunes_children_without_removing_properties(self):
        text = '<Configuration>\n\t\t<Properties>\n\t\t\t<DefaultLanguage>Language.English</DefaultLanguage>\n\t\t\t<CompatibilityMode>Version8_3_27</CompatibilityMode>\n\t\t</Properties>\n\t\t<ChildObjects>\n\t\t\t<Language>English</Language>\n\t\t\t<Catalog>Old</Catalog>\n\t\t</ChildObjects>\n</Configuration>'
        result = configuration_base(text)
        self.assertIn('<DefaultLanguage>Language.English</DefaultLanguage>', result)
        self.assertIn('<CompatibilityMode>Version8_3_27</CompatibilityMode>', result)
        self.assertIn('<Language>English</Language>', result)
        self.assertNotIn('<Catalog>Old</Catalog>', result)

    def test_configuration_sidecars_are_copied_and_conflicts_refused(self):
        with tempfile.TemporaryDirectory() as td:
            root=pathlib.Path(td);src=root/'Ext';src.mkdir();(src/'ClientApplicationInterface.xml').write_text('<ClientApplicationInterface/>',encoding='utf-8')
            target=root/'tree/Ext';merge_root_files(src,target)
            self.assertEqual((target/'ClientApplicationInterface.xml').read_bytes(),(src/'ClientApplicationInterface.xml').read_bytes())
            merge_root_files(src,target)
            (src/'ClientApplicationInterface.xml').write_text('<different/>',encoding='utf-8')
            with self.assertRaises(SystemExit):merge_root_files(src,target)

    def test_explicit_root_properties_keep_structured_children(self):
        with tempfile.TemporaryDirectory() as td:
            root=pathlib.Path(td);inputs=root/'input';(inputs/'base').mkdir(parents=True)
            (inputs/'values.json').write_text(json.dumps({'configuration_properties':['Permissions']}),encoding='utf-8')
            (inputs/'base/Configuration.xml').write_text('<Configuration><Permissions><Permission>Camera</Permission><Description>Coverage</Description></Permissions></Configuration>',encoding='utf-8')
            self.assertEqual(declared_configuration_properties(root),{'Permissions':'<Permissions><Permission>Camera</Permission><Description>Coverage</Description></Permissions>'})
            (inputs/'base/Configuration.xml').write_text('<Configuration><Permissions/><Permissions/></Configuration>',encoding='utf-8')
            with self.assertRaises(SystemExit):declared_configuration_properties(root)

if __name__=='__main__':unittest.main()
