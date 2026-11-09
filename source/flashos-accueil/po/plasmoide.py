#!/usr/bin/env python3
"""Traductions de la barre magique (bureau Plasma) : locale/<langue>/LC_MESSAGES/
plasma_applet_fr.flashos.barremagique.mo, à partir des mêmes traductions que FlashOS.
(Plasma veut des textes source en anglais : l'anglais n'a pas de traduction, le français en a une.)"""
import json, os, subprocess, tempfile
ici = os.path.dirname(os.path.abspath(__file__))
LANGUES = [('fr', 'fr'), ('it', 'it'), ('es', 'es'), ('de', 'de'), ('zh', 'zh_CN')]
COLONNE = {'fr': None, 'it': 1, 'es': 2, 'de': 3, 'zh': 4}
TEXTES = ["Bureautique", "Mail", "Internet", "Fichiers", "Applications", "Réglages", "Texte", "Tableur", "Présentation"]
base = json.load(open(os.path.join(ici, 'trad', 'base.json'), encoding='utf-8'))
for l in open(os.path.join(ici, 'trad', 'extras.tsv'), encoding='utf-8'):
    c = l.rstrip('\n').split('\t')
    if len(c) == 6: base[c[0]] = c[1:]
for code, dossier in LANGUES:
    po = ['msgid ""', 'msgstr ""', '"Content-Type: text/plain; charset=UTF-8\\n"', '']
    for t in TEXTES:
        anglais = base[t][0]
        po += [f'msgid "{anglais}"', f'msgstr "{t if COLONNE[code] is None else base[t][COLONNE[code]]}"', '']
    sortie = os.path.join(ici, '..', 'locale', dossier, 'LC_MESSAGES', 'plasma_applet_fr.flashos.barremagique.mo')
    os.makedirs(os.path.dirname(sortie), exist_ok=True)
    with tempfile.NamedTemporaryFile('w', suffix='.po', delete=False, encoding='utf-8') as f:
        f.write('\n'.join(po))
    subprocess.run(['msgfmt', '-o', sortie, f.name], check=True)
    os.unlink(f.name)
print(f"barre magique : {len(TEXTES)} textes, {len(LANGUES)} langues")
