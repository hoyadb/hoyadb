#!/usr/bin/env python3
"""SPEC-0034 / TC-0038 SR-31 permanent embedded/shaded component gate.

inventory(): byte-bound identities from embedded META-INF/maven pom.properties, plus relocated
namespaces attributed through an explicit namespace map and caller-supplied source-declared versions
(each with a distinct confidence label). evaluate(): every component needs an advisory result and a
triage entry; unknown versions, unmapped relocated namespaces, unavailable scans, untriaged advisories
and dispositions without evidence reject. No network: advisory results are inputs.
Exit codes: 0 closed, 1 open findings, 2 rejected.
"""
import argparse
import json
from pathlib import Path
import sys
import zipfile

THIRD_PARTY_ROOTS = {'com', 'org', 'it', 'io', 'net', 'one', 'javax', 'gnu', 'kotlinx'}
DISPOSITIONS = {'fixed', 'not_applicable', 'open'}


def _ga(coordinate):
    return ':'.join(coordinate.split(':')[:2])


def inventory(jar, own_coordinate, relocation_prefixes, namespace_map, source_declared):
    jar = Path(jar)
    own = _ga(own_coordinate)
    with zipfile.ZipFile(jar) as archive:
        names = archive.namelist()
        found, by_ga = [], {}
        for name in sorted(names):
            if name.startswith('META-INF/maven/') and name.endswith('/pom.properties'):
                props = {}
                for line in archive.read(name).decode('utf-8', 'replace').splitlines():
                    if '=' in line and not line.lstrip().startswith('#'):
                        key, _, value = line.partition('=')
                        props[key.strip()] = value.strip()
                ga = '%s:%s' % (props.get('groupId', ''), props.get('artifactId', ''))
                if ga == own:
                    continue
                version = props.get('version') or None
                item = {'ga': ga, 'version': version, 'coordinate': '%s:%s' % (ga, version or '?'),
                        'confidence': 'byte_bound_pom_properties' if version else 'unversioned_pom_properties',
                        'container': jar.name, 'evidence': name, 'namespaces': []}
                found.append(item)
                by_ga[ga] = item
    mapped = sorted(namespace_map.items(), key=lambda kv: -len(kv[0]))
    prefixes = sorted(set(relocation_prefixes), key=len, reverse=True)  # longest prefix wins; '' = unrelocated
    namespaces = {}
    for name in names:
        if not name.endswith('.class'):
            continue
        for prefix in prefixes:
            if not name.startswith(prefix):
                continue
            hit = next((ns for ns, _ in mapped if name.startswith(ns)), None)
            if hit:
                namespaces.setdefault(('mapped', hit), 0)
                namespaces[('mapped', hit)] += 1
            else:
                package = name[len(prefix):].split('/')[:-1]
                if package and package[0] in THIRD_PARTY_ROOTS:
                    key = prefix + '/'.join(package[:3]) + '/'
                    namespaces[('unmapped', key)] = namespaces.get(('unmapped', key), 0) + 1
            break
    for (kind, ns), count in sorted(namespaces.items()):
        if kind == 'unmapped':
            found.append({'ga': None, 'version': None, 'coordinate': None, 'confidence': 'unmapped_namespace',
                          'container': jar.name, 'evidence': ns, 'namespaces': [ns], 'classes': count})
            continue
        ga = dict(mapped)[ns]
        if ga in by_ga:
            by_ga[ga]['namespaces'].append(ns)
            continue
        declared = source_declared.get(ga) or {}
        version = declared.get('version')
        item = {'ga': ga, 'version': version, 'coordinate': '%s:%s' % (ga, version or '?'),
                'confidence': 'source_declared' if version else 'relocated_namespace_unversioned',
                'container': jar.name, 'evidence': declared.get('source') or ns, 'namespaces': [ns], 'classes': count}
        found.append(item)
        by_ga[ga] = item
    return found


def evaluate(components, advisories, triage):
    rejections, findings = [], []

    def reject(component, reason):
        rejections.append({'container': component.get('container'), 'component': component.get('coordinate') or component.get('evidence'),
                           'reason': reason})
    for component in components:
        if component['confidence'] == 'unreadable_container':
            reject(component, 'unreadable container')
            continue
        if component['confidence'] == 'unmapped_namespace':
            reject(component, 'unmapped relocated namespace')
            continue
        if not component.get('version'):
            reject(component, 'unknown version')
            continue
        coordinate = component['coordinate']
        results = advisories.get(coordinate)
        if results is None:
            reject(component, 'advisory scan unavailable')
            continue
        entry = triage.get(coordinate)
        if entry is None:
            reject(component, 'untriaged component')
            entry = {}
        for advisory in results:
            decision = entry.get(advisory)
            if decision is None:
                reject(component, 'untriaged advisory ' + advisory)
                continue
            disposition = decision.get('disposition')
            if disposition not in DISPOSITIONS:
                reject(component, 'invalid disposition for ' + advisory)
            elif disposition in ('fixed', 'not_applicable') and not decision.get('evidence'):
                reject(component, 'disposition without evidence for ' + advisory)
            elif disposition == 'open':
                findings.append({'coordinate': coordinate, 'advisory': advisory, 'container': component['container'],
                                 'confidence': component['confidence'], 'evidence': decision.get('evidence')})
    status = 'rejected' if rejections else 'open' if findings else 'closed'
    return {'status': status, 'rejections': rejections, 'findings': findings}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args(argv)
    plan = json.loads(args.plan.read_text())
    components = []
    for item in plan['jars']:
        try:
            components += inventory(item['path'], item['own_coordinate'], plan.get('relocation_prefixes', []),
                                    plan.get('namespace_map', {}), plan.get('source_declared', {}))
        except (zipfile.BadZipFile, OSError):
            components.append({'ga': None, 'version': None, 'coordinate': None, 'confidence': 'unreadable_container',
                               'container': Path(item['path']).name, 'evidence': str(item['path']), 'namespaces': []})
    result = evaluate(components, plan.get('advisories', {}), plan.get('triage', {}))
    report = dict(result, components=components)
    args.out.write_text(json.dumps(report, indent=1, sort_keys=True) + '\n')
    return {'closed': 0, 'open': 1, 'rejected': 2}[result['status']]


if __name__ == '__main__':
    sys.exit(main())
