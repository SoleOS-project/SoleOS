#!/usr/bin/env python3
# © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS
"""SoleOS – test des haut-parleurs (puce son HDA, par ex. Conexant SN6180 des Chuwi).

Lance des essais un par un : allumer un « interrupteur » (GPIO) de la puce son, ou la
sortie 0x1d, pendant qu'un bip est joué. On répond o/n : « j'entends le bip dans les
haut-parleurs ». Rien n'est écrit sur le disque de l'ordinateur ; tout est remis comme
avant à la fin (et au redémarrage).

Usage : sudo python3 test-son.py   (casque débranché, volume pas à zéro)
Sur une clé Linux « live » où sudo marche sans mot de passe (Ubuntu « Essayer », Debian live…).
"""
import array
import fcntl
import glob
import math
import os
import re
import struct
import subprocess
import sys
import tempfile
import time
import wave

IOCTL_VERB = 0xC0084811  # HDA_IOCTL_VERB_WRITE = _IOWR('H', 0x11, {u32 verb; u32 res;})


def trouver_puce():
    """La puce son analogique (pas l'HDMI) : (fichier /proc, périphérique /dev/snd/hwC*D*)."""
    for f in sorted(glob.glob("/proc/asound/card*/codec#*")):
        t = open(f, errors="replace").read()
        if re.search(r"Speaker at|HP Out at|Line Out at|Headphone", t) and "HDMI" not in t.split("\n")[0]:
            c = re.search(r"card(\d+)", f).group(1)
            d = f.rsplit("#", 1)[1]
            return f, t, f"/dev/snd/hwC{c}D{d}"
    return None, None, None


class Puce:
    def __init__(self, dev):
        self.fd = os.open(dev, os.O_RDWR)

    def verbe(self, nid, v):
        """v = verbe 20 bits déjà assemblé (ex. 0x70640, 0x20011, 0x3b000)."""
        buf = array.array("I", [((nid & 0xff) << 24) | (v & 0xfffff), 0])
        fcntl.ioctl(self.fd, IOCTL_VERB, buf, True)
        return buf[1]

    def lire(self, nid, verbe12, param=0):
        return self.verbe(nid, (verbe12 << 8) | param)

    def ecrire(self, nid, verbe12, param):
        return self.verbe(nid, (verbe12 << 8) | (param & 0xff))


def noeuds(texte):
    """{nid: bloc de texte} depuis /proc/asound/cardX/codec#Y"""
    blocs = {}
    for m in re.finditer(r"^Node (0x[0-9a-f]+) (.*?)(?=^Node |\Z)", texte, re.M | re.S):
        blocs[int(m.group(1), 16)] = m.group(0)
    return blocs


