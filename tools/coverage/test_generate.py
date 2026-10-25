"""Deterministic generation must include structural and typed property inputs."""
import pathlib, tempfile, unittest
from generate import input_file, set_property, write_body_files, combination_template

class StructuralInputs(unittest.TestCase):
    def test_structural_template_keeps_the_class_and_owner(self):
        with tempfile.TemporaryDirectory() as td:
            inputs = pathlib.Path(td)
            base = '<MetaDataObject><Catalog uuid="a"><Properties><Name>Template</Name></Properties><ChildObjects/></Catalog></MetaDataObject>'
            richer = base.replace('<ChildObjects/>', '<ChildObjects><Form>ObjectForm</Form></ChildObjects>')
            (inputs / 'owned.xml').write_text(richer, encoding='utf-8')
            self.assertEqual(combination_template(inputs, base, {'template': 'owned.xml'}), richer)
            self.assertEqual(combination_template(inputs, base, {}), base)
            (inputs / 'owned.xml').write_text(richer.replace('<Name>Template</Name>', '<Name>Other</Name>'), encoding='utf-8')
            with self.assertRaises(SystemExit):
                combination_template(inputs, base, {'template': 'owned.xml'})

    def test_typed_property_preserves_type(self):
        xml='<Properties><Value xsi:type="v8ui:Color">#000000</Value></Properties>'
        self.assertEqual(set_property(xml,'Value','enum','web:Tomato'),xml.replace('#000000','web:Tomato'))

    def test_body_written_deterministically_for_each_owner(self):
        with tempfile.TemporaryDirectory() as td:
            root=pathlib.Path(td);inputs=root/'input';inputs.mkdir()
            (inputs/'body.xml').write_text('<Form><Name>CovTemplate</Name><Ref>CommonForm.CovTemplate</Ref></Form>',encoding='utf-8')
            args=(inputs,root/'out',{'Ext/Form.xml':'body.xml'},'CovTemplate','CovOne','CommonForms')
            write_body_files(*args);one=(root/'out/CommonForms/CovOne/Ext/Form.xml').read_bytes()
            write_body_files(*args);self.assertEqual(one,(root/'out/CommonForms/CovOne/Ext/Form.xml').read_bytes())
            self.assertIn(b'CommonForm.CovOne',one)
            write_body_files(inputs,root/'out',{'Ext/Form.xml':'body.xml'},'CovTemplate','CovTwo','CommonForms')
            self.assertIn(b'CovTwo',(root/'out/CommonForms/CovTwo/Ext/Form.xml').read_bytes())

    def test_paths_cannot_escape_committed_inputs(self):
        with tempfile.TemporaryDirectory() as td:
            inputs=pathlib.Path(td)
            for path in ('../outside.xml','/outside.xml','C:/outside.xml','..\\outside.xml'):
                with self.subTest(path=path), self.assertRaises(SystemExit):input_file(inputs,path)
            with self.assertRaises(SystemExit):write_body_files(inputs,inputs,{'../../escape.xml':'body.xml'},'T','N','CommonForms')

    def test_binary_assets_keep_their_bytes(self):
        with tempfile.TemporaryDirectory() as td:
            root=pathlib.Path(td);inputs=root/'input';inputs.mkdir();data=b'\x89PNG\r\n\x00\xff'
            (inputs/'pixel.png').write_bytes(data)
            write_body_files(inputs,root/'tree',{'Ext/Template/Pixel.png':{'path':'pixel.png','binary':True}},'T','N','CommonTemplates')
            self.assertEqual((root/'tree/CommonTemplates/N/Ext/Template/Pixel.png').read_bytes(),data)

if __name__=='__main__':unittest.main()
