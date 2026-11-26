import json
import unittest
import xml.etree.ElementTree as ET
from observed_problems import canonical, classify, parse_xml

NS = 'urn:coverage:form'

def form(value):
    body = '' if value is None else '<Height>' + value + '</Height>'
    return ET.fromstring('<Form xmlns="' + NS + '"><InputField name="Probe">' + body + '</InputField></Form>')

class WitnessContract(unittest.TestCase):
    def setUp(self):
        self.native = form('2')
        self.case = {'id': 'height', 'anchor': {'tag': '{' + NS + '}InputField', 'name': 'Probe'},
                     'property': '{' + NS + '}Height',
                     'native': json.loads(json.dumps(canonical(self.native[0][0]))), 'known_symptom': None}

    def test_retained_widget_with_missing_property_is_the_known_symptom(self):
        self.assertEqual(classify(self.native, form(None), self.case), 'known-symptom')

    def test_an_actual_fix_is_reported_as_equal(self):
        self.assertEqual(classify(self.native, form('2'), self.case), 'equal')

    def test_missing_widget_is_not_a_missing_property_reproduction(self):
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, ET.fromstring('<Form/>'), self.case)

    def test_wrong_value_is_an_unrelated_regression(self):
        with self.assertRaisesRegex(ValueError, 'unrelated mismatch'):
            classify(self.native, form('3'), self.case)

    def test_discarded_native_property_invalidates_the_witness(self):
        with self.assertRaisesRegex(ValueError, 'native no longer retains'):
            classify(form(None), form(None), self.case)

    def test_an_element_in_another_namespace_is_not_the_widget(self):
        wrong = ET.fromstring('<Form><InputField name="Probe"/></Form>')
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, wrong, self.case)

    def test_mask_value_with_added_spaces_is_a_different_value(self):
        native = ET.fromstring('<Form xmlns="' + NS + '"><InputField name="Probe"><Mask>9999999999</Mask></InputField></Form>')
        case = dict(self.case, property='{' + NS + '}Mask',
                    native=json.loads(json.dumps(canonical(native[0][0]))))
        ours = ET.fromstring('<Form xmlns="' + NS + '"><InputField name="Probe"><Mask> 9999999999 </Mask></InputField></Form>')
        with self.assertRaisesRegex(ValueError, 'unrelated mismatch'):
            classify(native, ours, case)

    def test_indentation_between_children_does_not_change_the_condition(self):
        native = ET.fromstring('<Form xmlns="' + NS + '"><InputField name="Probe"><UserVisible>\n  <Common>false</Common>\n</UserVisible></InputField></Form>')
        case = dict(self.case, property='{' + NS + '}UserVisible',
                    native=json.loads(json.dumps(canonical(native[0][0]))))
        ours = ET.fromstring('<Form xmlns="' + NS + '"><InputField name="Probe"><UserVisible><Common>false</Common></UserVisible></InputField></Form>')
        self.assertEqual(classify(native, ours, case), 'equal')

    def test_repeated_property_is_not_accepted(self):
        wrong = form('2')
        wrong[0].append(ET.fromstring('<Height xmlns="' + NS + '">2</Height>'))
        with self.assertRaisesRegex(ValueError, 'repeated property'):
            classify(self.native, wrong, self.case)


CORE = 'http://v8.1c.ru/8.1/data-composition-system/core'
XSI_TYPE = '{http://www.w3.org/2001/XMLSchema-instance}type'


def parameter(name, item_type='dcsset:SettingsParameterValue', settings_namespace='http://v8.1c.ru/8.1/data-composition-system/settings'):
    return parse_xml(('<Settings xmlns:c="' + CORE
                         + '" xmlns:dcsset="' + settings_namespace
                         + '" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
                         + '<c:item xsi:type="' + item_type + '"><c:parameter>'
                         + name + '</c:parameter></c:item></Settings>').encode('utf-8'))