def bip(chemin):
    with wave.open(chemin, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(48000)
        n = 48000 * 3
        data = array.array("h")
        for i in range(n):
            a = int(9000 * math.sin(2 * math.pi * 660 * i / 48000)) if (i // 9600) % 2 == 0 else 0
            data.extend((a, a))
        w.writeframes(data.tobytes())


def comme_utilisateur(cmd):
    """Lancé avec sudo : le son de la session appartient à l'utilisateur, pas à root."""
    u = os.environ.get("SUDO_USER")
    if os.geteuid() == 0 and u and u != "root":
        import pwd
        uid = pwd.getpwnam(u).pw_uid
        return ["sudo", "-u", u, "env", f"XDG_RUNTIME_DIR=/run/user/{uid}",
                f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus"] + cmd
    return cmd


def jouer(chemin):
    os.chmod(chemin, 0o644)
    for cmd in (["pw-play", chemin], ["paplay", chemin], ["aplay", "-q", chemin]):
        cmd = comme_utilisateur(cmd)
        try:
            return subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except FileNotFoundError:
            continue
    sys.exit("Impossible de jouer un son (pw-play / aplay absents).")


def main():
    proc, texte, dev = trouver_puce()
    if not proc:
        sys.exit("Aucune puce son analogique trouvée.")
    nom = re.search(r"^Codec: (.*)$", texte, re.M).group(1)
    ident = re.search(r"^Subsystem Id: (.*)$", texte, re.M)
    ident = ident.group(1) if ident else "?"
    try:
        p = Puce(dev)
    except OSError as e:
        sys.exit(f"Accès refusé à {dev} ({e}).\nRelancez avec :  sudo python3 test-son.py")

    n_gpio = int(re.search(r"^GPIO: io=(\d+)", texte, re.M).group(1)) if re.search(r"^GPIO: io=(\d+)", texte, re.M) else 0
    blocs = noeuds(texte)
    sorties_eapd = [n for n, b in blocs.items() if "Pin Complex" in b and "EAPD" in b and " OUT" in b]
    dacs = [n for n, b in blocs.items() if "[Audio Output]" in b]

    print(f"Puce : {nom} ({ident}) – {dev}")
    print(f"Interrupteurs (GPIO) : {n_gpio} ; sorties avec ampli (EAPD) : {', '.join(hex(n) for n in sorties_eapd)}")
    print("\nDébranchez le casque. Le volume ne doit pas être à zéro.")
    for cmd in (["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "0.7"], ["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "0"]):
        try:
            subprocess.run(comme_utilisateur(cmd), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except FileNotFoundError:
            pass

    # État de départ (pour tout remettre à la fin)
    origine_pins = {n: (p.lire(n, 0xF07), p.lire(n, 0xF0C)) for n in sorties_eapd}

    def gpio(masque, direction, donnees):
        p.ecrire(0x01, 0x716, masque)
        p.ecrire(0x01, 0x717, direction)
        p.ecrire(0x01, 0x715, donnees)

    def remettre():
        gpio(0, 0, 0)
        for n, (ctl, eapd) in origine_pins.items():
            p.ecrire(n, 0x707, ctl)
            p.ecrire(n, 0x70C, eapd)

    def dac_actif():
        for d in dacs:
            if p.lire(d, 0xF06) & 0xF0:
                return d
        return None

    def ouvrir_sortie(n):
        """Comme sur le Chuwi CoreBook X : la sortie n reçoit le même son, ampli allumé."""
        b = blocs.get(n, "")
        conn = re.search(r"Connection: \d+\n\s+(.*)", b)
        if conn:
            liste = conn.group(1).split()
            choisi = next((c for c in liste if c.endswith("*")), liste[0] if liste else None)
            if choisi:
                d = int(choisi.rstrip("*"), 16)
                a = dac_actif()
                if a is not None and d != a:
                    flux = p.lire(a, 0xF06)
                    fmt = p.verbe(a, 0xA0000)
                    p.ecrire(d, 0x706, flux & 0xFF)             # même flux, mêmes voies
                    p.verbe(d, 0x20000 | (fmt & 0xFFFF))       # même format
                p.verbe(d, 0x3B000)                           # ampli du convertisseur : ouvert, 0 dB
        p.ecrire(n, 0x707, 0x40)                               # sortie activée
        p.ecrire(n, 0x70C, 0x02)                               # ampli externe (EAPD) allumé
        caps = re.search(r"Amp-Out caps: ofs=(0x[0-9a-f]+)", b)
        p.verbe(n, 0x3B000 | (int(caps.group(1), 16) if caps else 0))

    essais = [("Référence : rien de changé", lambda: None)]
    for k in range(n_gpio):
        essais.append((f"Interrupteur GPIO {k} allumé", lambda k=k: gpio(1 << k, 1 << k, 1 << k)))
    if n_gpio:
        tous = (1 << n_gpio) - 1
        essais.append(("Tous les interrupteurs allumés", lambda: gpio(tous, tous, tous)))
        essais.append(("Tous les interrupteurs éteints (commande inversée)", lambda: gpio(tous, tous, 0)))
    for n in sorties_eapd:
        essais.append((f"Sortie {hex(n)} activée, ampli allumé", lambda n=n: ouvrir_sortie(n)))
    if 0x1D in sorties_eapd and n_gpio:
        essais.append(("Sortie 0x1d + tous les interrupteurs", lambda: (ouvrir_sortie(0x1D), gpio(tous, tous, tous))))
    essais.append(("Toutes les sorties + tous les interrupteurs",
                   lambda: ([ouvrir_sortie(n) for n in sorties_eapd], n_gpio and gpio(tous, tous, tous))))

    son = os.path.join(tempfile.gettempdir(), "soleos-bip.wav")
    bip(son)
    resultats = []
    try:
        for i, (titre, action) in enumerate(essais):
            print(f"\nEssai {i}/{len(essais) - 1} : {titre}")
            while True:
                lecteur = jouer(son)
                time.sleep(0.4)          # la puce est réveillée par le son
                try:
                    action()
                except OSError as e:
                    print(f"  (réglage refusé : {e})")
                lecteur.wait()
                r = input("  Bips entendus dans les haut-parleurs ? [o]ui / [n]on / [r]ejouer : ").strip().lower()
                if r.startswith("r"):
                    continue
                break
            resultats.append((i, titre, "OUI" if r.startswith("o") else "non"))
            remettre()
    except KeyboardInterrupt:
        print("\nArrêté.")
    finally:
        remettre()

    rapport = [f"SoleOS – test du son – {time.strftime('%Y-%m-%d %H:%M')}",
               f"Puce : {nom} ({ident})", f"GPIO : {n_gpio} ; sorties EAPD : {[hex(n) for n in sorties_eapd]}", ""]
    rapport += [f"Essai {i:2d} : {res:3s}  {t}" for i, t, res in resultats]
    texte_rapport = "\n".join(rapport) + "\n"
    u = os.environ.get("SUDO_USER")
    maison = os.path.expanduser(f"~{u}") if os.geteuid() == 0 and u else os.path.expanduser("~")
    chemin = os.path.join(maison, "test-son-resultat.txt")
    open(chemin, "w").write(texte_rapport)
    os.chmod(chemin, 0o666)
    print("\n" + texte_rapport)
    print(f"Résultat enregistré dans {chemin}. Envoyez une photo de cet écran.")


if __name__ == "__main__":
    main()
