#!/usr/bin/env python3
"""Extract pinned TrustedQSL configuration into deterministic runtime data."""
from pathlib import Path
from decimal import Decimal
import json
import re
import sys
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent.parent
source = root / 'references/trustedqsl/upstream/src/config.xml'
target = root / 'lib/clacks-model/data/lotw-11.34.json'
xml = ET.parse(source).getroot()


def zones(text):
    return [{'itu': int(itu), 'cq': int(cq)}
            for item in (text or '').split(',') if item
            for itu, cq in [item.split(':')]]


mapping = {}
for row in xml.findall('./adifmap/adifmode'):
    label = (row.text or '').strip()
    parent = row.get('adif-mode', label)
    sub = row.get('adif-submode', '')
    mode = row.get('mode', '')
    if mode:
        for key in [parent, sub, label, f'{parent}%{sub}' if parent and sub else '']:
            if key:
                mapping[key.upper()] = mode.upper()

location_source = (root / 'references/trustedqsl/upstream/src/location.cpp').read_text()
primary_table = re.search(r'sasMapping\[\]\s*=\s*\{(.*?)\n\};', location_source, re.S)
if primary_table is None:
    raise SystemExit('Pinned primary-subdivision table missing')
primary_fields = re.findall(r'\{\s*"([A-Z_]+)"', primary_table.group(1))
fields = {}
for field in xml.findall('./locfields/field'):
    fields[field.get('Id')] = {
        'length': int(field.get('len', '0')),
        'flags': int(field.get('flags', '0')),
        'depends_on': field.get('dependsOn'),
        'values': [{'dependency': enums.get('dependency'),
                    'value': e.get('value', ''), 'label': (e.text or '').strip(),
                    'zones': zones(e.get('zonemap'))}
                   for enums in field.findall('enums') for e in enums.findall('enum')]
    }
entity_fields = {}
for page in xml.findall('./locpages/page'):
    if page.get('dependsOn') == 'DXCC':
        entity_fields.setdefault(page.get('dependency'), []).extend(
            (e.text or '').strip() for e in page.findall('pageField')
            if (e.text or '').strip() in fields)

data = {
    'release': '11.34',
    'upstream_commit': '6ce824363e27da455c6cf6fd440b9c401945bfcb',
    'mapping': mapping,
    'modes': {e.text.strip(): e.get('group') for e in xml.findall('./modes/mode')},
    'bands': [{'label': e.text.strip(),
               'lower_hz': int(Decimal(e.get('low')) * 1000),
               'upper_hz': int(Decimal(e.get('high')) * 1000)}
              for e in xml.findall('./bands/band')],
    'fields': fields,
    'primary_fields': primary_fields,
    # The native geographic secondary bindings selected by the service contract.
    'secondary_fields': ['US_COUNTY', 'JA_CITY_GUN_KU'],
    'entity_zones': {e.get('arrlId'): zones(e.get('zonemap')) for e in xml.findall('./dxcc/entity')},
    'entity_fields': entity_fields,
    'station_fields': [e.tag for e in xml.findall('./sigspecs/sigspec')[0].findall('./tSTATION/*')]
}
encoded = json.dumps(data, indent=2, ensure_ascii=False, sort_keys=True) + '\n'
if '--check' in sys.argv:
    if not target.exists() or target.read_text() != encoded:
        raise SystemExit('LoTW derived data differs from pinned source')
    print('LoTW data matches pinned source')
else:
    target.write_text(encoded)