class ParameterWitnessContract(unittest.TestCase):
    def setUp(self):
        self.native = parameter('Font')
        self.case = {'id': 'font-name',
                     'anchor': {'tag': '{' + CORE + '}item',
                                'attributes': {XSI_TYPE: '{http://v8.1c.ru/8.1/data-composition-system/settings}SettingsParameterValue'},
                                'child': {'tag': '{' + CORE + '}parameter',
                                          'texts': ['Font', 'Шрифт']}},
                     'property': '{' + CORE + '}parameter',
                     'native': canonical(self.native[0][0]),
                     'known_symptom': canonical(parameter('Шрифт')[0][0])}

    def test_known_parameter_name_change_keeps_the_item(self):
        self.assertEqual(classify(self.native, parameter('Шрифт'), self.case), 'known-symptom')

    def test_corrected_parameter_name_is_equal(self):
        self.assertEqual(classify(self.native, parameter('Font'), self.case), 'equal')

    def test_missing_parameter_item_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, ET.fromstring('<Settings/>'), self.case)

    def test_wrong_parameter_value_is_rejected(self):
        with self.assertRaises(ValueError):
            classify(self.native, parameter('Unexpected'), self.case)

    def test_wrong_item_type_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, parameter('Шрифт', 'dcsset:OtherValue'), self.case)

    def test_rebound_type_namespace_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, parameter('Шрифт', settings_namespace='urn:wrong-settings-type'), self.case)

    def test_equivalent_type_prefix_preserves_identity(self):
        xml = '<Settings xmlns:c="' + CORE + '" xmlns:t="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><c:item xsi:type="t:SettingsParameterValue"><c:parameter>Шрифт</c:parameter></c:item></Settings>'
        self.assertEqual(classify(self.native, parse_xml(xml.encode()), self.case), 'known-symptom')

    def test_undeclared_type_prefix_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'undeclared xsi:type'):
            parameter('Шрифт', item_type='missing:SettingsParameterValue')

    def test_nested_namespace_rebinding_is_rejected(self):
        xml = '<Settings xmlns:c="' + CORE + '" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><c:item xmlns:dcsset="urn:other" xsi:type="dcsset:SettingsParameterValue"><c:parameter>Шрифт</c:parameter></c:item></Settings>'
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, parse_xml(xml.encode()), self.case)

    def test_expanded_name_cannot_be_spoofed_as_lexical_type(self):
        with self.assertRaisesRegex(ValueError, 'invalid xsi:type'):
            parameter('Шрифт', item_type='{http://v8.1c.ru/8.1/data-composition-system/settings}SettingsParameterValue')

    def test_unprefixed_type_uses_the_default_namespace(self):
        root = parse_xml(b'<item xmlns="urn:type" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="Value"/>')
        self.assertEqual(root.get(XSI_TYPE), '{urn:type}Value')

    def test_repeated_parameter_item_is_rejected(self):
        ours = parameter('Шрифт')
        ours.append(parameter('Font')[0])
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, ours, self.case)

    def test_repeated_key_inside_one_item_is_rejected(self):
        ours = parameter('Шрифт')
        ours[0].append(parameter('Font')[0][0])
        with self.assertRaisesRegex(ValueError, 'repeated property'):
            classify(self.native, ours, self.case)

    def test_foreign_namespace_does_not_supply_the_key(self):
        ours = parameter('Шрифт')
        ours[0][0].tag = '{urn:other}parameter'
        with self.assertRaisesRegex(ValueError, 'intact widget'):
            classify(self.native, ours, self.case)

    def test_aliases_cannot_accept_another_name(self):
        case = json.loads(json.dumps(self.case))
        case['anchor']['child']['texts'].append('Unexpected')
        with self.assertRaisesRegex(ValueError, 'child identity'):
            classify(self.native, parameter('Unexpected'), case)




def spreadsheet(font=False):
    return ET.fromstring('<document xmlns="urn:coverage:spreadsheet">'
                         + '<rowsItem><index>0</index></rowsItem>'
                         + ('<font faceName="Arial" height="10"/>' if font else '')
                         + '<format><font>0</font></format></document>')


