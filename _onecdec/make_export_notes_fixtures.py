"""Derive the export-report fixtures from platform-built ones: containers the
export can read only in part, to pin what the report says about the rest. No
platform run -- these are damaged on purpose.

    python make_export_notes_fixtures.py

Output: tests/fixtures/external/export_notes/
  metadata_miss.cf  -- home_page/one_column with ФормаСправа's metadata row
                       unreadable (its name unquoted);
  wrong_kind.cf     -- the same row under an unknown inner version code;
  withheld_form.cf  -- choice_list_dates with a 13-digit date in the choice
                       list, which makes the list opaque and withholds Form.xml.
"""
import os, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8c

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.join(os.path.dirname(HERE), 'tests', 'fixtures', 'external')


def derive(source, name, old, new, out):
    raw = open(os.path.join(FIXTURES, source), 'rb').read()
    storage_version = struct.unpack_from('<I', raw, 8)[0]
    entries = v8c.read(os.path.join(FIXTURES, source))
    text = entries[name].decode('utf-8-sig')
    assert text.count(old) >= 1, (source, name, old)
    entries[name] = ('﻿' + text.replace(old, new, 1)).encode('utf-8')
    dst = os.path.join(FIXTURES, 'export_notes')
    os.makedirs(dst, exist_ok=True)
    open(os.path.join(dst, out), 'wb').write(v8c.write15(entries, storage_version))
    print('ok', out)


def main():
    # The name unquoted: the header does not read. (An unknown version code
    # inside the properties no longer serves: the root names the row a common
    # form, and that writer reads it.)
    derive('home_page/one_column/input.cf', '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12',
           '"ФормаСправа"', 'ФормаСправа', 'metadata_miss.cf')
    # An unknown version code inside ФормаСправа's properties: guessed from
    # the code, the row was a common picture and its form body went to
    # CommonPictures/…/Picture.xml; the root names it a common form.
    derive('home_page/one_column/input.cf', '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12',
           '{13,', '{99,', 'wrong_kind.cf')
    derive('choice_list_dates/input.cf', '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21.0',
           '{"D",00010101000000}', '{"D",0001010100000}', 'withheld_form.cf')
    # search_form's DefaultSearchForm (root field 37, the first occurrence of
    # the form's uuid in the root row) naming no object of the container.
    derive('search_form/input.cf', 'ba46775a-8ecc-49a2-8dc9-f4173b708a73',
           '5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a01', 'ffffffff-0000-4000-8000-0000000000aa',
           'unresolved_search_form.cf')


if __name__ == '__main__':
    main()
