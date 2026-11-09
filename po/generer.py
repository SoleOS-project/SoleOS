#!/usr/bin/env python3
"""Fabrique po/<langue>.po à partir de flashos.pot et des traductions (po/trad/*.tsv),
puis les fichiers compilés locale/<langue>/LC_MESSAGES/flashos.mo (msgfmt)."""
import re, json, os, glob, subprocess, sys
ici = os.path.dirname(os.path.abspath(__file__))
LANGUES = [('en', 'en'), ('it', 'it'), ('es', 'es'), ('de', 'de'), ('zh', 'zh_CN')]
# Traductions : po/trad/base.json (texte français → 5 traductions), complété par extras.tsv
base = json.load(open(os.path.join(ici, 'trad', 'base.json'), encoding='utf-8'))
for l in open(os.path.join(ici, 'trad', 'extras.tsv'), encoding='utf-8'):
    c = l.rstrip('\n').split('\t')
    if len(c) == 6: base[c[0]] = c[1:]
pot = open(os.path.join(ici, 'flashos.pot'), encoding='utf-8').read()
ids = re.findall(r'^msgid "((?:[^"\\]|\\.)*)"$', pot, re.M)[1:]
manque = [t for t in ids if t not in base]
erreurs = []
for t in ids:
    if t in base:
        for (code, _), tr in zip(LANGUES, base[t]):
            if sorted(re.findall(r'\{[a-z_]+\}', t)) != sorted(re.findall(r'\{[a-z_]+\}', tr)):
                erreurs.append(f'{code}: valeurs différentes : {t!r} → {tr!r}')
            if '"' in tr.replace('\\"', ''):
                erreurs.append(f'{code}: guillemet droit non échappé : {tr!r}')
for e in erreurs: print('ERREUR', e)
for m in manque: print('MANQUE', m)
sortie = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ici, '..', 'locale')
for i, (code, dossier) in enumerate(LANGUES):
    p = os.path.join(ici, f'{dossier}.po')
    with open(p, 'w', encoding='utf-8') as o:
        o.write(f'# FlashOS — traduction ({dossier})\n# © 2026 GDH (Giuseppe D\'Arco Holding) — LicenseRef-SoleOS\n'
                f'msgid ""\nmsgstr ""\n"Project-Id-Version: FlashOS\\n"\n"Language: {dossier}\\n"\n'
                '"MIME-Version: 1.0\\n"\n"Content-Type: text/plain; charset=UTF-8\\n"\n"Content-Transfer-Encoding: 8bit\\n"\n\n')
        for t in ids:
            if t in base:
                o.write(f'msgid "{t}"\nmsgstr "{base[t][i]}"\n\n')
    d = os.path.join(sortie, dossier, 'LC_MESSAGES'); os.makedirs(d, exist_ok=True)
    subprocess.run(['msgfmt', '--check-format', '-o', os.path.join(d, 'flashos.mo'), p], check=True)
print(len(ids), 'textes,', len(manque), 'manquants,', len(erreurs), 'erreurs')
# Écran verrouillé (gtklock) : po/gtklock/<langue>.po
for p in sorted(glob.glob(os.path.join(ici, 'gtklock', '*.po'))):
    l = os.path.basename(p)[:-3]
    d = os.path.join(sortie, l, 'LC_MESSAGES'); os.makedirs(d, exist_ok=True)
    subprocess.run(['msgfmt', '-o', os.path.join(d, 'gtklock.mo'), p], check=True)