class DocumentWitnessContract(unittest.TestCase):
    def setUp(self):
        self.native = spreadsheet(True)
        self.case = {'id': 'legacy-font',
                     'anchor': {'tag': self.native.tag, 'root': True,
                                'context': canonical(spreadsheet())},
                     'property': '{urn:coverage:spreadsheet}font',
                     'native': json.loads(json.dumps(canonical(self.native[1]))), 'known_symptom': None}

    def test_only_missing_font_is_the_known_symptom(self):
        self.assertEqual(classify(self.native, spreadsheet(), self.case), 'known-symptom')

    def test_restored_font_is_equal(self):
        self.assertEqual(classify(self.native, spreadsheet(True), self.case), 'equal')

    def test_empty_document_does_not_substitute_for_font_loss(self):
        with self.assertRaises(ValueError):
            classify(self.native, ET.fromstring('<document xmlns="urn:coverage:spreadsheet"/>'), self.case)

    def test_changed_format_reference_is_rejected(self):
        ours = spreadsheet()
        ours[-1][0].text = '1'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_duplicate_context_is_rejected(self):
        ours = spreadsheet()
        ours.append(ET.fromstring('<format xmlns="urn:coverage:spreadsheet"><font>0</font></format>'))
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_nested_document_is_not_the_root(self):
        ours = ET.fromstring('<wrapper/>')
        ours.append(spreadsheet())
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_duplicate_font_is_rejected(self):
        ours = spreadsheet(True)
        ours.append(ET.fromstring('<font xmlns="urn:coverage:spreadsheet" faceName="Arial" height="10"/>'))
        with self.assertRaisesRegex(ValueError, 'repeated property'):
            classify(self.native, ours, self.case)

    def test_changed_font_is_an_unrelated_mismatch(self):
        ours = spreadsheet(True)
        ours[1].set('faceName', 'Tahoma')
        with self.assertRaisesRegex(ValueError, 'unrelated mismatch'):
            classify(self.native, ours, self.case)

    def test_meaningful_tail_after_format_is_rejected(self):
        ours = spreadsheet()
        ours[-1].tail = 'unexpected-document-content'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_meaningful_tail_inside_context_is_rejected(self):
        ours = spreadsheet()
        ours[-1][0].tail = 'unexpected-format-content'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_non_xml_space_in_context_text_is_rejected(self):
        ours = spreadsheet()
        ours.text = '\u00a0'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_non_xml_space_in_tail_is_rejected(self):
        ours = spreadsheet()
        ours[-1].tail = '\u00a0'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_indentation_tails_remain_acceptable(self):
        ours = spreadsheet()
        for node in ours.iter():
            node.tail = '\n\t'
        self.assertEqual(classify(self.native, ours, self.case), 'known-symptom')

    def test_wrong_document_namespace_is_rejected(self):
        with self.assertRaises(ValueError):
            classify(self.native, ET.fromstring('<document/>'), self.case)


def merge_document(order):
    return ET.fromstring('<document xmlns="urn:coverage:spreadsheet">'
                         + '<height>4</height>'
                         + ''.join('<merge><c>' + str(index) + '</c></merge>' for index in order)
                         + '<format><font>0</font></format></document>')


class CollectionWitnessContract(unittest.TestCase):
    def setUp(self):
        self.native = merge_document([0, 3])
        self.case = {'id': 'merge-order', 'collection': True,
                     'anchor': {'tag': self.native.tag, 'root': True,
                                'context': canonical(merge_document([]))},
                     'property': '{urn:coverage:spreadsheet}merge',
                     'native': [canonical(x) for x in self.native if x.tag.endswith('}merge')],
                     'known_symptom': [canonical(x) for x in merge_document([3, 0]) if x.tag.endswith('}merge')]}

    def test_specific_wrong_order_is_known(self):
        self.assertEqual(classify(self.native, merge_document([3, 0]), self.case), 'known-symptom')

    def test_restored_collection_is_equal(self):
        self.assertEqual(classify(self.native, merge_document([0, 3]), self.case), 'equal')

    def test_lost_collection_is_an_unrelated_mismatch(self):
        with self.assertRaises(ValueError):
            classify(self.native, merge_document([]), self.case)

    def test_lost_member_is_an_unrelated_mismatch(self):
        with self.assertRaises(ValueError):
            classify(self.native, merge_document([0]), self.case)

    def test_other_duplicate_is_an_unrelated_mismatch(self):
        with self.assertRaises(ValueError):
            classify(self.native, merge_document([3, 3]), self.case)

    def test_changed_coordinate_is_an_unrelated_mismatch(self):
        with self.assertRaises(ValueError):
            classify(self.native, merge_document([4, 0]), self.case)

    def test_changed_document_context_is_rejected(self):
        ours = merge_document([3, 0])
        ours[0].text = '5'
        with self.assertRaises(ValueError):
            classify(self.native, ours, self.case)

    def test_collection_mode_requires_a_root_context(self):
        case = dict(self.case, anchor={'tag': self.native.tag, 'name': 'Probe'})
        with self.assertRaises(ValueError):
            classify(self.native, merge_document([3, 0]), case)


if __name__ == '__main__':
    unittest.main()
