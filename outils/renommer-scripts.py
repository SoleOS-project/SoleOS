#!/usr/bin/env python3
"""SoleOS : dans Fichiers (Nautilus), le sous-menu « Scripts » du clic droit s'appelle « Raccourcis »
(il contient « Ajouter au bureau »). On modifie la traduction de Nautilus pour les 6 langues ;
pour l'anglais (langue d'origine, sans traduction), on crée une petite traduction dédiée.
Usage : renommer-scripts.py [racine des locales, par défaut /usr/share/locale]"""
import os
import struct
import sys

RACINE = sys.argv[1] if len(sys.argv) > 1 else "/usr/share/locale"
NOMS = {"fr": "Raccourcis", "en": "Shortcuts", "it": "Collegamenti", "es": "Accesos directos",
        "de": "Verknüpfungen", "zh_CN": "快捷方式"}
CIBLES = ("_Scripts", "Scripts")


def lire(chemin):
    d = open(chemin, "rb").read()
    magie = struct.unpack("<I", d[:4])[0]
    e = "<" if magie == 0x950412DE else ">"
    _, n, o_orig, o_trad = struct.unpack(e + "4I", d[4:20])
    paires = []
    for i in range(n):
        lo, po = struct.unpack(e + "2I", d[o_orig + 8 * i:o_orig + 8 * i + 8])
        lt, pt = struct.unpack(e + "2I", d[o_trad + 8 * i:o_trad + 8 * i + 8])
        paires.append((d[po:po + lo], d[pt:pt + lt]))
    return paires


def ecrire(chemin, paires):
    paires = sorted(paires, key=lambda p: p[0])
    n = len(paires)
    o_orig = 28
    o_trad = o_orig + 8 * n
    pos = o_trad + 8 * n
    t_orig, t_trad, donnees = b"", b"", b""
    for cle, _ in paires:
        t_orig += struct.pack("<2I", len(cle), pos + len(donnees))
        donnees += cle + b"\0"
    for _, val in paires:
        t_trad += struct.pack("<2I", len(val), pos + len(donnees))
        donnees += val + b"\0"
    entete = struct.pack("<7I", 0x950412DE, 0, n, o_orig, o_trad, 0, 0)
    open(chemin + ".tmp", "wb").write(entete + t_orig + t_trad + donnees)
    os.replace(chemin + ".tmp", chemin)


for langue, nom in NOMS.items():
    mo = os.path.join(RACINE, langue, "LC_MESSAGES", "nautilus.mo")
    if os.path.exists(mo):
        paires = lire(mo)
    elif langue == "en":
        os.makedirs(os.path.dirname(mo), exist_ok=True)
        paires = [(b"", b"Content-Type: text/plain; charset=UTF-8\n")]
    else:
        print(f"{langue} : pas de traduction de Nautilus, ignoré")
        continue
    changes = 0
    nouvelles = []
    trouvees = set()
    for cle, val in paires:
        base = cle.split(b"\x04")[-1].decode("utf-8", "replace")
        if base in CIBLES:
            val = ("_" + nom if base.startswith("_") else nom).encode()
            changes += 1
            trouvees.add(base)
        nouvelles.append((cle, val))
    for c in CIBLES:
        if c not in trouvees:
            nouvelles.append((c.encode(), ("_" + nom if c.startswith("_") else nom).encode()))
            changes += 1
    ecrire(mo, nouvelles)
    print(f"{langue} : {changes} texte(s) → {nom}")
