#!/usr/bin/env python3
"""Extrait les textes à traduire vers po/soleos.pot :
- programmes Rust : tr("…"), trv("…"), n_("…"), selon("…", "…") et les petites fonctions
  d'affichage qui traduisent elles-mêmes leur texte (etiquette, petit, bouton…) ;
- scripts shell : t "…" (fonction de traduction des scripts SoleOS)."""
import re, glob, os, sys
ici = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ici)
from lexique import chaines

from lexique import MOTS, appel, extrait

def nettoyer(t):
    # continuation de ligne Rust : « \ » + retour à la ligne + espaces = rien
    return re.sub(r'\\\n\s*', '', t)

textes = {}
def noter(t, lieu):
    if t.strip(): textes.setdefault(t, []).append(lieu)

for f in sorted(glob.glob(os.path.join(ici, '..', 'src', '**', '*.rs'), recursive=True)):
    s = open(f, encoding='utf-8').read()
    k = s.find('#[cfg(test)]')
    if k > 0: s = s[:k]
    rel = os.path.relpath(f, os.path.join(ici, '..'))
    for a, b, t in chaines(s):
        if extrait(s, a):
            noter(nettoyer(t), f"{rel}:{s.count(chr(10), 0, a) + 1}")

for f in sys.argv[1:]:
    s = open(f, encoding='utf-8').read()
    for m in re.finditer(r'''(?:^|[\s;(`$])t\s+"((?:[^"\\]|\\.)*)"''', s, re.M):
        debut_ligne = s.rfind('\n', 0, m.start()) + 1
        if s[debut_ligne:m.start()].lstrip().startswith('#'):
            continue
        noter(m.group(1), f"{os.path.basename(f)}:{s.count(chr(10), 0, m.start()) + 1}")

with open(os.path.join(ici, 'soleos.pot'), 'w', encoding='utf-8') as o:
    o.write('# SoleOS — textes de l\'interface (langue d\'origine : français)\n'
            '# © 2026 GDH (Giuseppe D\'Arco Holding) — LicenseRef-SoleOS\n'
            'msgid ""\nmsgstr ""\n"Project-Id-Version: SoleOS\\n"\n"Content-Type: text/plain; charset=UTF-8\\n"\n'
            '"Content-Transfer-Encoding: 8bit\\n"\n"Language: fr\\n"\n\n')
    for t, lieux in textes.items():
        o.write('#: ' + ' '.join(lieux[:4]) + '\n')
        if '{' in t:
            o.write('#, soleos-format\n')
        o.write(f'msgid "{t}"\nmsgstr ""\n\n')
print(len(textes), 'textes')
