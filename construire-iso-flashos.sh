#!/bin/bash
# =====================================================================
#  SoleOS – construction de la clé « essayer puis installer » (ISO)
#
#  À lancer en root sur une Debian 13 reliée à Internet (votre VM) :
#      bash construire-iso-flashos.sh
#  À placer dans le même dossier : flashos-accueil, flashos-applications, flashos-installateur, flashos-bluetooth,
#  flashos-installer, flashos-systeme, flashos-securite, et les dossiers « fonds », « craft », « polices »
#  et « locale » (traductions : anglais, italien, espagnol, allemand, chinois).
#
#  SoleOS – créé par Giuseppe D'Arco – © 2026 GDH
#  Licence non commerciale SoleOS (PolyForm Noncommercial 1.0.0 + autorisations, voir LICENSE)
#
#  Résultat : SoleOS-<version>.iso dans le dossier personnel, à graver avec Rufus
#  (ou à tester dans une nouvelle machine VirtualBox).
#
#  Données chiffrées (code à 6 chiffres avec la puce TPM, sinon mot de passe),
#  clé de secours, écran verrouillé, pare-feu.
#  Bureautique : LibreOffice (logiciel libre, redistribuable).
# =====================================================================
set -euo pipefail
export PATH="/usr/local/sbin:/usr/sbin:/sbin:$PATH"

VERSION_FLASHOS="v1.0.3"   # affichée dans SoleOS (menu ⏻) et dans le nom de l'ISO
ESPACE_NECESSAIRE=15   # en Go
DOSSIER_SCRIPT="$(cd "$(dirname "$0")" && pwd)"
UTILISATEUR="$(id -nu 1000 2>/dev/null || echo root)"
MAISON="$(getent passwd "$UTILISATEUR" | cut -d: -f6)"
ATELIER=/srv/flashos-iso

if [ "$(id -u)" -ne 0 ]; then
  echo "Ce script doit être lancé en root (commande : su -  puis relancer)."; exit 1
fi
for f in flashos-accueil flashos-applications flashos-installateur flashos-bluetooth flashos-installer flashos-systeme flashos-securite; do
  if [ ! -f "$DOSSIER_SCRIPT/$f" ]; then
    echo "Le fichier $f doit être dans le même dossier que ce script."; exit 1
  fi
done
if ! ls "$DOSSIER_SCRIPT"/locale/*/LC_MESSAGES/flashos.mo >/dev/null 2>&1; then
  echo "Le dossier « locale » (traductions de SoleOS) doit être dans le même dossier que ce script."; exit 1
fi
if ! ls "$DOSSIER_SCRIPT"/polices/*.ttf >/dev/null 2>&1; then
  echo "Le dossier « polices » (Space Grotesk, JetBrains Mono) doit être dans le même dossier que ce script."; exit 1
fi
if ! ls "$DOSSIER_SCRIPT"/fonds/flashos-*.jpg >/dev/null 2>&1; then
  echo "Le dossier « fonds » (fonds d'écran SoleOS) doit être dans le même dossier que ce script."; exit 1
fi

# --- 1. Place disponible -------------------------------------------------
place_libre() { df --output=avail -BG "$1" 2>/dev/null | tail -1 | tr -dc '0-9'; }
mkdir -p "$ATELIER"
LIBRE="$(place_libre "$ATELIER")"
if [ "${LIBRE:-0}" -lt "$ESPACE_NECESSAIRE" ]; then
  echo "==> Seulement ${LIBRE} Go libres, il en faut au moins ${ESPACE_NECESSAIRE}."
  # Recherche d'un disque vide ajouté dans VirtualBox (aucune partition, aucun système de fichiers)
  VIDE=""
  for d in $(lsblk -dn -o NAME,TYPE | awk '$2=="disk"{print $1}'); do
    taille=$(lsblk -dn -b -o SIZE "/dev/$d")
    if [ "$(lsblk -n "/dev/$d" | wc -l)" -eq 1 ] && [ -z "$(blkid -p "/dev/$d" 2>/dev/null)" ] \
       && [ "$taille" -ge $((25*1024*1024*1024)) ]; then VIDE="/dev/$d"; break; fi
  done
  if [ -z "$VIDE" ]; then
    echo "    Ajoutez un deuxième disque de 40 Go à la VM (VirtualBox : Configuration → Stockage),"
    echo "    redémarrez la VM, puis relancez ce script."
    exit 1
  fi
  echo "    Disque vide trouvé : $VIDE ($(lsblk -dn -o SIZE "$VIDE"))."
  read -r -p "    Le formater pour servir d'atelier ? Tout son contenu sera effacé. Tapez OUI : " reponse
  [ "$reponse" = "OUI" ] || { echo "Annulé."; exit 1; }
  mkfs.ext4 -F -L atelier "$VIDE"
  mount "$VIDE" "$ATELIER"
  grep -q "LABEL=atelier" /etc/fstab || echo "LABEL=atelier $ATELIER ext4 defaults,nofail 0 2" >> /etc/fstab
fi
if ! mountpoint -q "$ATELIER" && grep -q "LABEL=atelier" /etc/fstab; then mount "$ATELIER" || true; fi

# --- 2. Outils de construction ----------------------------------------------
echo "==> Installation de l'outil de construction (live-build)…"
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y live-build curl ca-certificates gnupg

cd "$ATELIER"
[ -d config ] && lb clean >/dev/null 2>&1 || true
rm -rf config

lb config \
  --distribution trixie \
  --archive-areas "main contrib non-free non-free-firmware" \
  --debian-installer none \
  --binary-images iso-hybrid \
  --apt-recommends false \
  --firmware-chroot false \
  --firmware-binary false \
  --uefi-secure-boot enable \
  --memtest none \
  --security true \
  --updates true \
  --mirror-binary http://deb.debian.org/debian/ \
  --mirror-binary-security http://security.debian.org/debian-security/ \
  --image-name flashos \
  --iso-application "SoleOS" \
  --iso-publisher "GDH" \
  --iso-volume "SoleOS" \
  --chroot-squashfs-compression-type zstd \
  --chroot-squashfs-compression-level 19 \
  --bootappend-live "boot=live components quiet splash loglevel=3 vt.global_cursor_default=0 preempt=full systemd.show_status=false rd.systemd.show_status=false locales=fr_FR.UTF-8,en_US.UTF-8,it_IT.UTF-8,es_ES.UTF-8,de_DE.UTF-8,zh_CN.UTF-8 keyboard-layouts=fr username=soleos user-fullname=SoleOS hostname=soleos live-media=/dev/disk/by-label/SoleOS live-media-timeout=15"

C="$ATELIER/config"
I="$C/includes.chroot"

# --- 3. Liste des logiciels -------------------------------------------------
cat > "$C/package-lists/flashos.list.chroot" <<'FIN'
# Affichage et session
labwc xwayland libgtk-4-1 dbus-user-session libgl1-mesa-dri
# Applications
chromium chromium-sandbox chromium-l10n
# Messagerie : Thunderbird (comptes Gmail, Outlook… réglés tout seuls), en 6 langues
thunderbird thunderbird-l10n-fr thunderbird-l10n-it thunderbird-l10n-es-es thunderbird-l10n-de thunderbird-l10n-zh-cn
foot
# Applications ajoutées par l'utilisateur (boutique : Flatpak/Flathub ; fichiers AppImage)
flatpak libfuse2t64 glib-networking
# Certificats des sites sécurisés (https) : indispensables à la boutique et aux mises à jour
ca-certificates
# Outils de diagnostic réseau (console, sans droits d'administrateur)
net-tools iputils-ping traceroute bind9-dnsutils curl iproute2
# Fichiers (Nautilus) et Images (Loupe) : applications GNOME modernes (GTK 4), même style que SoleOS.
# Nautilus ouvre et crée aussi les .zip. Portail GTK : thème sombre et boîtes « Ouvrir / Enregistrer »
# pour le navigateur et les applications Flatpak.
nautilus loupe xdg-desktop-portal-gtk adwaita-icon-theme gsettings-desktop-schemas dconf-gsettings-backend dconf-service
# Icônes : lecture des icônes SVG (sans ce paquet, les icônes des applications restent vides ou
# floues) et thème d'icônes coloré Papirus (applications, dossiers colorés dans Fichiers)
librsvg2-common papirus-icon-theme gtk-update-icon-cache
# Trousseau de mots de passe (dossiers réseau, applications) : ouvert tout seul avec la session
gnome-keyring libsecret-1-0
# Vidéo et musique : VLC (lit tous les formats)
vlc qtwayland5
# Conteneurs (Distrobox : logiciels d'autres distributions, paquets .deb) sans toucher au système
podman distrobox uidmap passt fuse-overlayfs podman-docker
# Miniatures (les générateurs tournent dans une « boîte » bubblewrap) ; partages réseau (NAS) vus
# comme des dossiers ordinaires par toutes les applications (gvfs-fuse)
bubblewrap gvfs-fuse
gvfs gvfs-backends udisks2 polkitd xdg-user-dirs xdg-utils desktop-file-utils
# Apparence
fonts-inter fonts-dejavu-core fonts-noto-color-emoji
# Son et réseau
pipewire pipewire-pulse pipewire-alsa wireplumber
# Capture d'écran (touche Impr. écran ; Maj + Impr. écran : une zone)
grim slurp wlsunset swappy wl-clipboard
# Outils de diagnostic (assistance) et essai du son : aplay, amixer, speaker-test, lspci, rfkill
alsa-utils pciutils rfkill
network-manager wpasupplicant iw wireless-regdb
# Écran de démarrage (le texte sert aussi à demander le code)
plymouth plymouth-themes plymouth-label
# Sécurité : écran verrouillé après inactivité, pare-feu, confinement des programmes (AppArmor)
swayidle nftables apparmor
# Barre de progression du son et de la luminosité (touches Fn)
wob
# Calculatrice (dans « Mes applications »)
gnome-calculator
# Disques : formater une clé USB (FAT, exFAT, NTFS), aussi par Fichiers → clic droit → Formater… (v1.0.2)
gnome-disk-utility exfatprogs ntfs-3g
# Mémoire pleine : ferme l'onglet ou l'appli la plus gourmande au lieu de laisser le PC se figer
earlyoom
# Batterie : profils d'énergie (économie automatique sur batterie), régulation thermique Intel
power-profiles-daemon thermald
# Système
locales console-setup keyboard-configuration sudo
# Mémoire compressée (PC à 4 Go)
systemd-zram-generator
# Heure exacte par Internet ; touches volume, luminosité, lecture
systemd-timesyncd brightnessctl playerctl
# Imprimantes (sans pilote : réseau et USB) et scanners
cups cups-client cups-filters ipp-usb avahi-daemon libnss-mdns printer-driver-gutenprint printer-driver-hpcups
# (v13) Tous les pilotes d'impression libres de Debian (Brother, Epson, Samsung, Ricoh, Kyocera…), HP complet
printer-driver-all hplip
sane-airscan
# (numérisation : NAPS2, installé plus bas depuis son site officiel)
# Bluetooth (casques, souris, enceintes)
bluez libspa-0.2-bluetooth
# Visio : partage d'écran ; deuxième écran / vidéoprojecteur
xdg-desktop-portal xdg-desktop-portal-wlr wlr-randr
libgtk4-layer-shell0 wtype
# Bureautique : LibreOffice en français (Texte, Tableur, Présentation), dictionnaires,
# polices compatibles avec celles de Microsoft Office (Calibri, Cambria, Arial, Times…)
libreoffice-writer libreoffice-calc libreoffice-impress libreoffice-gtk3 libreoffice-l10n-fr libreoffice-style-colibre
hunspell-fr hyphen-fr mythes-fr fonts-crosextra-carlito fonts-crosextra-caladea fonts-liberation2
# Autres langues de SoleOS (choisies au premier écran de la clé) : LibreOffice et dictionnaires
# en anglais, italien, espagnol, allemand et chinois ; caractères chinois ; traductions (gettext)
libreoffice-l10n-it libreoffice-l10n-es libreoffice-l10n-de libreoffice-l10n-zh-cn
hunspell-en-us hunspell-it hunspell-es hunspell-de-de
fonts-noto-cjk gettext-base xdg-user-dirs
# Écriture du chinois (pinyin) : lancée seulement quand le chinois est choisi
fcitx5 fcitx5-chinese-addons fcitx5-frontend-gtk3 fcitx5-frontend-gtk4
# Session « essai » : création de l'utilisateur flashos au démarrage de la clé
live-boot live-config live-config-systemd user-setup
libpam-systemd systemd-sysv kbd
# Installation sur le disque (installateur SoleOS)
grub2-common grub-pc-bin grub-efi-amd64-bin grub-efi-amd64-signed shim-signed efibootmgr
cryptsetup dmsetup dosfstools e2fsprogs rsync squashfs-tools fdisk
FIN

# --- 4. Bureautique : LibreOffice (paquets Debian, ci-dessus) ---------------

# --- 5. Fichiers SoleOS ----------------------------------------------------
mkdir -p "$I/usr/local/bin" "$I/usr/local/sbin" "$I/etc/xdg/labwc" "$I/etc/xdg/foot" \
         "$I/etc/chromium" "$I/etc/profile.d" \
         "$I/etc/systemd/system/getty@tty1.service.d"

install -m 755 "$DOSSIER_SCRIPT/flashos-accueil" "$I/usr/local/bin/flashos-accueil"
install -m 755 "$DOSSIER_SCRIPT/flashos-installateur" "$I/usr/local/bin/flashos-installateur"
install -m 755 "$DOSSIER_SCRIPT/flashos-applications" "$I/usr/local/bin/flashos-applications"
install -m 755 "$DOSSIER_SCRIPT/flashos-bluetooth" "$I/usr/local/bin/flashos-bluetooth"
install -m 755 "$DOSSIER_SCRIPT/flashos-installer" "$I/usr/local/sbin/flashos-installer"
install -m 755 "$DOSSIER_SCRIPT/flashos-systeme" "$I/usr/local/sbin/flashos-systeme"
install -m 755 "$DOSSIER_SCRIPT/flashos-securite" "$I/usr/local/sbin/flashos-securite"
#  Fonds d'écran SoleOS (images 4K et petites vignettes pour les Réglages)
#  Polices du thème Éclair (licence SIL OFL 1.1) : titres Space Grotesk, clé de secours JetBrains Mono
mkdir -p "$I/usr/share/fonts/truetype/flashos" "$I/usr/share/doc/flashos/polices"
install -m 644 "$DOSSIER_SCRIPT"/polices/*.ttf "$I/usr/share/fonts/truetype/flashos/"
install -m 644 "$DOSSIER_SCRIPT"/polices/OFL-*.txt "$I/usr/share/doc/flashos/polices/" 2>/dev/null || true
# Traductions de SoleOS et de l'écran verrouillé (gtklock), compilées (.mo)
for f in "$DOSSIER_SCRIPT"/locale/*/LC_MESSAGES/*.mo; do
  l=$(basename "$(dirname "$(dirname "$f")")")
  install -D -m 644 "$f" "$I/usr/share/locale/$l/LC_MESSAGES/$(basename "$f")"
done
mkdir -p "$I/usr/share/flashos/fonds/vignettes"
install -m 644 "$DOSSIER_SCRIPT"/fonds/flashos-*.jpg "$I/usr/share/flashos/fonds/"
install -m 644 "$DOSSIER_SCRIPT"/fonds/vignettes/flashos-*.jpg "$I/usr/share/flashos/fonds/vignettes/" 2>/dev/null || true
#  Icônes de la suite Craft (applications libres MIT / Apache-2.0, installables depuis « Applications »)
if [ -d "$DOSSIER_SCRIPT/craft" ]; then
  mkdir -p "$I/usr/share/flashos/craft"
  install -m 644 "$DOSSIER_SCRIPT"/craft/*.png "$I/usr/share/flashos/craft/"
fi
echo "$VERSION_FLASHOS" > "$I/etc/flashos-version"
# >>> FICHIERS SYSTEME (générés depuis flashos-installateur/systeme : ne pas modifier ici)
mkdir -p "$I/etc/NetworkManager/conf.d"
cat > "$I/etc/NetworkManager/conf.d/flashos-wifi.conf" <<'FINSYS'
# SoleOS : économie d'énergie du Wi-Fi désactivée. Sur beaucoup de cartes (Intel notamment),
# elle divise le débit par 5 à 10 pour un gain d'autonomie minime.
[connection]
wifi.powersave = 2
FINSYS
mkdir -p "$I/etc/chromium.d"
cat > "$I/etc/chromium.d/flashos" <<'FINSYS'
# SoleOS : réglages de Chromium, quelle que soit la façon de l'ouvrir
# (tuiles de l'accueil, lien ou PDF ouvert depuis une autre appli…)
# - affichage Wayland, interface dans la langue choisie ; mots de passe et cookies protégés par le
#   trousseau de la session (ouvert d'office) : les connexions (Google, Gmail…) sont gardées
# - vidéos décodées par la carte graphique (VA-API) : moins de travail pour le processeur, plus d'autonomie
# - partage d'écran (visio) par PipeWire
# - langue de l'interface : celle choisie pour l'ordinateur ; chinois : écriture pinyin (fcitx5)
case "$(cat /run/flashos/langue 2>/dev/null)" in
  en) FLASHOS_LANGUE_WEB=en-US ;; it) FLASHOS_LANGUE_WEB=it ;; es) FLASHOS_LANGUE_WEB=es ;;
  de) FLASHOS_LANGUE_WEB=de ;; zh) FLASHOS_LANGUE_WEB="zh-CN --enable-wayland-ime --wayland-text-input-version=3" ;;
  *) FLASHOS_LANGUE_WEB=fr ;;
esac
export CHROMIUM_FLAGS="$CHROMIUM_FLAGS --ozone-platform=wayland --lang=$FLASHOS_LANGUE_WEB --no-default-browser-check --password-store=gnome-libsecret --enable-remote-extensions --disable-features=Translate --enable-features=AcceleratedVideoDecodeLinuxGL,AcceleratedVideoDecodeLinuxZeroCopyGL,AcceleratedVideoEncoder,VaapiIgnoreDriverChecks,WebRTCPipeWireCapturer"
FINSYS
mkdir -p "$I/etc/chromium/policies/managed"
cat > "$I/etc/chromium/policies/managed/flashos.json" <<'FINSYS'
{
  "BrowserSignin": 0,
  "SyncDisabled": true,
  "BrowserAddPersonEnabled": false,
  "BrowserGuestModeEnabled": false,
  "DefaultBrowserSettingEnabled": false,
  "PromotionalTabsEnabled": false
}
FINSYS
mkdir -p "$I/etc/chromium/policies/recommended"
cat > "$I/etc/chromium/policies/recommended/flashos-recherche.json" <<'FINSYS'
{
  "DefaultSearchProviderEnabled": true,
  "DefaultSearchProviderName": "Google",
  "DefaultSearchProviderKeyword": "google.com",
  "DefaultSearchProviderSearchURL": "{google:baseURL}search?q={searchTerms}&{google:RLZ}{google:originalQueryForSuggestion}{google:assistedQueryStats}{google:searchFieldtrialParameter}{google:searchClient}{google:sourceId}ie={inputEncoding}",
  "DefaultSearchProviderSuggestURL": "{google:baseURL}complete/search?output=chrome&q={searchTerms}",
  "DefaultSearchProviderIconURL": "https://www.google.com/favicon.ico",
  "DefaultSearchProviderEncodings": ["UTF-8"]
}
FINSYS
mkdir -p "$I/etc/initramfs-tools/scripts/local-bottom"
cat > "$I/etc/initramfs-tools/scripts/local-bottom/flashos-protection" <<'FINSYS'
#!/bin/sh
# SoleOS : le système installé tourne en lecture seule.
# Les modifications éventuelles (y compris celles d'un logiciel malveillant)
# ne vivent qu'en mémoire et disparaissent au redémarrage.
PREREQ=""
prereqs() { echo "$PREREQ"; }
case "$1" in prereqs) prereqs; exit 0 ;; esac

grep -q "flashos\.slot=" /proc/cmdline || exit 0          # clé USB, réparation : rien à faire
grep -q "flashos\.ecriture" /proc/cmdline && exit 0        # maintenance exceptionnelle

modprobe overlay 2>/dev/null
grep -qw overlay /proc/filesystems || exit 0

mkdir -p /run/flashos/bas /run/flashos/rw
mount -t tmpfs -o mode=0755,size=25% tmpfs /run/flashos/rw || exit 0
mkdir -p /run/flashos/rw/haut /run/flashos/rw/travail
mount -o remount,ro "${rootmnt}" 2>/dev/null
mount --move "${rootmnt}" /run/flashos/bas || exit 0
if ! mount -t overlay overlay \
     -o lowerdir=/run/flashos/bas,upperdir=/run/flashos/rw/haut,workdir=/run/flashos/rw/travail \
     "${rootmnt}"; then
  mount --move /run/flashos/bas "${rootmnt}"   # en cas d'échec : démarrage normal
fi
exit 0
FINSYS
mkdir -p "$I/etc/modprobe.d"
cat > "$I/etc/modprobe.d/flashos-energie.conf" <<'FINSYS'
# SoleOS : la carte son se met en veille quand rien ne joue (après 1 s)
options snd_hda_intel power_save=1 power_save_controller=Y
FINSYS
mkdir -p "$I/etc"
cat > "$I/etc/nftables.conf" <<'FINSYS'
#!/usr/sbin/nft -f
# SoleOS : pare-feu. Rien n'entre sans y avoir été invité
# (les réponses aux connexions ouvertes par l'ordinateur passent normalement).
flush ruleset

table inet flashos {
  chain entree {
    type filter hook input priority filter; policy drop;
    ct state established,related accept
    ct state invalid drop
    iif lo accept
    meta l4proto { icmp, ipv6-icmp } accept
    # Découverte des imprimantes et appareils du réseau local (mDNS)
    udp dport 5353 accept
    # Adresse réseau automatique (DHCP)
    udp sport 67 udp dport 68 accept
    udp sport 547 udp dport 546 accept
  }
  chain transit {
    type filter hook forward priority filter; policy drop;
  }
}
FINSYS
mkdir -p "$I/etc/polkit-1/rules.d"
cat > "$I/etc/polkit-1/rules.d/50-soleos-cles-usb.rules" <<'FINSYS'
// SoleOS v1.0.2 – © 2026 GDH – LicenseRef-SoleOS
// Clés USB sans compte administrateur, pour l'utilisateur assis devant l'ordinateur :
// - écrire une image (Impression, Disques : créer une clé démarrable) ;
// - formater, renommer, partitionner (Fichiers → Formater…, Disques).
// Seulement les clés et disques amovibles : le disque du système relève d'autres actions
// (« open-device-system », « modify-device-system »), qui restent refusées.
polkit.addRule(function (action, subject) {
  if ((action.id == "org.freedesktop.udisks2.open-device" ||
       action.id == "org.freedesktop.udisks2.modify-device") &&
      subject.local && subject.active) {
    return polkit.Result.YES;
  }
});
FINSYS
mkdir -p "$I/etc/sudoers.d"
cat > "$I/etc/sudoers.d/flashos-heure" <<'FINSYS'
# SoleOS : date et heure depuis les Réglages (valeurs vérifiées par l'outil)
ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-systeme fuseau *, /usr/local/sbin/flashos-systeme heure-auto *, /usr/local/sbin/flashos-systeme heure-regler *
# Langue et clavier (premier écran de la clé, Réglages) : valeurs vérifiées par l'outil
ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-systeme langue *
FINSYS
mkdir -p "$I/etc/sudoers.d"
cat > "$I/etc/sudoers.d/flashos-journal" <<'FINSYS'
# SoleOS : commandes d'administration dans un terminal séparé, et tracées (ANSSI R33, R39)
Defaults use_pty
Defaults log_subcmds
FINSYS
mkdir -p "$I/etc/sudoers.d"
cat > "$I/etc/sudoers.d/flashos-reparation" <<'FINSYS'
ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-systeme preparer-reparation
FINSYS
mkdir -p "$I/etc/sudoers.d"
cat > "$I/etc/sudoers.d/flashos-securite" <<'FINSYS'
# SoleOS : choix et changement du code, écran verrouillé (vérifications faites par l'outil)
ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-securite etat, /usr/local/sbin/flashos-securite preparer-secours, /usr/local/sbin/flashos-securite definir, /usr/local/sbin/flashos-securite definir *, /usr/local/sbin/flashos-securite changer, /usr/local/sbin/flashos-securite nouvelle-cle, /usr/local/sbin/flashos-securite verifier
FINSYS
mkdir -p "$I/etc/sudoers.d"
cat > "$I/etc/sudoers.d/flashos-son" <<'FINSYS'
# SoleOS : l'essai du son (Réglages → Son) règle la puce son ; le programme n'accepte que
# des numéros de recettes connues (voir /usr/local/sbin/flashos-son)
ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-son etat, /usr/local/sbin/flashos-son essayer *, /usr/local/sbin/flashos-son annuler, /usr/local/sbin/flashos-son garder *, /usr/local/sbin/flashos-son oublier, /usr/local/sbin/flashos-son appliquer
FINSYS
mkdir -p "$I/etc/sysctl.d"
cat > "$I/etc/sysctl.d/90-flashos-energie.conf" <<'FINSYS'
# SoleOS : moins de réveils du processeur (batterie)
# Surveillance matérielle du noyau inutile sur un ordinateur personnel
kernel.nmi_watchdog = 0
# Écritures sur le disque regroupées toutes les 15 s au lieu de 5 s
vm.dirty_writeback_centisecs = 1500
FINSYS
mkdir -p "$I/etc/sysctl.d"
cat > "$I/etc/sysctl.d/90-flashos-memoire.conf" <<'FINSYS'
# SoleOS : réglages adaptés à la mémoire compressée
vm.swappiness = 150
vm.page-cluster = 0
vm.watermark_boost_factor = 0
vm.watermark_scale_factor = 125
FINSYS
mkdir -p "$I/etc/sysctl.d"
cat > "$I/etc/sysctl.d/90-flashos-securite.conf" <<'FINSYS'
# SoleOS : réglages de sécurité du noyau (guide ANSSI « Recommandations de sécurité
# relatives à un système GNU/Linux », R9, R11, R12, R14).
# Non appliqués volontairement : interdiction des espaces de noms utilisateur (nécessaires
# au bac à sable de Chromium et de Flatpak) et désactivation d'IPv6 (utilisé par les box).

# Noyau
kernel.dmesg_restrict = 1
kernel.kptr_restrict = 2
kernel.kexec_load_disabled = 1
kernel.perf_event_paranoid = 3
kernel.randomize_va_space = 2
kernel.sysrq = 0
kernel.unprivileged_bpf_disabled = 1
net.core.bpf_jit_harden = 2
kernel.yama.ptrace_scope = 1

# Systèmes de fichiers
fs.protected_hardlinks = 1
fs.protected_symlinks = 1
fs.protected_fifos = 2
fs.protected_regular = 2
fs.suid_dumpable = 0

# Réseau
net.ipv4.conf.all.rp_filter = 1
net.ipv4.conf.default.rp_filter = 1
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.default.accept_redirects = 0
net.ipv4.conf.all.secure_redirects = 0
net.ipv4.conf.default.secure_redirects = 0
net.ipv4.conf.all.send_redirects = 0
net.ipv4.conf.default.send_redirects = 0
net.ipv4.conf.all.accept_source_route = 0
net.ipv4.conf.default.accept_source_route = 0
net.ipv4.icmp_echo_ignore_broadcasts = 1
net.ipv4.icmp_ignore_bogus_error_responses = 1
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_rfc1337 = 1
net.ipv6.conf.all.accept_redirects = 0
net.ipv6.conf.default.accept_redirects = 0
net.ipv6.conf.all.accept_source_route = 0
net.ipv6.conf.default.accept_source_route = 0
FINSYS
mkdir -p "$I/etc/systemd/journald.conf.d"
cat > "$I/etc/systemd/journald.conf.d/flashos.conf" <<'FINSYS'
# SoleOS : journal du système limité (il vit en mémoire, le système étant en lecture seule)
[Journal]
SystemMaxUse=48M
RuntimeMaxUse=32M
MaxRetentionSec=2week
FINSYS
mkdir -p "$I/etc/systemd/system/NetworkManager.service.d"
cat > "$I/etc/systemd/system/NetworkManager.service.d/flashos.conf" <<'FINSYS'
# SoleOS : les réseaux Wi-Fi enregistrés sont dans les données (ouvertes après le code)
[Unit]
RequiresMountsFor=/etc/NetworkManager/system-connections
After=flashos-deverrouiller.service
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-bluetooth.service" <<'FINSYS'
[Unit]
Description=SoleOS : association automatique d'un clavier et d'une souris Bluetooth
# Démarre tôt : un clavier Bluetooth doit pouvoir taper le code des données
After=bluetooth.service dbus.service plymouth-start.service
Wants=bluetooth.service
Before=flashos-deverrouiller.service flashos-peripheriques.service

[Service]
Type=simple
ExecStart=/usr/local/bin/flashos-bluetooth associer 900

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-compte.service" <<'FINSYS'
[Unit]
Description=SoleOS : code de l'écran verrouillé et prénom
ConditionKernelCommandLine=flashos.slot
RequiresMountsFor=/home
After=flashos-deverrouiller.service
Wants=flashos-deverrouiller.service
Before=getty@tty1.service systemd-user-sessions.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-securite restaurer-compte

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-confirmer.service" <<'FINSYS'
[Unit]
Description=SoleOS : validation du démarrage après une mise à jour
ConditionKernelCommandLine=flashos.slot
RequiresMountsFor=/flashos/demarrage

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-systeme confirmer
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-confirmer.timer" <<'FINSYS'
[Unit]
Description=SoleOS : validation du démarrage (dès que l'interface tourne)
ConditionKernelCommandLine=flashos.slot

[Timer]
OnBootSec=60s
OnUnitActiveSec=60s

[Install]
WantedBy=timers.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-deverrouiller.service" <<'FINSYS'
[Unit]
Description=SoleOS : ouverture des données (code ou mot de passe)
ConditionPathExists=/etc/flashos-chiffrement
# Après le Bluetooth : un clavier Bluetooth peut taper le code. Les données sont montées sur
# /home par ce service (pas par fstab) ; la session, le réseau et l'impression l'attendent.
After=bluetooth.service flashos-bluetooth.service plymouth-start.service keyboard-setup.service systemd-vconsole-setup.service flashos-heure.service
Wants=bluetooth.service flashos-bluetooth.service
RequiresMountsFor=/flashos/demarrage
Before=systemd-user-sessions.service getty@tty1.service flashos-compte.service NetworkManager.service flashos-imprimantes-restaurer.service cups.service umount.target
Conflicts=umount.target

[Service]
Type=oneshot
RemainAfterExit=yes
TimeoutSec=0
KeyringMode=shared
ExecStart=/usr/local/sbin/flashos-securite ouvrir

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-energie.service" <<'FINSYS'
[Unit]
Description=SoleOS : profil d'énergie selon le chargeur (économie sur batterie)
After=power-profiles-daemon.service
Wants=power-profiles-daemon.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-energie

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-heure.service" <<'FINSYS'
[Unit]
Description=SoleOS : langue, clavier, fuseau horaire et heure choisis dans les Réglages
DefaultDependencies=no
RequiresMountsFor=/flashos/demarrage
After=local-fs.target
# Avant la saisie du code : messages dans la bonne langue, clavier de la console réglé
Before=sysinit.target time-set.target systemd-timesyncd.service flashos-deverrouiller.service flashos-bluetooth.service
ConditionKernelCommandLine=flashos.slot

[Service]
Type=oneshot
ExecStart=-/usr/local/sbin/flashos-systeme langue-appliquer
ExecStart=/usr/local/sbin/flashos-systeme heure-appliquer

[Install]
WantedBy=sysinit.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-imprimantes-restaurer.service" <<'FINSYS'
[Unit]
Description=SoleOS : remet en place les imprimantes ajoutées
ConditionKernelCommandLine=flashos.slot
RequiresMountsFor=/home
After=flashos-deverrouiller.service
Before=cups.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-systeme restaurer-imprimantes

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-imprimantes-sauver.path" <<'FINSYS'
[Unit]
Description=SoleOS : surveille les imprimantes ajoutées ou retirées
ConditionKernelCommandLine=flashos.slot

[Path]
PathChanged=/etc/cups/printers.conf
PathModified=/etc/cups/ppd
Unit=flashos-imprimantes-sauver.service

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-imprimantes-sauver.service" <<'FINSYS'
[Unit]
Description=SoleOS : garde les imprimantes dans les données
RequiresMountsFor=/home
After=flashos-deverrouiller.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-systeme sauver-imprimantes
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-maj.service" <<'FINSYS'
[Unit]
Description=SoleOS : préparation des mises à jour (en arrière-plan)
ConditionKernelCommandLine=flashos.slot
Wants=network-online.target
After=network-online.target flashos-confirmer.service
RequiresMountsFor=/flashos/demarrage

[Service]
Type=oneshot
Nice=19
IOSchedulingClass=idle
CPUSchedulingPolicy=idle
ExecStart=/usr/local/sbin/flashos-systeme maj
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-maj.timer" <<'FINSYS'
[Unit]
Description=SoleOS : recherche des mises à jour (15 min après l'allumage, puis toutes les 12 h)
ConditionKernelCommandLine=flashos.slot

[Timer]
OnBootSec=15min
OnUnitActiveSec=12h
RandomizedDelaySec=10min

[Install]
WantedBy=timers.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-peripheriques.service" <<'FINSYS'
[Unit]
Description=SoleOS : attente d'un clavier et d'une souris (Bluetooth) avant la session
After=flashos-bluetooth.service flashos-deverrouiller.service
Wants=flashos-bluetooth.service
Before=systemd-user-sessions.service getty@tty1.service

[Service]
Type=oneshot
RemainAfterExit=yes
TimeoutSec=200
ExecStart=/usr/local/bin/flashos-bluetooth attendre 180

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-son.service" <<'FINSYS'
[Unit]
Description=SoleOS : réglages du son propres à ce modèle d'ordinateur (s'il y en a)
After=sound.target flashos-deverrouiller.service
Before=systemd-user-sessions.service getty@tty1.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/flashos-son appliquer

[Install]
WantedBy=multi-user.target
FINSYS
mkdir -p "$I/etc/systemd/system"
cat > "$I/etc/systemd/system/flashos-verrnum.service" <<'FINSYS'
[Unit]
Description=SoleOS : pavé numérique actif dès le démarrage (Verr Num)
DefaultDependencies=no
After=systemd-vconsole-setup.service keyboard-setup.service
Before=flashos-deverrouiller.service sysinit.target

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'for t in /dev/tty1 /dev/tty2 /dev/tty3 /dev/tty4 /dev/tty5 /dev/tty6; do [ -e "$t" ] && setleds -D +num < "$t"; done; true'

[Install]
WantedBy=sysinit.target
FINSYS
mkdir -p "$I/etc/systemd/system/ldconfig.service.d"
cat > "$I/etc/systemd/system/ldconfig.service.d/flashos.conf" <<'FINSYS'
# SoleOS : sur la clé (système « live » en lecture seule), la liste des bibliothèques est déjà à jour
[Unit]
ConditionPathExists=!/run/live/medium
FINSYS
mkdir -p "$I/etc/systemd/user"
cat > "$I/etc/systemd/user/flashos-session.target" <<'FINSYS'
# SoleOS : session graphique (labwc) déclarée à systemd, pour les services qui l'attendent
# (portail GNOME : boîtes « Ouvrir / Enregistrer » de Nautilus, fond d'écran)
[Unit]
Description=Session graphique SoleOS
Documentation=man:systemd.special(7)
BindsTo=graphical-session.target
Wants=graphical-session-pre.target
After=graphical-session-pre.target
FINSYS
mkdir -p "$I/etc/systemd/zram-generator.conf.d"
cat > "$I/etc/systemd/zram-generator.conf.d/flashos.conf" <<'FINSYS'
# SoleOS : mémoire compressée (indispensable sur les PC à 4 Go)
# Les pages peu utilisées sont compressées en mémoire (environ 3 pour 1) au lieu d'aller sur le disque.
[zram0]
zram-size = min(ram, 8192)
compression-algorithm = zstd
swap-priority = 100
FINSYS
mkdir -p "$I/etc/udev/rules.d"
cat > "$I/etc/udev/rules.d/60-flashos-disques.rules" <<'FINSYS'
# SoleOS : ordonnanceur des disques adapté à chaque type de disque (tous les PC) :
# NVMe : aucun (le disque gère lui-même) ; SSD SATA / eMMC / carte : mq-deadline ;
# disque dur à plateaux : BFQ (l'interface reste fluide pendant les grosses copies)
ACTION=="add|change", KERNEL=="nvme[0-9]*n[0-9]*", ENV{DEVTYPE}=="disk", ATTR{queue/scheduler}="none"
ACTION=="add|change", KERNEL=="sd[a-z]*|mmcblk[0-9]*", ENV{DEVTYPE}=="disk", ATTR{queue/rotational}=="0", ATTR{queue/scheduler}="mq-deadline"
ACTION=="add|change", KERNEL=="sd[a-z]*", ENV{DEVTYPE}=="disk", ATTR{queue/rotational}=="1", ATTR{queue/scheduler}="bfq"
FINSYS
mkdir -p "$I/etc/udev/rules.d"
cat > "$I/etc/udev/rules.d/61-flashos-cacher.rules" <<'FINSYS'
# SoleOS : les partitions du système (démarrage, A, B, usine, données) n'apparaissent pas dans
# Fichiers : l'utilisateur ne voit que ses propres disques et clés USB
ENV{ID_FS_LABEL}=="FlashOS-*", ENV{UDISKS_IGNORE}="1"
ENV{ID_PART_ENTRY_NAME}=="FLASHOS-*", ENV{UDISKS_IGNORE}="1"
ENV{ID_FS_TYPE}=="crypto_LUKS", ENV{ID_FS_LABEL}=="FlashOS-Donnees", ENV{UDISKS_IGNORE}="1"
FINSYS
mkdir -p "$I/etc/udev/rules.d"
cat > "$I/etc/udev/rules.d/90-flashos-energie.rules" <<'FINSYS'
# SoleOS : économie d'énergie automatique au branchement / débranchement du chargeur
SUBSYSTEM=="power_supply", ACTION=="change", RUN+="/usr/bin/systemctl --no-block start flashos-energie.service"
FINSYS
mkdir -p "$I/etc/udev/rules.d"
cat > "$I/etc/udev/rules.d/90-flashos-luminosite.rules" <<'FINSYS'
# SoleOS : luminosité de l'écran réglable par l'utilisateur (groupe « video »), sans mot de passe
# (touches Fn et Réglages : sans elle, « Permission denied » et la luminosité ne bougeait pas)
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video /sys/class/backlight/%k/brightness", RUN+="/bin/chmod g+w /sys/class/backlight/%k/brightness"
ACTION=="add", SUBSYSTEM=="leds", KERNEL=="*kbd_backlight*", RUN+="/bin/chgrp video /sys/class/leds/%k/brightness", RUN+="/bin/chmod g+w /sys/class/leds/%k/brightness"
FINSYS
mkdir -p "$I/etc/xdg/xdg-desktop-portal-wlr"
cat > "$I/etc/xdg/xdg-desktop-portal-wlr/config" <<'FINSYS'
# SoleOS : partage de l'écran entier, sans fenêtre de choix supplémentaire
[screencast]
chooser_type=none
max_fps=30
FINSYS
mkdir -p "$I/etc/xdg/xdg-desktop-portal"
cat > "$I/etc/xdg/xdg-desktop-portal/wlroots-portals.conf" <<'FINSYS'
# SoleOS : partage d'écran (visio) et captures via le portail wlroots ;
# apparence (thème sombre) et boîtes « Ouvrir / Enregistrer » via le portail GTK ;
# « Définir comme arrière-plan » (Fichiers…) : portail SoleOS (le fond de l'accueil change)
[preferred]
default=gtk
org.freedesktop.impl.portal.ScreenCast=wlr
org.freedesktop.impl.portal.Screenshot=wlr
org.freedesktop.impl.portal.Wallpaper=flashos
FINSYS
mkdir -p "$I/usr/local/bin"
cat > "$I/usr/local/bin/flashos-veille" <<'FINSYS'
#!/bin/sh
# SoleOS : verrouillage automatique de l'écran.
# Délai en minutes dans ~/.config/flashos/verrouillage (0 = jamais ; 10 par défaut).
# Mise en veille (capot fermé…) : toujours verrouillé avant.
m="$(cat "${XDG_CONFIG_HOME:-$HOME/.config}/flashos/verrouillage" 2>/dev/null || echo 10)"
case "$m" in ''|*[!0-9]*) m=10 ;; esac
if [ "$m" -gt 0 ]; then
  exec swayidle -w timeout $((m * 60)) flashos-verrouiller before-sleep flashos-verrouiller
fi
exec swayidle -w before-sleep flashos-verrouiller
FINSYS
mkdir -p "$I/usr/local/bin"
cat > "$I/usr/local/bin/flashos-verrouiller" <<'FINSYS'
#!/bin/sh
# SoleOS : verrouille l'écran (même code qu'au démarrage).
# Pas encore de code (premier démarrage, clé USB) : rien à verrouiller.
[ -e /run/flashos/verrouillable ] || exit 0
pgrep -u "$(id -u)" -x gtklock >/dev/null && exit 0
exec gtklock -d -s /usr/share/flashos/gtklock.css
FINSYS
mkdir -p "$I/usr/local/libexec"
cat > "$I/usr/local/libexec/flashos-verifier-code" <<'FINSYS'
#!/bin/sh
# SoleOS : vérification du code à l'écran verrouillé (appelé par PAM, code sur l'entrée)
exec sudo -n /usr/local/sbin/flashos-securite verifier
FINSYS
mkdir -p "$I/usr/local/sbin"
cat > "$I/usr/local/sbin/flashos-energie" <<'FINSYS'
#!/bin/sh
# SoleOS : économie d'énergie automatique.
# Sur batterie : profil « économie » (processeur moins gourmand) ; sur secteur : profil « équilibré ».
# Appelé au démarrage et à chaque branchement / débranchement du chargeur (règle udev).
secteur=0
batterie=0
for d in "${FLASHOS_SYS_ALIM:-/sys/class/power_supply}"/*; do
  [ -f "$d/type" ] || continue
  case "$(cat "$d/type")" in
    Mains|USB|USB_C|USB_PD) [ "$(cat "$d/online" 2>/dev/null)" = 1 ] && secteur=1 ;;
    Battery) [ "$(cat "$d/scope" 2>/dev/null)" = Device ] || batterie=1 ;;
  esac
done
[ "$batterie" = 1 ] || exit 0          # PC fixe : rien à faire
if [ "$secteur" = 1 ]; then profil=balanced; alpm=max_performance; else profil=power-saver; alpm=med_power_with_dipm; fi
# Disques SATA : liaison en économie sur batterie
for h in /sys/class/scsi_host/host*/link_power_management_policy; do
  [ -w "$h" ] && echo "$alpm" > "$h" 2>/dev/null
done
for nom in org.freedesktop.UPower.PowerProfiles net.hadess.PowerProfiles; do
  chemin="/$(echo "$nom" | tr . /)"
  busctl set-property "$nom" "$chemin" "$nom" ActiveProfile s "$profil" 2>/dev/null && exit 0
done
exit 0
FINSYS
mkdir -p "$I/usr/local/sbin"
cat > "$I/usr/local/sbin/flashos-son" <<'FINSYS'
#!/bin/bash
# =====================================================================
#  SoleOS – réglages du son propres à un modèle d'ordinateur
#  © 2026 GDH – créé par Giuseppe D'Arco – LicenseRef-SoleOS
#
#  Certaines puces son (HDA) sont câblées autrement que la norme par le fabricant :
#  haut-parleurs alimentés par une sortie « GPIO », sorties non déclarées, prise casque
#  mal détectée. Ces « recettes » se règlent sans rien installer, par le noyau
#  (/sys/class/sound/hwC*D* : init_verbs, user_pin_configs, hints, reconfig).
#
#    flashos-son etat                 puce trouvée, recettes possibles, recettes gardées
#    flashos-son essayer <n> [<n>…]   applique ces recettes tout de suite (son arrêté)
#    flashos-son annuler              revient au réglage d'origine
#    flashos-son garder <n> [<n>…]    garde ces recettes (appliquées à chaque démarrage)
#    flashos-son oublier              efface les recettes gardées
#    flashos-son appliquer            au démarrage (service flashos-son)
#
#  Recettes : 1 sorties GPIO allumées · 2 sorties non déclarées = haut-parleurs ·
#             3 = 1 + 2 · 4 prise casque sans détection · 5 détection de la prise inversée
# =====================================================================
set -uo pipefail
export LC_ALL=C PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

if [ -d /home/.flashos ]; then FICHIER=/home/.flashos/son; else FICHIER=/run/flashos/son; fi

# La puce son analogique (pas celle de l'HDMI) : hwC<carte>D<puce>
trouver_puce() {
  local h c d f
  for h in /sys/class/sound/hwC*D*; do
    [ -e "$h/vendor_id" ] || continue
    c="${h##*hwC}"; d="${c#*D}"; c="${c%%D*}"
    f="/proc/asound/card$c/codec#$d"
    [ -r "$f" ] || continue
    grep -qE 'Speaker at|HP Out at|Line Out at' "$f" || continue
    echo "$h $c $d $f"
    return 0
  done
  return 1
}

PUCE="$(trouver_puce)" || PUCE=""
HW="" CARTE="" ADR="" PROC=""
[ -n "$PUCE" ] && read -r HW CARTE ADR PROC <<< "$PUCE"

identite() { echo "$(cat "$HW/vendor_id" 2>/dev/null):$(cat "$HW/subsystem_id" 2>/dev/null)"; }

# Nombre de GPIO de la puce (« GPIO: io=8 »)
nb_gpio() { sed -n 's/^GPIO: io=\([0-9]*\).*/\1/p' "$PROC" | head -1; }

# Sorties analogiques non déclarées (« [N/A] ») capables de sortir du son
sorties_libres() {
  awk '
    /^Node 0x[0-9a-f]+ \[Pin Complex\]/ { if (n!="" && sortie && libre && !numerique) print n
                                          n=$2; sortie=0; libre=0; numerique=($0 ~ /Digital/); next }
    /^Node / { if (n!="" && sortie && libre && !numerique) print n; n=""; next }
    n!="" && /Pincap/ && / OUT/ { sortie=1 }
    n!="" && /Pin Default/ && /\[N\/A\]/ { libre=1 }
    END { if (n!="" && sortie && libre && !numerique) print n }
  ' "$PROC"
}

valider() {  # recettes : chiffres 1 à 5 seulement
  local r
  [ $# -ge 1 ] || { echo "aucune recette" >&2; exit 2; }
  for r in "$@"; do [[ "$r" =~ ^[1-5]$ ]] || { echo "recette inconnue : $r" >&2; exit 2; }; done
}

# Une écriture = un réglage (le noyau n'en lit qu'un à la fois)
verbe() { echo "$1 $2 $3" > "$HW/init_verbs"; }
indice() { echo "$1" > "$HW/hints"; }

remettre() { echo 1 > "$HW/clear" 2>/dev/null || true; }

reconfigurer() {
  if ! echo 1 > "$HW/reconfig" 2>/dev/null; then
    echo "occupé : le son est en cours d'utilisation" >&2
    return 3
  fi
}

appliquer_recettes() {
  local r p n=0 masque=0 io seq=1
  remettre
  for r in "$@"; do
    case "$r" in
      1|3)
        io="$(nb_gpio)"; io="${io:-0}"
        if [ "$io" -gt 0 ]; then
          masque=$(( (1 << io) - 1 ))
          verbe 0x01 0x716 "$(printf '0x%x' "$masque")"   # masque des GPIO
          verbe 0x01 0x717 "$(printf '0x%x' "$masque")"   # en sortie
          verbe 0x01 0x715 "$(printf '0x%x' "$masque")"   # allumées
        fi ;;
    esac
    case "$r" in
      2|3)
        for p in $(sorties_libres); do
          printf '%s 0x9017011%x\n' "$p" "$seq" > "$HW/user_pin_configs"
          verbe "$p" 0x70c 0x2                              # amplificateur (EAPD) allumé
          seq=$((seq + 1)); n=$((n + 1))
        done ;;
      4) indice 'jack_detect = no'; indice 'auto_mute = no' ;;
      5) indice 'inv_jack_detect = yes' ;;
    esac
  done
  reconfigurer
}

case "${1:-etat}" in
  etat)
    if [ -z "$PUCE" ]; then echo "puce=aucune"; exit 1; fi
    echo "puce=$(sed -n 's/^Codec: //p' "$PROC" | head -1)"
    echo "carte=$CARTE"
    echo "identite=$(identite)"
    echo "gpio=$(nb_gpio)"
    echo "sorties_libres=$(sorties_libres | paste -sd, -)"
    echo "reglable=$([ -w "$HW/reconfig" ] && [ -e "$HW/init_verbs" ] && echo oui || echo non)"
    if [ -f "$FICHIER" ]; then echo "gardees=$(sed -n 's/^RECETTES=//p' "$FICHIER")"; else echo "gardees="; fi
    ;;
  essayer)
    shift; valider "$@"
    [ -n "$PUCE" ] || { echo "aucune puce son" >&2; exit 1; }
    appliquer_recettes "$@"
    ;;
  annuler)
    [ -n "$PUCE" ] || exit 0
    remettre; reconfigurer
    ;;
  garder)
    shift; valider "$@"
    [ -n "$PUCE" ] || { echo "aucune puce son" >&2; exit 1; }
    mkdir -p "$(dirname "$FICHIER")"
    printf 'IDENTITE=%s\nRECETTES=%s\n' "$(identite)" "$*" > "$FICHIER"
    chmod 644 "$FICHIER"
    ;;
  oublier)
    rm -f "$FICHIER"
    ;;
  appliquer)
    [ -n "$PUCE" ] && [ -f "$FICHIER" ] || exit 0
    # Seulement sur l'ordinateur pour lequel les recettes ont été trouvées
    [ "$(sed -n 's/^IDENTITE=//p' "$FICHIER")" = "$(identite)" ] || exit 0
    read -ra liste <<< "$(sed -n 's/^RECETTES=//p' "$FICHIER")"
    for r in "${liste[@]}"; do [[ "$r" =~ ^[1-5]$ ]] || exit 0; done
    [ ${#liste[@]} -gt 0 ] && appliquer_recettes "${liste[@]}"
    ;;
  *) echo "usage : flashos-son etat | essayer <n>… | annuler | garder <n>… | oublier | appliquer" >&2; exit 2 ;;
esac
FINSYS
mkdir -p "$I/usr/share/dbus-1/services"
cat > "$I/usr/share/dbus-1/services/org.freedesktop.impl.portal.desktop.flashos.service" <<'FINSYS'
[D-BUS Service]
Name=org.freedesktop.impl.portal.desktop.flashos
Exec=/usr/local/bin/flashos-accueil --portail-fond
FINSYS
mkdir -p "$I/usr/share/flashos"
cat > "$I/usr/share/flashos/gtklock.css" <<'FINSYS'
/* SoleOS : écran verrouillé (thème Éclair) */
window {
  background-color: #0B1020;
  background-image: linear-gradient(115deg, transparent 72%, alpha(#FFC23D, 0.07) 72.2%, transparent 72.6%);
  color: #E9ECF5;
  font-family: "Inter", "DejaVu Sans", sans-serif;
}
label { color: #E9ECF5; font-size: 18px; }
#clock-label { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 96px; font-weight: 500; letter-spacing: -3px; }
#date-label { font-size: 20px; color: #A7B0C8; }
entry {
  min-height: 46px; min-width: 280px; padding: 4px 18px; border-radius: 14px; border: 1px solid #2A3352;
  background: #141A2E; color: #E9ECF5; font-size: 22px; box-shadow: none; caret-color: #FFC23D;
}
entry:focus { border-color: #FFC23D; box-shadow: none; }
button {
  border-radius: 14px; border: none; padding: 8px 18px; color: #1A1300; box-shadow: none;
  background: #FFC23D; background-image: none; font-size: 16px; font-weight: 600;
}
button label { color: #1A1300; }
#error-label, #warning-label { color: #FF9B6B; }
FINSYS
mkdir -p "$I/usr/share/xdg-desktop-portal/portals"
cat > "$I/usr/share/xdg-desktop-portal/portals/flashos.portal" <<'FINSYS'
[portal]
DBusName=org.freedesktop.impl.portal.desktop.flashos
Interfaces=org.freedesktop.impl.portal.Wallpaper
UseIn=wlroots;labwc
FINSYS
chmod 755 "$I/etc/initramfs-tools/scripts/local-bottom/flashos-protection"
chmod 440 "$I/etc/sudoers.d/flashos-heure"
chmod 440 "$I/etc/sudoers.d/flashos-journal"
chmod 440 "$I/etc/sudoers.d/flashos-reparation"
chmod 440 "$I/etc/sudoers.d/flashos-securite"
chmod 440 "$I/etc/sudoers.d/flashos-son"
chmod 755 "$I/usr/local/bin/flashos-veille"
chmod 755 "$I/usr/local/bin/flashos-verrouiller"
chmod 755 "$I/usr/local/libexec/flashos-verifier-code"
chmod 755 "$I/usr/local/sbin/flashos-energie"
chmod 755 "$I/usr/local/sbin/flashos-son"
# <<< FICHIERS SYSTEME
#  Sur la clé, l'installateur peut lancer son moteur en administrateur sans mot de passe
mkdir -p "$I/etc/sudoers.d"
echo 'ALL ALL=(root) NOPASSWD: /usr/local/sbin/flashos-installer' > "$I/etc/sudoers.d/flashos-installateur"
chmod 440 "$I/etc/sudoers.d/flashos-installateur"

cat > "$I/usr/local/bin/flashos-web" <<'FIN'
#!/bin/sh
# Les réglages de Chromium sont dans /etc/chromium.d/flashos (communs à toutes les façons de l'ouvrir)
exec chromium "$@"
FIN

#  Connexion automatique du premier utilisateur (essai : « flashos » ; après installation : le vôtre)
cat > "$I/usr/local/sbin/flashos-getty" <<'FIN'
#!/bin/sh
trouver() { awk -F: '$3>=1000 && $3<60000 && $7 !~ /(nologin|false)$/ {print $1; exit}' /etc/passwd; }
# Sur la clé, l'utilisateur est créé au démarrage : on lui laisse jusqu'à 20 s.
U=$(trouver); n=0
while [ -z "$U" ] && [ -d /run/live ] && [ $n -lt 40 ]; do sleep 0.5; n=$((n+1)); U=$(trouver); done
if [ -n "$U" ]; then
  exec /sbin/agetty --noclear --noissue --nohints --skip-login --login-options "-p -f -- $U" "$1" "$2"
fi
exec /sbin/agetty -o "-p -- \\u" --noclear "$1" "$2"
FIN
cat > "$I/etc/systemd/system/getty@tty1.service.d/zz-flashos.conf" <<'FIN'
[Service]
ExecStart=
ExecStart=-/usr/local/sbin/flashos-getty %I $TERM
FIN

#  Le logo reste affiché jusqu'à l'apparition de l'interface (pas d'écran noir ni de texte)
mkdir -p "$I/etc/systemd/system/plymouth-quit.service.d"
cat > "$I/etc/systemd/system/plymouth-quit.service.d/zz-flashos.conf" <<'FIN'
[Service]
ExecStart=
ExecStart=-/usr/bin/plymouth quit --retain-splash
FIN

#  Allège le fichier de démarrage (initramfs) : les pilotes Wi-Fi, Bluetooth… ne servent
#  qu'une fois le système lancé. Démarrage plus rapide, surtout depuis une clé USB.
mkdir -p "$I/etc/initramfs-tools/hooks"
cat > "$I/etc/initramfs-tools/hooks/zz-flashos-allege" <<'FIN'
#!/bin/sh
# Retire du fichier de démarrage les pilotes Wi-Fi / Bluetooth et leurs firmwares :
# ils seront chargés normalement juste après, depuis le système complet.
PREREQ=""
prereqs() { echo "$PREREQ"; }
case "$1" in prereqs) prereqs; exit 0 ;; esac
[ -n "$DESTDIR" ] && [ -d "$DESTDIR" ] || exit 0
for m in "$DESTDIR"/usr/lib/modules/*/kernel "$DESTDIR"/lib/modules/*/kernel; do
  [ -d "$m" ] && [ ! -L "$m" ] || continue
  rm -rf "$m/drivers/net/wireless" "$m/drivers/bluetooth" "$m/net/wireless" "$m/net/mac80211" "$m/net/bluetooth"
done
for d in "$DESTDIR/usr/lib/firmware" "$DESTDIR/lib/firmware"; do
  [ -d "$d" ] && [ ! -L "$d" ] || continue
  for motif in iwlwifi-* ath9k_htc ath10k ath11k ath12k ath3k* ar3k rtw88 rtw89 rtlwifi rtl_bt \
               brcm cypress mrvl mediatek qca ti-connectivity libertas intel/ibt-*; do
    rm -rf "$d"/$motif
  done
done
exit 0
FIN

#  Préparation du dossier personnel à la première connexion
mkdir -p "$I/usr/share/flashos"
cat > "$I/usr/share/flashos/gtk4.css" <<'FIN'
/* SoleOS : applications GNOME (Fichiers, Images, Calculatrice…) en clair, accent bleu SoleOS */
@define-color accent_bg_color #2563EB;
@define-color accent_color #1D4ED8;
@define-color accent_fg_color #ffffff;
:root {
  --accent-bg-color: #2563EB; --accent-color: #1D4ED8; --accent-fg-color: #ffffff;
}
FIN
cat > "$I/usr/local/bin/flashos-preparer-utilisateur" <<'FIN'
#!/bin/bash
xdg-user-dirs-update 2>/dev/null || true
# Couleurs SoleOS pour les applications GNOME (Fichiers, Images, Calculatrice) : bleu nuit
if [ ! -f "$HOME/.config/gtk-4.0/gtk.css" ]; then
  mkdir -p "$HOME/.config/gtk-4.0" && cp /usr/share/flashos/gtk4.css "$HOME/.config/gtk-4.0/gtk.css"
fi
# Fonds d'écran SoleOS rangés aussi dans le dossier Images (une seule fois)
IMAGES="$(xdg-user-dir PICTURES 2>/dev/null)"
if [ -n "$IMAGES" ] && [ "$IMAGES" != "$HOME" ] && [ ! -e "$HOME/.config/flashos/fonds-copies" ]; then
  mkdir -p "$IMAGES/Fonds SoleOS" "$HOME/.config/flashos" && \
    for f in /usr/share/flashos/fonds/flashos-*.jpg; do n="$(basename "$f" .jpg)"; n="${n#flashos-}"; [ "$n" = eclair ] && n=soleil; cp -n "$f" "$IMAGES/Fonds SoleOS/soleos-$n.jpg" 2>/dev/null; done; \
    touch "$HOME/.config/flashos/fonds-copies"
fi
# Trousseau (mots de passe des dossiers réseau, des applications) toujours ouvert, sans mot de
# passe à retaper : trousseau « login » sans mot de passe, créé une fois. Il est rangé dans les
# données personnelles, déjà chiffrées par le code SoleOS.
K="$HOME/.local/share/keyrings"
if [ ! -e "$K/login.keyring" ]; then
  mkdir -p "$K" && chmod 700 "$K"
  printf '[keyring]\ndisplay-name=Login\nctime=0\nmtime=0\nlock-on-idle=false\nlock-after=false\n' > "$K/login.keyring"
  chmod 600 "$K/login.keyring"
  [ -e "$K/default" ] || printf 'login' > "$K/default"
fi
# Bureautique : formats Microsoft Office (.docx, .xlsx, .pptx) à l'enregistrement, réglés aussi dans
# le profil de l'utilisateur (ce niveau passe avant tout le reste ; un ancien choix ne les annule plus).
# Seulement quand LibreOffice est fermé (il réécrit ce fichier en quittant).
LO="$HOME/.config/libreoffice/4/user"; X="$LO/registrymodifications.xcu"
if ! pgrep -u "$(id -u)" -x soffice.bin >/dev/null 2>&1; then
  mkdir -p "$LO"
  if [ ! -s "$X" ] || ! grep -q '</oor:items>' "$X"; then
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' \
      '<oor:items xmlns:oor="http://openoffice.org/2001/registry" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">' \
      '</oor:items>' > "$X"
  fi
  F="/org.openoffice.Setup/Office/Factories/org.openoffice.Setup:Factory"
  # Fenêtres (v12.4) : taille « normale » à 80 % de l'écran ; le gestionnaire de fenêtres les
  # ouvre en grand (règle « libreoffice » de rc.xml) et le bouton □ les ramène à cette taille
  # dès le premier clic. (LibreOffice ne sait pas s'ouvrir lui-même « en grand » sous Wayland.)
  E="$(cat /sys/class/drm/card*-*/modes 2>/dev/null | head -1)"; LE="${E%x*}"; HE="${E#*x}"
  case "$LE$HE" in *[!0-9]*|"") LE=1366; HE=768 ;; esac
  LF=$((LE * 80 / 100)); HF=$((HE * 80 / 100))
  [ $LF -lt 800 ] && LF=800; [ $HF -lt 540 ] && HF=540
  sed -i '/ooSetupFactoryWindowAttributes/d' "$X"
  sed -i -e '/ooSetupFactoryDefaultFilter/d' -e '/WarnAlienFormat/d' -e '/"SymbolStyle"/d' -e '/"StartWithTemplate"/d' -e '/Impress\/Misc\/NewDoc/d' -e '\#</oor:items>#d' "$X"
  {
    printf '<item oor:path="%s[%s]"><prop oor:name="ooSetupFactoryDefaultFilter" oor:op="fuse"><value>%s</value></prop></item>\n' \
      "$F" "'com.sun.star.text.TextDocument'" "MS Word 2007 XML" \
      "$F" "'com.sun.star.sheet.SpreadsheetDocument'" "Calc MS Excel 2007 XML" \
      "$F" "'com.sun.star.presentation.PresentationDocument'" "Impress MS PowerPoint 2007 XML"
    printf '%s\n' '<item oor:path="/org.openoffice.Office.Common/Save/Document"><prop oor:name="WarnAlienFormat" oor:op="fuse"><value>false</value></prop></item>'
    # (v12.6) icônes Colibre ; Impress s'ouvre sur une présentation vide, sans la fenêtre des thèmes
    printf '%s\n' '<item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="SymbolStyle" oor:op="fuse"><value>colibre</value></prop></item>'
    printf '%s\n' '<item oor:path="/org.openoffice.Office.Impress/Misc/NewDoc"><prop oor:name="AutoPilot" oor:op="fuse"><value>false</value></prop></item>'
    for M in com.sun.star.text.TextDocument com.sun.star.sheet.SpreadsheetDocument com.sun.star.presentation.PresentationDocument; do
      printf '<item oor:path="%s[%s]"><prop oor:name="ooSetupFactoryWindowAttributes" oor:op="fuse"><value>%s</value></prop></item>\n' \
        "$F" "'$M'" "$(( (LE - LF) / 2 )),$(( (HE - HF) / 2 )),$LF,$HF;1;0,0,0,0;"
    done
    printf '%s\n' '</oor:items>'
  } >> "$X"
fi
# Navigateur (v12.4) : la « connexion à Chromium » est coupée par une règle (fichier
# /etc/chromium/policies/managed/flashos.json, BrowserSignin = 0). Ce Chromium n'a pas les clés
# d'accès de Google : en essayant de relier Gmail à un compte Chrome, il effaçait la session Google
# à chaque fermeture. Sans cette liaison, Google est un site comme les autres : la connexion reste.
# Navigateur (v12.1) : cookies « tiers » de nouveau acceptés. Les bloquer déconnectait les comptes
# Google (Gmail, google.fr…) et certains portails à chaque ouverture. Une seule fois, navigateur fermé.
P="$HOME/.config/chromium/Default/Preferences"
if [ -f "$P" ] && [ ! -e "$HOME/.config/flashos/cookies-v121" ] && ! pgrep -u "$(id -u)" -x chromium >/dev/null; then
  sed -i 's/"cookie_controls_mode":2/"cookie_controls_mode":0/' "$P" && mkdir -p "$HOME/.config/flashos" && touch "$HOME/.config/flashos/cookies-v121"
fi
# Dossiers en couleur dans Fichiers (thème d'icônes Papirus) : Documents bleu, Téléchargements vert,
# Images orange, Musique rose, Vidéos violet, Bureau turquoise. Une couleur choisie par l'utilisateur
# (clic droit → Propriétés) est respectée.
(
  for c in DOCUMENTS:blue:documents DOWNLOAD:green:download PICTURES:orange:pictures \
           MUSIC:pink:music VIDEOS:violet:videos DESKTOP:teal:desktop; do
    d="$(xdg-user-dir "${c%%:*}" 2>/dev/null)"
    [ -n "$d" ] && [ -d "$d" ] && [ "$d" != "$HOME" ] || continue
    gio info -a metadata::custom-icon-name -a metadata::custom-icon "$d" 2>/dev/null | grep -q 'metadata::custom-icon' && continue
    r="${c#*:}"
    gio set "$d" metadata::custom-icon-name "folder-${r%%:*}-${r#*:}" 2>/dev/null
  done
) &
# (v13.1) Fichiers : clic droit → Scripts → « Ajouter au bureau » (nom dans la langue de l'utilisateur)
NS="$HOME/.local/share/nautilus/scripts"
case "${LANG:-fr}" in
  en*) NOM_SCRIPT="Add to desktop" ;; it*) NOM_SCRIPT="Aggiungi al desktop" ;; es*) NOM_SCRIPT="Añadir al escritorio" ;;
  de*) NOM_SCRIPT="Zum Schreibtisch hinzufügen" ;; zh*) NOM_SCRIPT="添加到桌面" ;; *) NOM_SCRIPT="Ajouter au bureau" ;;
esac
if [ ! -e "$NS/$NOM_SCRIPT" ]; then
  mkdir -p "$NS" && find "$NS" -maxdepth 1 -type l -lname /usr/local/bin/flashos-ajouter-bureau -delete 2>/dev/null
  ln -sf /usr/local/bin/flashos-ajouter-bureau "$NS/$NOM_SCRIPT"
fi
SIGNETS="$HOME/.config/gtk-3.0/bookmarks"
[ -f "$SIGNETS" ] && exit 0
mkdir -p "$(dirname "$SIGNETS")"
encoder() { local LC_ALL=C s="$1" i c o=""; for (( i=0; i<${#s}; i++ )); do c="${s:i:1}"
  case "$c" in [a-zA-Z0-9/._~-]) o+="$c";; *) o+="$(printf '%%%02X' "'$c")";; esac; done; printf '%s' "$o"; }
for t in DOCUMENTS DOWNLOAD PICTURES VIDEOS MUSIC; do
  d="$(xdg-user-dir "$t" 2>/dev/null)"
  [ -n "$d" ] && [ "$d" != "$HOME" ] && [ -d "$d" ] && echo "file://$(encoder "$d") $(basename "$d")" >> "$SIGNETS"
done
FIN

#  Démarrage de l'interface SoleOS sur l'écran principal
cat > "$I/etc/profile.d/zz-flashos.sh" <<'FIN'
if [ "$(tty)" = "/dev/tty1" ] && [ -z "${WAYLAND_DISPLAY:-}" ] && [ "$(id -u)" -ge 1000 ]; then
  /usr/local/bin/flashos-preparer-utilisateur >/dev/null 2>&1
  # Attendre que le vrai pilote graphique remplace l'affichage provisoire du démarrage :
  # d'abord la fin du chargement des pilotes (15 s au plus), puis 6 s au plus pour l'écran.
  udevadm settle --timeout=15 2>/dev/null
  n=0
  while [ $n -lt 12 ]; do
    provisoire=0
    for c in /sys/class/drm/card[0-9]*; do
      [ -e "$c/device/driver" ] || continue
      [ "$(basename "$(readlink -f "$c/device/driver")")" = "simple-framebuffer" ] && provisoire=1
    done
    [ $provisoire -eq 0 ] && break
    sleep 0.5; n=$((n+1))
  done
  # Rendu de l'interface GTK : OpenGL (« ngl »), le plus sûr et le plus rapide sur les cartes
  # graphiques anciennes (Vulkan peut y être lent) ; machine virtuelle : rendu logiciel
  export GSK_RENDERER=ngl
  if systemd-detect-virt -q --vm; then export WLR_RENDERER=pixman GSK_RENDERER=cairo; fi
  # Partage d'écran (visio) : le portail wlroots doit savoir quel bureau tourne
  export XDG_CURRENT_DESKTOP=wlroots XDG_SESSION_TYPE=wayland
  # Curseur de souris assorti au thème (sinon curseur X11 d'origine)
  export XCURSOR_THEME=Adwaita XCURSOR_SIZE=24
  # LibreOffice : apparence GTK (comme les autres applications)
  export SAL_USE_VCLPLUGIN=gtk3
  # Clavier : celui choisi (premier écran de la clé, Réglages → Langue et clavier),
  # AZERTY français par défaut. Le changement en cours de session passe par
  # ~/.config/labwc/environment, réécrit à chaque choix.
  XKBLAYOUT=""; XKBVARIANT=""; XKBMODEL=""; XKBOPTIONS=""
  [ -r /etc/default/keyboard ] && . /etc/default/keyboard
  if [ -z "$XKBLAYOUT" ]; then
    XKBLAYOUT=fr; XKBVARIANT=""; XKBMODEL=pc105
  fi
  # Langue choisie pour l'ordinateur (écrite au démarrage, ou par le premier écran de la clé)
  case "$(cat /run/flashos/langue 2>/dev/null)" in
    en) export LANG=en_US.UTF-8 LANGUAGE=en_US:en ;;
    it) export LANG=it_IT.UTF-8 LANGUAGE=it_IT:it ;;
    es) export LANG=es_ES.UTF-8 LANGUAGE=es_ES:es ;;
    de) export LANG=de_DE.UTF-8 LANGUAGE=de_DE:de ;;
    zh) export LANG=zh_CN.UTF-8 LANGUAGE=zh_CN:zh GTK_IM_MODULE=fcitx QT_IM_MODULE=fcitx ;;
    fr) export LANG=fr_FR.UTF-8 LANGUAGE=fr_FR:fr ;;
  esac
  # Écriture du chinois (fcitx5) : applications X11 éventuelles
  export XMODIFIERS=@im=fcitx
  # Clavier de la session : celui de ~/.config/labwc/environment s'il a été choisi depuis
  if [ -r "$HOME/.config/labwc/environment" ]; then
    l="$(sed -n 's/^XKB_DEFAULT_LAYOUT=//p' "$HOME/.config/labwc/environment" | tail -1)"
    [ -n "$l" ] && [ ! -d /run/live/medium ] && [ "$l" != "$XKBLAYOUT" ] && \
      sed -i '/^XKB_DEFAULT_\(LAYOUT\|VARIANT\)=/d' "$HOME/.config/labwc/environment"
  fi
  export XKB_DEFAULT_LAYOUT="$XKBLAYOUT" XKB_DEFAULT_VARIANT="$XKBVARIANT" \
         XKB_DEFAULT_MODEL="$XKBMODEL" XKB_DEFAULT_OPTIONS="$XKBOPTIONS"
  echo "clavier : $XKB_DEFAULT_LAYOUT $XKB_DEFAULT_VARIANT" > "$HOME/.flashos-clavier.log" 2>/dev/null
  debut=$(date +%s)
  labwc >"$HOME/.flashos.log" 2>&1
  if [ $(( $(date +%s) - debut )) -lt 10 ]; then
    # Le logo de démarrage peut encore masquer la console : on la réaffiche
    chvt 2 2>/dev/null; sleep 0.3; chvt 1 2>/dev/null
    case "$LANG" in
      en*) echo "SoleOS stopped too quickly. Details: cat ~/.flashos.log" ;;
      it*) echo "SoleOS si è fermato troppo presto. Dettagli: cat ~/.flashos.log" ;;
      es*) echo "SoleOS se detuvo demasiado pronto. Detalles: cat ~/.flashos.log" ;;
      de*) echo "SoleOS wurde zu schnell beendet. Details: cat ~/.flashos.log" ;;
      zh*) echo "SoleOS 过早停止。详情：cat ~/.flashos.log" ;;
      *) echo "SoleOS s'est arrêté trop vite. Détails : cat ~/.flashos.log" ;;
    esac
  else
    exit
  fi
fi
FIN

cat > "$I/etc/xdg/labwc/rc.xml" <<'FIN'
<?xml version="1.0"?>
<labwc_config>
  <theme>
    <cornerRadius>10</cornerRadius>
    <dropShadows>yes</dropShadows>
    <font place="ActiveWindow"><name>Inter</name><size>11</size><weight>bold</weight></font>
    <font place="InactiveWindow"><name>Inter</name><size>11</size></font>
  </theme>
  <core><gap>0</gap></core>
  <placement><policy>cascade</policy></placement>
  <!-- Fenêtre glissée contre un bord : moitié gauche / droite (en haut : tout l'écran), avec aperçu -->
  <snapping>
    <range>12</range>
    <overlay><enabled>yes</enabled><delay inner="250" outer="250" /></overlay>
    <topMaximize>yes</topMaximize>
  </snapping>
  <keyboard>
    <!-- Pavé numérique actif d'office (codes, calculs) -->
    <numlock>on</numlock>
    <default />
    <keybind key="C-A-t"><action name="Execute" command="/usr/local/bin/flashos-console" /></keybind>
    <keybind key="Super_L" onRelease="yes"><action name="Execute" command="/usr/local/bin/flashos-accueil" /></keybind>
    <!-- ⊞ + ← / → / ↑ / ↓ : moitié gauche / droite, plein écran, fenêtre (raccourcis par défaut de labwc) -->
    <keybind key="C-A-a"><action name="Execute" command="/usr/local/bin/flashos-accueil" /></keybind>
    <!-- Alt + Tab : sélecteur SoleOS avec les icônes des applications (remplace la liste de labwc) -->
    <keybind key="A-Tab"><action name="Execute" command="/usr/local/bin/flashos-accueil --alttab" /></keybind>
    <keybind key="A-S-Tab"><action name="Execute" command="/usr/local/bin/flashos-accueil --alttab-arriere" /></keybind>
    <keybind key="W-l"><action name="Execute" command="/usr/local/bin/flashos-verrouiller" /></keybind>
    <keybind key="XF86ScreenSaver"><action name="Execute" command="/usr/local/bin/flashos-verrouiller" /></keybind>
    <!-- Touches spéciales du clavier (Fn) : volume, micro, luminosité, lecture -->
    <keybind key="Print"><action name="Execute" command="/usr/local/bin/flashos-capture" /></keybind>
    <keybind key="S-Print"><action name="Execute" command="/usr/local/bin/flashos-capture zone" /></keybind>
    <keybind key="W-S-s"><action name="Execute" command="/usr/local/bin/flashos-capture zone" /></keybind>
    <keybind key="C-S-Print"><action name="Execute" command="/usr/local/bin/flashos-capture edition" /></keybind>
    <keybind key="XF86AudioRaiseVolume"><action name="Execute" command="/usr/local/bin/flashos-touche son-plus" /></keybind>
    <keybind key="XF86AudioLowerVolume"><action name="Execute" command="/usr/local/bin/flashos-touche son-moins" /></keybind>
    <keybind key="XF86AudioMute"><action name="Execute" command="/usr/local/bin/flashos-touche son-coupe" /></keybind>
    <keybind key="XF86AudioMicMute"><action name="Execute" command="wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle" /></keybind>
    <keybind key="XF86MonBrightnessUp"><action name="Execute" command="/usr/local/bin/flashos-touche lumiere-plus" /></keybind>
    <keybind key="XF86MonBrightnessDown"><action name="Execute" command="/usr/local/bin/flashos-touche lumiere-moins" /></keybind>
    <keybind key="XF86AudioPlay"><action name="Execute" command="playerctl play-pause" /></keybind>
    <keybind key="XF86AudioPause"><action name="Execute" command="playerctl play-pause" /></keybind>
    <keybind key="XF86AudioNext"><action name="Execute" command="playerctl next" /></keybind>
    <keybind key="XF86AudioPrev"><action name="Execute" command="playerctl previous" /></keybind>
  </keyboard>
  <windowRules>
    <!-- Les applications s'ouvrent en fenêtre (plusieurs visibles à la fois, en cascade) ;
         seules les pages SoleOS (applications, installation…) occupent tout l'écran. -->
    <windowRule identifier="fr.flashos.Accueil"><action name="Maximize" /></windowRule>
    <windowRule identifier="fr.flashos.Langue"><action name="Maximize" /></windowRule>
    <windowRule identifier="fr.flashos.Bienvenue"><action name="Maximize" /></windowRule>
    <windowRule identifier="fr.flashos.Installateur"><action name="Maximize" /></windowRule>
    <!-- Bureautique : documents ouverts en grand (□ les ramène à 80 % de l'écran) -->
    <windowRule identifier="libreoffice*" type="normal"><action name="Maximize" /></windowRule>
    <!-- Assistant de bienvenue (choix du code) : toujours devant, jusqu'au bout -->
    <windowRule identifier="fr.flashos.Bienvenue"><action name="ToggleAlwaysOnTop" /></windowRule>
    <!-- Fenêtres SoleOS et applications GNOME (Fichiers, Images, Calculatrice) : elles dessinent
         elles-mêmes leur barre ; jamais de deuxième barre ajoutée par-dessus -->
    <windowRule identifier="fr.flashos.*" serverDecoration="no" />
    <!-- … sauf « Applications » et la fenêtre d'installation d'un paquet : barre SoleOS avec − □ ✕ -->
    <windowRule identifier="fr.flashos.Applications" serverDecoration="yes" />
    <windowRule identifier="fr.flashos.PaquetDeb" serverDecoration="yes" />
    <windowRule identifier="org.gnome.*" serverDecoration="no" />
    <!-- Applis sans barre de titre propre (Console…) : barre SoleOS avec − □ ✕ -->
    <windowRule identifier="foot" serverDecoration="yes" />
    <windowRule identifier="chromium*" serverDecoration="yes" />
    <windowRule identifier="thunderbird*" serverDecoration="yes" />
  </windowRules>
</labwc_config>
FIN

cat > "$I/etc/xdg/labwc/themerc-override" <<'FIN'
border.width: 0
padding.height: 8
window.active.title.bg.color: #141A2E
window.inactive.title.bg.color: #0E1426
window.active.label.text.color: #f2f4f8
window.inactive.label.text.color: #a9b3c9
window.label.text.justify: center
window.active.button.unpressed.image.color: #f2f4f8
window.inactive.button.unpressed.image.color: #a9b3c9
window.active.border.color: #141A2E
window.inactive.border.color: #0E1426
# (v13.1) ombre douce sous les fenêtres (comme Windows 11 et macOS) ; ignorée par un labwc plus ancien
window.active.shadow.size: 26
window.inactive.shadow.size: 16
window.active.shadow.color: #00000070
window.inactive.shadow.color: #00000045
FIN

cat > "$I/etc/xdg/labwc/autostart" <<'FIN'
dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE >/dev/null 2>&1
systemctl --user start flashos-session.target >/dev/null 2>&1 &
# Résolution d'origine et taille d'affichage AVANT la première fenêtre (5 s au plus)
timeout 5 /usr/local/bin/flashos-accueil --taille >/dev/null 2>&1
sh -c 'while true; do /usr/local/bin/flashos-accueil; sleep 1; done' >/dev/null 2>&1 &
sh -c 'while true; do /usr/local/bin/flashos-veille; sleep 2; done' >/dev/null 2>&1 &
/usr/local/bin/flashos-saisie >/dev/null 2>&1 &
/usr/local/bin/flashos-barre >/dev/null 2>&1 &
# Notepad Next, Impression, Caméra et KeePassXC installés d'office (une fois, en arrière-plan, quand Internet est là)
sh -c 'sleep 40; nice -n 10 /usr/local/bin/flashos-applis-defaut' >/dev/null 2>&1 &
FIN

#  Touches Fn (son, luminosité) : réglage + petite barre de progression à l'écran (wob)
mkdir -p "$I/etc/flashos"
cat > "$I/etc/flashos/wob.ini" <<'FIN'
# SoleOS : barre du son et de la luminosité (touches Fn)
timeout = 1500
max = 100
width = 320
height = 30
border_offset = 0
border_size = 2
bar_padding = 6
anchor = bottom center
margin = 90
background_color = 0E1426E6
border_color = FFC23DCC
bar_color = FFC23DFF
overflow_mode = nowrap
FIN
#  Diagnostic pour l'assistance : « flashos-diagnostic » dans la Console, résultat dans Documents
cat > "$I/usr/local/bin/flashos-diagnostic" <<'FIN'
#!/bin/bash
# SoleOS : relevé complet pour l'assistance (Wi-Fi, son, luminosité, navigateur, bureautique…).
# Aucun mot de passe ni contenu personnel : seulement des états et des réglages.
# iw, rfkill… sont dans /usr/sbin (absent du chemin d'un utilisateur ordinaire)
export PATH="$PATH:/usr/sbin:/sbin"
D="$(xdg-user-dir DOCUMENTS 2>/dev/null)"; [ -d "$D" ] || D="$HOME"
F="$D/diagnostic-flashos-$(date +%Y%m%d-%H%M).txt"
titre() { printf '\n===== %s =====\n' "$1"; }
cmd() { printf '$ %s\n' "$*"; timeout 20 "$@" 2>&1 | head -60; }
{
  echo "SoleOS $(cat /etc/flashos-version 2>/dev/null) — $(date '+%F %T') — $(cat /sys/class/dmi/id/sys_vendor /sys/class/dmi/id/product_name 2>/dev/null | paste -sd' ')"
  echo "Langue : $(cat /run/flashos/langue 2>/dev/null) — démarré depuis $(cut -d. -f1 /proc/uptime) s"

  titre "Session et fenêtres"
  cmd labwc --version
  cmd flashos-accueil --fenetres
  echo "rail : $(cat "$HOME/.config/flashos/rail" 2>/dev/null | paste -sd' ')"
  echo "-- fin du journal de la session"; tail -40 "$HOME/.flashos.log" 2>/dev/null

  titre "Wi-Fi"
  cmd nmcli general
  cmd nmcli radio
  cmd rfkill list
  cmd nmcli -f DEVICE,TYPE,STATE,CONNECTION device
  nmcli device wifi rescan >/dev/null 2>&1; sleep 4
  cmd nmcli -f IN-USE,SSID,CHAN,RATE,SIGNAL,SECURITY device wifi list
  for i in $(ls /sys/class/net); do [ -d "/sys/class/net/$i/wireless" ] && { cmd iw dev "$i" link; cmd iw dev "$i" get power_save; }; done
  cmd iw reg get

  titre "Son"
  cmd wpctl status
  cmd wpctl get-volume @DEFAULT_AUDIO_SINK@
  v="$(wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null | awk '{print $2}')"
  [ -n "$v" ] && { printf 'essai : volume remis à %s → ' "$v"; wpctl set-volume @DEFAULT_AUDIO_SINK@ "$v" && echo OK || echo ÉCHEC; }
  cmd cat /proc/asound/cards
  cmd aplay -l
  cmd lspci -nnk -d ::0403
  for c in /proc/asound/card*/codec#*; do [ -r "$c" ] && { echo "-- $c"; grep -E '^(Codec|Vendor Id|Subsystem Id)|^GPIO:' "$c"; }; done
  cmd sudo -n /usr/local/sbin/flashos-son etat
  cmd amixer -c "$(sudo -n /usr/local/sbin/flashos-son etat 2>/dev/null | sed -n 's/^carte=//p')" contents

  titre "Luminosité"
  cmd ls -l /sys/class/backlight/
  cmd brightnessctl -m
  b="$(brightnessctl -m 2>/dev/null | cut -d, -f3)"
  [ -n "$b" ] && { printf 'essai : luminosité remise à %s → ' "$b"; brightnessctl -q set "$b" && echo OK || echo ÉCHEC; }
  echo "groupes : $(id -nG)"

  titre "Barre du son et de la luminosité (touches)"
  cmd pgrep -a wob
  cmd ls -l "${XDG_RUNTIME_DIR:-/tmp}/flashos-barre"
  tail -5 "$HOME/.cache/flashos-barre.log" 2>/dev/null

  titre "Trousseau"
  cmd busctl --user call org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service ReadAlias s default
  cmd busctl --user get-property org.freedesktop.secrets /org/freedesktop/secrets/collection/login org.freedesktop.Secret.Collection Locked
  ls -l "$HOME/.local/share/keyrings/" 2>&1

  titre "Navigateur"
  cmd chromium --version
  grep -o 'password-store=[a-z-]*' /etc/chromium.d/flashos 2>/dev/null
  P="$HOME/.config/chromium/Default/Preferences"
  [ -f "$P" ] && grep -o '"cookie_controls_mode":[0-9]*\|"signin":{[^}]*}\|"restore_on_startup":[0-9]*' "$P" | head -5
  echo "règle : $(tr -d ' \n' < /etc/chromium/policies/managed/flashos.json 2>/dev/null)"
  grep -o '"os_crypt":{[^}]*}' "$HOME/.config/chromium/Local State" 2>/dev/null | sed 's/"encrypted_key":"[^"]*"/"encrypted_key":"…"/'
  for c in "$HOME/.config/chromium/Default/Cookies" "$HOME/.config/chromium/Default/Network/Cookies"; do
    [ -f "$c" ] || continue
    echo "-- $c"
    python3 - "$c" <<'PY' 2>&1
import sqlite3, sys, shutil, tempfile, time
t = tempfile.mktemp(); shutil.copy(sys.argv[1], t)
c = sqlite3.connect(t)
for nom, pers, pre, exp, maj in c.execute("select name,is_persistent,substr(encrypted_value,1,3),expires_utc,last_update_utc from cookies where host_key in ('.google.com','accounts.google.com','mail.google.com') and name in ('SID','__Secure-1PSID','__Secure-1PSIDTS','SAPISID','OSID','__Host-GAPS') order by name"):
    jours = (exp / 1e6 - 11644473600 - time.time()) / 86400 if exp else 0
    print(f"{nom:20} persistant={pers} chiffrement={pre} expire_dans={jours:.0f} j")
print("cookies au total :", c.execute("select count(*) from cookies").fetchone()[0])
PY
  done

  titre "Bureautique"
  grep -o 'ooSetupFactoryDefaultFilter[^<]*<value>[^<]*\|WindowAttributes[^<]*<value>[^<]*' "$HOME/.config/libreoffice/4/user/registrymodifications.xcu" 2>/dev/null | sed 's/.*<value>/  /'

  titre "Écran et mémoire"
  cat /sys/class/drm/card*-*/status 2>/dev/null | paste -sd' '
  head -1 /sys/class/drm/card*-*/modes 2>/dev/null | grep -v '^$' | head -3
  free -h | head -2
} > "$F" 2>&1
echo "Diagnostic enregistré : $F"
echo "Envoyez ce fichier à l'assistance (il ne contient aucun mot de passe)."
FIN
chmod 755 "$I/usr/local/bin/flashos-diagnostic"

#  Mesures de vitesse (assistance) : « flashos-mesure » dans la Console, résultat dans le dossier Documents
cat > "$I/usr/local/bin/flashos-mesure" <<'FIN'
#!/bin/bash
# SoleOS : relevé de vitesse de cet ordinateur (démarrage, mémoire, disque, Wi-Fi).
# Aucune donnée personnelle : seulement des temps et des caractéristiques du matériel.
D="$(xdg-user-dir DOCUMENTS 2>/dev/null)"; [ -d "$D" ] || D="$HOME"
F="$D/mesure-flashos-$(date +%Y%m%d-%H%M).txt"
{
  echo "== SoleOS $(cat /etc/flashos-version 2>/dev/null) — $(date '+%F %T')"
  echo "== Matériel"
  grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //'
  echo "Coeurs : $(nproc)"
  free -h | sed -n '1,3p'
  cat /sys/class/dmi/id/sys_vendor /sys/class/dmi/id/product_name 2>/dev/null | paste -sd' '
  lsblk -dno NAME,MODEL,SIZE,ROTA,TRAN 2>/dev/null | grep -v '^zram\|^loop'
  echo
  echo "== Démarrage"
  systemd-analyze 2>/dev/null
  echo "-- services les plus longs"
  systemd-analyze blame 2>/dev/null | head -15
  echo "-- chaîne critique"
  systemd-analyze critical-chain 2>/dev/null | head -20
  echo
  echo "== Mémoire et charge"
  uptime
  echo "zram : $(zramctl --noheadings -o NAME,DISKSIZE,DATA,COMPR,ALGORITHM 2>/dev/null | paste -sd' ')"
  echo "Écriture en mémoire (système en lecture seule) : $(df -h /run/flashos/rw 2>/dev/null | tail -1 | awk '{print $3" / "$2}')"
  echo "-- programmes qui occupent le plus de mémoire"
  ps -eo rss,comm --sort=-rss | head -12 | awk 'NR==1{print "Mo\tprogramme"; next} {printf "%d\t%s\n", $1/1024, $2}'
  echo
  echo "== Disque (écriture puis lecture de 512 Mo, données chiffrées)"
  f="$HOME/.cache/flashos-mesure.tmp"; mkdir -p "$HOME/.cache"
  dd if=/dev/zero of="$f" bs=4M count=128 oflag=direct conv=fdatasync 2>&1 | tail -1 | sed 's/^/écriture : /'
  dd if="$f" of=/dev/null bs=4M iflag=direct 2>&1 | tail -1 | sed 's/^/lecture  : /'
  rm -f "$f"
  echo
  echo "== Graphisme"
  for c in /sys/class/drm/card[0-9]; do [ -e "$c/device/driver" ] && echo "$(basename "$c") : $(basename "$(readlink -f "$c/device/driver")")"; done
  echo "Rendu de l'interface : ${GSK_RENDERER:-?}"
  echo
  echo "== Wi-Fi"
  for i in $(ls /sys/class/net 2>/dev/null); do
    [ -d "/sys/class/net/$i/wireless" ] || continue
    echo "Carte $i : $(basename "$(readlink -f /sys/class/net/$i/device/driver)" 2>/dev/null)"
    iw dev "$i" link 2>/dev/null | grep -E 'SSID|freq|signal|bitrate'
    iw dev "$i" get power_save 2>/dev/null
  done
  nmcli -f IN-USE,SSID,CHAN,RATE,SIGNAL,SECURITY dev wifi list 2>/dev/null | head -6
  echo
  echo "== Téléchargement d'essai (100 Mo)"
  curl -o /dev/null -s -w 'vitesse : %{speed_download} octets/s en %{time_total} s\n' -m 60 \
    https://speed.cloudflare.com/__down?bytes=100000000 2>/dev/null || echo "pas de connexion"
} > "$F" 2>&1
echo "Relevé enregistré : $F"
FIN
chmod 755 "$I/usr/local/bin/flashos-mesure"

cat > "$I/usr/local/bin/flashos-touche" <<'FIN'
#!/bin/sh
# flashos-touche son-plus|son-moins|son-coupe|lumiere-plus|lumiere-moins
F="${XDG_RUNTIME_DIR:-/tmp}/flashos-barre"
barre() {  # <pourcentage> : affiche la barre (le serveur est lancé à l'ouverture de la session)
  [ -p "$F" ] || return 0
  printf '%s\n' "$1" > "$F" &
}
# (calculs avec le point décimal : en français, awk lisait « 0.45 » comme 0 et la barre restait vide)
export LC_ALL=C
son() {
  v="$(wpctl get-volume @DEFAULT_AUDIO_SINK@ 2>/dev/null)"
  case "$v" in *MUTED*) echo 0 ;; *) echo "$v" | awk '{printf "%d", $2 * 100 + 0.5}' ;; esac
}
case "$1" in
  son-plus) wpctl set-mute @DEFAULT_AUDIO_SINK@ 0; wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+; barre "$(son)" ;;
  son-moins) wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-; barre "$(son)" ;;
  son-coupe) wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle; barre "$(son)" ;;
  lumiere-plus) brightnessctl -q set 5%+; barre "$(brightnessctl -m | cut -d, -f4 | tr -d %)" ;;
  lumiere-moins) brightnessctl -q -n5% set 5%-; barre "$(brightnessctl -m | cut -d, -f4 | tr -d %)" ;;
esac
FIN
chmod 755 "$I/usr/local/bin/flashos-touche"
cat > "$I/usr/local/bin/flashos-ajouter-bureau" <<'FIN'
#!/bin/bash
# SoleOS (v13.1) : Fichiers → clic droit → Scripts → « Ajouter au bureau ».
# Un raccourci sur le bureau pour chaque élément choisi ; le fichier ou le dossier ne bouge pas.
B="$(xdg-user-dir DESKTOP 2>/dev/null)"; { [ -n "$B" ] && [ "$B" != "$HOME" ]; } || B="$HOME/Bureau"
mkdir -p "$B"
libre() { local n="$1" t e i=2 c; c="$B/$n"
  case "$n" in *?.*) t="${n%.*}"; e=".${n##*.}" ;; *) t="$n"; e="" ;; esac
  while [ -e "$c" ] || [ -L "$c" ]; do c="$B/$t ($i)$e"; i=$((i+1)); done; printf '%s' "$c"; }
# Les adresses (URI) sont toujours données : fichier local → lien ; dossier réseau (smb://…,
# même ouvert) → lanceur vers l'adresse, qui reconnecte le NAS tout seul au double-clic
# (un lien vers le chemin « monté » ne marcherait plus une fois le NAS déconnecté).
while IFS= read -r u; do
  [ -n "$u" ] || continue
  case "$u" in
    file://*)
      p="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.unquote(sys.argv[1][7:]))' "$u")"
      [ -e "$p" ] || continue
      [ "$(dirname "$p")" = "$B" ] && continue
      deja=""; for l in "$B"/*; do [ -L "$l" ] && [ "$(readlink "$l")" = "$p" ] && deja=1; done
      [ -n "$deja" ] && continue
      ln -s "$p" "$(libre "$(basename "$p")")" ;;
    *)
      deja=""; for l in "$B"/*.desktop; do [ -f "$l" ] && grep -qxF "URL=$u" "$l" && deja=1; done
      [ -n "$deja" ] && continue
      n="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.unquote(sys.argv[1].rstrip("/").rsplit("/", 1)[-1]))' "$u")"
      [ -n "$n" ] || n="Réseau"
      printf '[Desktop Entry]\nType=Link\nName=%s\nURL=%s\nIcon=folder-remote\n' "$n" "$u" > "$(libre "$n.desktop")" ;;
  esac
done <<< "${NAUTILUS_SCRIPT_SELECTED_URIS:-}"
FIN
chmod 755 "$I/usr/local/bin/flashos-ajouter-bureau"
cat > "$I/usr/local/bin/flashos-capture" <<'FIN'
#!/bin/bash
# SoleOS : capture d'écran, enregistrée dans Images ET copiée (à coller dans un mail, un document…)
#   (rien)   : tout l'écran (Impr. écran)
#   zone     : une partie de l'écran (Maj + Impr. écran, ou ⊞ + Maj + S comme sous Windows)
#   edition  : une partie, puis le petit éditeur swappy : flèches, texte, surligneur, flou
#              (Ctrl + Maj + Impr. écran) ; « Enregistrer » remplace l'image dans Images
D="$(xdg-user-dir PICTURES 2>/dev/null)"; [ -d "$D" ] || D="$HOME"
F="$D/Capture $(date '+%Y-%m-%d %H-%M-%S').png"
case "${1:-}" in
  zone|edition)
    z="$(slurp -d 2>/dev/null)" || exit 0
    grim -g "$z" "$F" ;;
  *) grim "$F" ;;
esac
[ -f "$F" ] || exit 0
command -v wl-copy >/dev/null && wl-copy --type image/png < "$F" 2>/dev/null &
if [ "${1:-}" = edition ] && command -v swappy >/dev/null; then
  exec swappy -f "$F" -o "$F"
fi
# Confirmation à l'écran (« Capture enregistrée dans Images »)
if [ -n "${XDG_RUNTIME_DIR:-}" ]; then
  printf 'capture' > "$XDG_RUNTIME_DIR/flashos-ouvrir"; /usr/local/bin/flashos-accueil >/dev/null 2>&1 &
fi
FIN
chmod 755 "$I/usr/local/bin/flashos-capture"
# apt dans la Console : programmes Debian installés dans la « boîte Debian » (Distrobox), rangée dans
# le dossier personnel. Ils survivent aux mises à jour et ne peuvent pas abîmer le système (en lecture
# seule) ; leurs applications apparaissent dans « Mes applications » et dans l'éclair.
cat > "$I/usr/local/bin/apt" <<'FIN'
#!/bin/bash
C=flashos-debian
outil="$(basename "$0")"
# Administrateur (construction, réparation) : le vrai apt du système
[ "$(id -u)" = 0 ] && exec "/usr/bin/$outil" "$@"
if ! command -v distrobox >/dev/null; then echo "Distrobox manque : $outil indisponible." >&2; exit 1; fi
if ! distrobox list --no-color 2>/dev/null | awk -F'|' '{gsub(/ /,"",$2); print $2}' | grep -qx "$C"; then
  # (v13.2) messages clairs : la création se fait en deux étapes, la seconde est silencieuse
  echo "Boîte Debian : préparation (une seule fois, connexion Internet nécessaire)."
  echo "Étape 1/2 : téléchargement de Debian…"
  if ! distrobox create --yes --name "$C" --image docker.io/library/debian:trixie >/tmp/flashos-boite-$$.log 2>&1; then
    tail -n 5 /tmp/flashos-boite-$$.log >&2; rm -f /tmp/flashos-boite-$$.log
    echo "La boîte Debian n'a pas pu être créée (Internet ?). Réessayez plus tard." >&2; exit 1
  fi
  rm -f /tmp/flashos-boite-$$.log
  echo "Étape 2/2 : première mise en route (2 à 5 minutes, sans affichage : c'est normal)…"
  if ! distrobox enter "$C" -- true >/dev/null 2>&1; then
    echo "La boîte Debian n'a pas démarré. Réessayez." >&2; exit 1
  fi
  echo "Boîte Debian prête."
fi
avant="$(distrobox enter "$C" -- sh -c 'ls /usr/share/applications 2>/dev/null' 2>/dev/null)"
case "${1:-}" in
  install|remove|purge|reinstall|upgrade|full-upgrade|dist-upgrade|autoremove|update)
    [ "$1" = install ] && distrobox enter "$C" -- sudo apt-get update -q >/dev/null 2>&1
    distrobox enter "$C" -- sudo env DEBIAN_FRONTEND=readline "$outil" "$@" ;;
  *) distrobox enter "$C" -- "$outil" "$@" ;;
esac
r=$?
# Nouvelles applications : dans « Mes applications » ; applications retirées : retirées
apres="$(distrobox enter "$C" -- sh -c 'ls /usr/share/applications 2>/dev/null' 2>/dev/null)"
for f in $(comm -13 <(echo "$avant" | sort) <(echo "$apres" | sort) | grep '\.desktop$'); do
  grep -q '^NoDisplay=true' <(distrobox enter "$C" -- cat "/usr/share/applications/$f" 2>/dev/null) && continue
  distrobox enter "$C" -- distrobox-export --app "${f%.desktop}" >/dev/null 2>&1 && echo "→ ${f%.desktop} ajouté à « Mes applications »"
done
for f in $(comm -23 <(echo "$avant" | sort) <(echo "$apres" | sort) | grep '\.desktop$'); do
  rm -f "$HOME/.local/share/applications/$C-$f"
done
exit $r
FIN
chmod 755 "$I/usr/local/bin/apt"
ln -sf apt "$I/usr/local/bin/apt-get"
# Applications installées d'office à l'ouverture de la session, dès qu'Internet est là (une fois) :
# Notepad Next (éditeur de texte façon Notepad++), Impression (créer une clé USB, comme Rufus),
# Caméra (GNOME Snapshot : essai de la webcam, Réglages → Caméra) et KeePassXC (mots de passe, v1.0.3)
cat > "$I/usr/local/bin/flashos-applis-defaut" <<'FIN'
#!/bin/bash
M="$HOME/.config/flashos/applis-defaut"
for id in com.github.dail8859.NotepadNext io.gitlab.adhami3310.Impression org.gnome.Snapshot org.keepassxc.KeePassXC; do
  grep -qx "$id" "$M" 2>/dev/null && continue
  if flatpak --user info "$id" >/dev/null 2>&1 || /usr/local/bin/flashos-applications --installer "$id" >/dev/null 2>&1; then
    mkdir -p "$(dirname "$M")"; echo "$id" >> "$M"
  fi
done
FIN
chmod 755 "$I/usr/local/bin/flashos-applis-defaut"
cat > "$I/usr/local/bin/flashos-barre" <<'FIN'
#!/bin/sh
# Serveur de la barre de progression : un tube nommé lu par wob (relancé s'il s'arrête)
F="${XDG_RUNTIME_DIR:-/tmp}/flashos-barre"
rm -f "$F"; mkfifo -m 600 "$F"
# (le tube est rouvert après chaque écriture : pas de « tail -f », peu fiable sur un tube
# selon les versions ; les erreurs de wob sont gardées pour l'assistance)
J="${XDG_CACHE_HOME:-$HOME/.cache}/flashos-barre.log"; mkdir -p "$(dirname "$J")"
while true; do
  while true; do cat "$F"; done | wob -c /etc/flashos/wob.ini 2>>"$J"
  sleep 1
done
FIN
chmod 755 "$I/usr/local/bin/flashos-barre"

#  Bureautique plus rapide à ouvrir : ses fichiers sont lus en mémoire cache peu après l'ouverture
#  de la session, en douceur (priorité la plus basse). Ce cache est rendu dès qu'un programme a
#  besoin de mémoire : aucune mémoire n'est « réservée ».
#  Écriture du chinois : fcitx5 (pinyin) démarré seulement si le chinois est choisi.
#  Ctrl+Espace passe du pinyin aux lettres latines (et inversement).
cat > "$I/usr/local/bin/flashos-saisie" <<'FIN'
#!/bin/sh
[ "$(cat /run/flashos/langue 2>/dev/null)" = zh ] || exit 0
command -v fcitx5 >/dev/null 2>&1 || exit 0
p="${XDG_CONFIG_HOME:-$HOME/.config}/fcitx5"
if [ ! -f "$p/profile" ]; then
  mkdir -p "$p"
  cat > "$p/profile" <<'PROFIL'
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=pinyin

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=pinyin
Layout=

[GroupOrder]
0=Default
PROFIL
fi
exec fcitx5 --replace
FIN
chmod 755 "$I/usr/local/bin/flashos-saisie"


cat > "$I/etc/xdg/foot/foot.ini" <<'FIN'
[main]
font=DejaVu Sans Mono:size=13
pad=14x12

[colors]
background=161e36
foreground=e8ecf5
selection-background=3b5ba5
selection-foreground=ffffff

[key-bindings]
# Copier / coller : Ctrl+Maj+C / Ctrl+Maj+V (Ctrl+C arrête une commande), aussi Maj+Inser
clipboard-copy=Control+Shift+c XF86Copy
clipboard-paste=Control+Shift+v XF86Paste Shift+Insert
primary-paste=none
FIN
#  Console : rappel des raccourcis à l'ouverture
cat > "$I/usr/local/bin/flashos-console" <<'FIN'
#!/bin/sh
# Console SoleOS : reprend la taille (colonnes x lignes) laissée par l'utilisateur la dernière fois
T="${XDG_CONFIG_HOME:-$HOME/.config}/flashos/console-taille"
taille=""
if [ -r "$T" ]; then
  t="$(head -n1 "$T")"
  case "$t" in
    [0-9]*x[0-9]*)
      c="${t%x*}"; l="${t#*x}"
      if [ "$c" -ge 40 ] 2>/dev/null && [ "$c" -le 400 ] && [ "$l" -ge 10 ] 2>/dev/null && [ "$l" -le 200 ]; then
        taille="--window-size-chars=${c}x${l}"
      fi ;;
  esac
fi
case "$LANG" in
  en*) aide="Copy: Ctrl+Shift+C  ·  Paste: Ctrl+Shift+V or Shift+Insert"; titre="Console" ;;
  it*) aide="Copia: Ctrl+Maiusc+C  ·  Incolla: Ctrl+Maiusc+V o Maiusc+Ins"; titre="Console" ;;
  es*) aide="Copiar: Ctrl+Mayús+C  ·  Pegar: Ctrl+Mayús+V o Mayús+Insert"; titre="Consola" ;;
  de*) aide="Kopieren: Strg+Umschalt+C  ·  Einfügen: Strg+Umschalt+V oder Umschalt+Einfg"; titre="Konsole" ;;
  zh*) aide="复制：Ctrl+Shift+C  ·  粘贴：Ctrl+Shift+V 或 Shift+Insert"; titre="控制台" ;;
  *) aide="Copier : Ctrl+Maj+C  ·  Coller : Ctrl+Maj+V ou Maj+Inser"; titre="Console" ;;
esac
export FLASHOS_AIDE_CONSOLE="$aide"
# (v13.1) titre « Console » (et non « foot ») dans la barre de titre ; il ne change plus ensuite
exec foot $taille --title="$titre" -o locked-title=yes sh -c 'printf "\033[38;5;215m%s\033[0m\n\n" "$FLASHOS_AIDE_CONSOLE"; exec bash --rcfile /usr/share/flashos/console-bashrc'
FIN
cat > "$I/usr/share/flashos/console-bashrc" <<'FIN'
# Console SoleOS : réglages habituels de l'utilisateur, puis mémorisation de la taille de la fenêtre
[ -r /etc/bash.bashrc ] && . /etc/bash.bashrc
[ -r "$HOME/.bashrc" ] && . "$HOME/.bashrc"
__flashos_taille() {
  set -- $(stty size 2>/dev/null)
  [ -n "$2" ] || return 0
  _d="${XDG_CONFIG_HOME:-$HOME/.config}/flashos"
  mkdir -p "$_d" 2>/dev/null && printf '%sx%s\n' "$2" "$1" > "$_d/console-taille" 2>/dev/null
  return 0
}
trap __flashos_taille WINCH
PROMPT_COMMAND="__flashos_taille${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
# (v13.1) « sudo » tapé en suivant un tutoriel (clé, dépôt, sudo apt install…) : la commande
# s'exécute dans la boîte Debian, où l'utilisateur est administrateur sans mot de passe. Le système
# SoleOS (en lecture seule) n'est jamais touché ; les applications installées vont dans
# « Mes applications ». Les quelques commandes SoleOS autorisées restent sur le système.
__FLASHOS_BOITE=flashos-debian
sudo() {
  case "${1:-}" in
    -n|-l|-v|-k|-h|--help|-V|--version) command sudo "$@"; return ;;
  esac
  if command sudo -n -l "$@" >/dev/null 2>&1; then command sudo "$@"; return; fi
  # sudo apt / apt-get : l'apt de SoleOS (boîte Debian + raccourcis dans « Mes applications »)
  case "${1:-}" in
    apt|apt-get) local o="$1"; shift; command "$o" "$@"; return ;;
  esac
  if ! command -v distrobox >/dev/null; then
    printf '\033[38;5;215m%s\033[0m\n' "sudo : boîte Debian indisponible." >&2; return 1
  fi
  # Boîte pas encore créée : apt la prépare (une seule fois)
  if ! distrobox list --no-color 2>/dev/null | awk -F'|' '{gsub(/ /,"",$2); print $2}' | grep -qx "$__FLASHOS_BOITE"; then
    command apt --version >/dev/null || return 1
  fi
  [ -t 2 ] && printf '\033[2m(boîte Debian)\033[0m\n' >&2
  distrobox enter "$__FLASHOS_BOITE" -- sudo "$@"
}
# « debian » : ouvrir un terminal dans la boîte Debian (avancé)
debian() { distrobox enter "$__FLASHOS_BOITE" "$@"; }
FIN
chmod 755 "$I/usr/local/bin/flashos-console"

#  Fichiers AppImage : ouverts depuis Fichiers, ils s'ajoutent à « Applications »
mkdir -p "$I/usr/share/applications"
cat > "$I/usr/share/applications/flashos-appimage.desktop" <<'FIN'
[Desktop Entry]
Type=Application
Name=Ajouter à mes applications
Name[en]=Add to my apps
Name[it]=Aggiungi alle mie app
Name[es]=Añadir a mis aplicaciones
Name[de]=Zu meinen Apps hinzufügen
Name[zh_CN]=添加到我的应用
Exec=/usr/local/bin/flashos-applications --ajouter-appimage %f
MimeType=application/vnd.appimage;application/x-iso9660-appimage;
NoDisplay=true
FIN
#  Paquets .deb : installés dans un espace Debian séparé (conteneur), jamais dans le système
cat > "$I/usr/share/applications/flashos-deb.desktop" <<'FIN'
[Desktop Entry]
Type=Application
Name=Installer le paquet
Name[en]=Install package
Name[it]=Installa il pacchetto
Name[es]=Instalar el paquete
Name[de]=Paket installieren
Name[zh_CN]=安装软件包
Exec=/usr/local/bin/flashos-applications --deb %f
MimeType=application/vnd.debian.binary-package;application/x-deb;
NoDisplay=true
FIN
cat > "$I/etc/xdg/mimeapps.list" <<'FIN'
[Default Applications]
application/vnd.debian.binary-package=flashos-deb.desktop
application/x-deb=flashos-deb.desktop
application/vnd.appimage=flashos-appimage.desktop
application/x-iso9660-appimage=flashos-appimage.desktop
inode/directory=org.gnome.Nautilus.desktop
image/jpeg=org.gnome.Loupe.desktop
image/png=org.gnome.Loupe.desktop
image/gif=org.gnome.Loupe.desktop
image/webp=org.gnome.Loupe.desktop
image/bmp=org.gnome.Loupe.desktop
image/tiff=org.gnome.Loupe.desktop
image/heif=org.gnome.Loupe.desktop
image/avif=org.gnome.Loupe.desktop
image/svg+xml=org.gnome.Loupe.desktop
image/x-icon=org.gnome.Loupe.desktop
application/pdf=chromium.desktop
application/vnd.openxmlformats-officedocument.wordprocessingml.document=libreoffice-writer.desktop
application/msword=libreoffice-writer.desktop
application/vnd.oasis.opendocument.text=libreoffice-writer.desktop
application/rtf=libreoffice-writer.desktop
text/rtf=libreoffice-writer.desktop
application/vnd.openxmlformats-officedocument.spreadsheetml.sheet=libreoffice-calc.desktop
application/vnd.ms-excel=libreoffice-calc.desktop
application/vnd.oasis.opendocument.spreadsheet=libreoffice-calc.desktop
text/csv=libreoffice-calc.desktop
application/vnd.openxmlformats-officedocument.presentationml.presentation=libreoffice-impress.desktop
application/vnd.ms-powerpoint=libreoffice-impress.desktop
application/vnd.oasis.opendocument.presentation=libreoffice-impress.desktop
video/mp4=vlc.desktop
video/x-matroska=vlc.desktop
video/webm=vlc.desktop
video/x-msvideo=vlc.desktop
video/quicktime=vlc.desktop
video/mpeg=vlc.desktop
video/ogg=vlc.desktop
video/3gpp=vlc.desktop
video/x-flv=vlc.desktop
video/x-ms-wmv=vlc.desktop
audio/mpeg=vlc.desktop
audio/mp4=vlc.desktop
audio/x-m4a=vlc.desktop
audio/flac=vlc.desktop
audio/ogg=vlc.desktop
audio/x-wav=vlc.desktop
audio/wav=vlc.desktop
audio/aac=vlc.desktop
audio/x-ms-wma=vlc.desktop
audio/opus=vlc.desktop
audio/webm=vlc.desktop
application/ogg=vlc.desktop
application/x-matroska=vlc.desktop
FIN

#  LibreOffice : présentation à onglets (moderne), pas d'astuce au démarrage, enregistrement
#  aux formats Word / Excel / PowerPoint (ceux qu'on échange le plus) sans avertissement
mkdir -p "$I/etc/libreoffice/registry"
cat > "$I/etc/libreoffice/registry/flashos.xcd" <<'FIN'
<?xml version="1.0" encoding="UTF-8"?>
<oor:data xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:oor="http://openoffice.org/2001/registry">
<dependency file="main"/>
<dependency file="writer"/>
<dependency file="calc"/>
<dependency file="impress"/>
<oor:component-data oor:package="org.openoffice.Office" oor:name="Common">
 <node oor:name="Misc">
  <prop oor:name="ShowTipOfTheDay" oor:op="fuse"><value>false</value></prop>
  <prop oor:name="SymbolStyle" oor:op="fuse"><value>colibre</value></prop>
 </node>
 <node oor:name="Save"><node oor:name="Document"><prop oor:name="WarnAlienFormat" oor:op="fuse"><value>false</value></prop></node></node>
</oor:component-data>
<oor:component-data oor:package="org.openoffice.Office.UI" oor:name="ToolbarMode">
 <node oor:name="Applications">
  <node oor:name="Writer"><prop oor:name="Active" oor:op="fuse"><value>notebookbar.ui</value></prop></node>
  <node oor:name="Calc"><prop oor:name="Active" oor:op="fuse"><value>notebookbar.ui</value></prop></node>
  <node oor:name="Impress"><prop oor:name="Active" oor:op="fuse"><value>notebookbar.ui</value></prop></node>
 </node>
</oor:component-data>
<oor:component-data oor:package="org.openoffice.Office" oor:name="Impress">
 <node oor:name="Misc"><node oor:name="NewDoc"><prop oor:name="AutoPilot" oor:op="fuse"><value>false</value></prop></node></node>
</oor:component-data>
<oor:component-data oor:package="org.openoffice" oor:name="Setup">
 <node oor:name="Product"><prop oor:name="ooSetupLastVersion" oor:op="fuse"><value>99.0</value></prop></node>
 <node oor:name="Office"><node oor:name="Factories">
  <node oor:name="com.sun.star.text.TextDocument"><prop oor:name="ooSetupFactoryDefaultFilter" oor:op="fuse"><value>MS Word 2007 XML</value></prop></node>
  <node oor:name="com.sun.star.sheet.SpreadsheetDocument"><prop oor:name="ooSetupFactoryDefaultFilter" oor:op="fuse"><value>Calc MS Excel 2007 XML</value></prop></node>
  <node oor:name="com.sun.star.presentation.PresentationDocument"><prop oor:name="ooSetupFactoryDefaultFilter" oor:op="fuse"><value>Impress MS PowerPoint 2007 XML</value></prop></node>
 </node></node>
</oor:component-data>
</oor:data>
FIN

#  Apparence commune à toutes les applications : thème clair (le bureau SoleOS reste sombre), accent bleu, icônes,
#  curseur et police (applications GTK 4/libadwaita via le portail, GTK 3 via settings.ini)
mkdir -p "$I/usr/share/glib-2.0/schemas"
cat > "$I/usr/share/glib-2.0/schemas/90_flashos.gschema.override" <<'FIN'
[org.gnome.desktop.interface]
color-scheme='default'
accent-color='blue'
gtk-theme='Adwaita'
icon-theme='Papirus'
cursor-theme='Adwaita'
cursor-size=24
font-name='Inter 11'
document-font-name='Inter 11'
monospace-font-name='DejaVu Sans Mono 11'
clock-format='24h'
enable-hot-corners=false

[org.gnome.desktop.wm.preferences]
button-layout='appmenu:minimize,maximize,close'

[org.gnome.nautilus.preferences]
default-folder-viewer='icon-view'
open-folder-on-dnd-hover=true
recursive-search='always'
show-image-thumbnails='always'

[org.gtk.gtk4.Settings.FileChooser]
sort-directories-first=true
view-type='grid'
FIN
mkdir -p "$I/etc/gtk-3.0" "$I/etc/gtk-4.0"
printf '[Settings]\ngtk-application-prefer-dark-theme=0\ngtk-icon-theme-name=Papirus\ngtk-cursor-theme-name=Adwaita\ngtk-cursor-theme-size=24\ngtk-font-name=Inter 11\n' > "$I/etc/gtk-3.0/settings.ini"

#  Navigateur : barre de titre SoleOS (− □ ✕) au lieu de celle de Chromium
mkdir -p "$I/usr/lib/chromium"
#  (réglages appliqués au premier lancement ; les écrans d'accueil de Chromium sont masqués)
cat > "$I/usr/lib/chromium/initial_preferences" <<'FIN'
{
  "browser": { "custom_chrome_frame": false, "check_default_browser": false, "show_home_button": true },
  "homepage": "https://www.google.fr/",
  "homepage_is_newtabpage": false,
  "session": { "restore_on_startup": 4, "startup_urls": ["https://www.google.fr/"] },
  "bookmark_bar": { "show_on_all_tabs": true },
  "performance_tuning": { "high_efficiency_mode": { "state": 2 }, "battery_saver_mode": { "state": 2 } },
  "background_mode": { "enabled": false },
  "profile": { "cookie_controls_mode": 0, "block_third_party_cookies": false },
  "distribution": {
    "skip_first_run_ui": true,
    "suppress_first_run_default_browser_prompt": true,
    "suppress_first_run_bubble": true,
    "import_bookmarks": false, "import_history": false, "import_search_engine": false,
    "import_bookmarks_from_file": "/usr/share/flashos/favoris.html",
    "make_chrome_default": false, "make_chrome_default_for_user": false
  },
  "sync_promo": { "show_on_first_run_allowed": false }
}
FIN
cp "$I/usr/lib/chromium/initial_preferences" "$I/etc/chromium/master_preferences"
cp "$I/usr/lib/chromium/initial_preferences" "$I/etc/chromium/initial_preferences"
# Variantes par langue (page d'accueil et favoris) : copiées à la place de celles-ci
# par « flashos-systeme langue » (nouveaux profils du navigateur)
mkdir -p "$I/usr/share/flashos/navigateur"
for v in "fr|https://www.google.fr/|Google|https://www.google.fr/maps|Maps|https://www.youtube.com/|YouTube" \
         "en|https://www.google.com/|Google|https://www.google.com/maps|Maps|https://www.youtube.com/|YouTube" \
         "it|https://www.google.it/|Google|https://www.google.it/maps|Maps|https://www.youtube.com/|YouTube" \
         "es|https://www.google.es/|Google|https://www.google.es/maps|Maps|https://www.youtube.com/|YouTube" \
         "de|https://www.google.de/|Google|https://www.google.de/maps|Maps|https://www.youtube.com/|YouTube" \
         "zh|https://www.bing.com/|Bing|https://www.baidu.com/|百度|https://www.bilibili.com/|哔哩哔哩"; do
  IFS='|' read -r code web n1 u2 n2 u3 n3 <<< "$v"
  sed -e "s#https://www.google.fr/#$web#g" \
      -e "s#/usr/share/flashos/favoris.html#/usr/share/flashos/navigateur/favoris.$code.html#" \
      "$I/usr/lib/chromium/initial_preferences" > "$I/usr/share/flashos/navigateur/initial_preferences.$code"
  cat > "$I/usr/share/flashos/navigateur/favoris.$code.html" <<FAV
<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
  <DT><H3 PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
  <DL><p>
    <DT><A HREF="$web">$n1</A>
    <DT><A HREF="$u2">$n2</A>
    <DT><A HREF="$u3">$n3</A>
  </DL><p>
</DL><p>
FAV
done

#  Favoris SoleOS proposés au premier lancement (l'utilisateur peut les modifier :
#  pas de règle imposée, donc pas de message « géré par votre organisation »)
mkdir -p "$I/usr/share/flashos"
cat > "$I/usr/share/flashos/favoris.html" <<'FIN'
<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Favoris</TITLE>
<H1>Favoris</H1>
<DL><p>
  <DT><H3 PERSONAL_TOOLBAR_FOLDER="true">Barre de favoris</H3>
  <DL><p>
    <DT><A HREF="https://www.google.fr/">Google</A>
    <DT><A HREF="https://www.google.fr/maps">Maps</A>
    <DT><A HREF="https://www.youtube.com/">YouTube</A>
  </DL><p>
</DL><p>
FIN

# --- 5b. Image de marque : logo et démarrage sans menu -------------------
IMG="$C/flashos-images"
mkdir -p "$IMG" "$I/usr/share/plymouth/themes/flashos"
base64 -d > "$IMG/logo.png" <<'FIN'
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
FIN
base64 -d > "$IMG/splash-640x480.png" <<'FIN'
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
oDRVO4phsnSz7+pn3T+fZTrPVw5oPfRKK71+Wk839EIrX273zRildKa1VibPVyh1RQCrLt0WpHBW
vCtcwcETwODfc0H6vMrJOF/02rYNGDhvMWKVEsQl4JqxDyLcmrUMjtKGoR7xBWrfbLVwX/gOUq8x
SmvdSxorpdWu7dOP25Wdd3K+c2u+fdbMNrMjS3rfgrr3sLrhPnX9HvPdu8zBBWOUNlo1tMpzY3qG
Czd/e1J49WsaFTtw7Dws7atYwyXF5VcspmCNTbFXGTBJYwgAQ8COVUJuEUmEzUKFwe/H1yKgnqbO
teE08oj2HRC+fbrVWj9q18b9R9u33LeolD7vtNmXPmbjhWccPXHTsjGq1VF5xyjT0Zt1I9ONRqYz
vdzK9xw019ymL/tu/uUbOu1cdWm4k5te1rnHuys0aVZy0SvBdDmYvrwUOTj62A6Yix7HhRnUpnTU
f7hLkPDFTig0rTdvf1D3X4DC7KdHjpXbAC5SIyno4XpOO7rik7J8aQZYt7Jadd2q6iUFYBZBLgCR
aMfDz7r8Y/FaFa5WgX0zpYrs2xW+WW6MVvpRp2++6OFbvrb7yGe/u3/9dOPnn3jcRQ+bOXPzvZ3W
0bbRSplsVTEr08ska2Wmmo3ZmUY7N9+9q/OBb+iPfrvT7qhMq9zkppvCNsqsPKNl+o0yq68rMP1j
g+3yyF9Fp95RBFmWAE7KirDE5jJiPV9VERRtuQFIo8zwv0z5WWd1y4Hqd3GYik+u/vKsW+G62iLI
jvUYOj5HEWC2IAwBW85NCBhCl0j6jEPAGPZVhSvkYN+eCF55l6Q5advs888/4YkP3vGvn7/xf76z
b+vc1G89+/RHntQ8qfmjdc0lpZsNrYwxeY9D9YpCNw2tM6VyozrKKKVmZ5rrp5vX3Np58+fzy2/u
GGUypfIe6Rpl4+DiMuDNwWmybx/CNAwzD2QwZvdYnrWeSIeAR8pMCLj8GOwQ/DyGgGHLwZgTsDOY
ZAm4pLoMAY+WMT0K9SDggVu/BfbVXcnbv+mrte7mnJ/xiBN+4qHH7Ng6d9k393z0a/dsmp361aef
ccax02fM7T5h43w715nK8zxfP53NzjSN0rkxWaYypTq5WW7li61Ox6im1plWbaM6Rm2ayZTO3ne1
+t+faS21VKZNx6xQ7zAHrzwvrQorwWiTIXNOj/wjQcTn4DgimI2ARw5LETDESJoEXFXd5rpOBAw4
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
qgbbY3c1/+ni2cZkqZsgPUxS0BOExsp3f/r/7aO92G63dTZnTDtXDa1mTGNDoaBZufWrB40ZrVSu
Vl4qqRYO33/H4fvvmV6349gTTzpux85jtmaN/Spf6mSNI+11dx7Z2mpnJ83tX9/M797fftX5+n9+
0LnmzkamTWflvRxmdirTurnU6jQyNdXQR5bzfUc7vZ9hMIOqE9rkW2+755/f/ZF/ftdH1q+fOuP0
nU+78Amv/LmXbNq4cajYL77q5R+95JM37r63ePBBg3d2P/O5y42xOjbGfOZzlxdvBp911mm2wqU4
88yBX0V83W+/+nW//Wpo3dNP+da3v4dyR8H8kvnwta0PX9tqNtTDdzbOO7XxuF3Nx+5qzoysauef
1nj5Y6ffd9V4PBQ9wfhgsi2cIAUYpZTpLOatpbZqtnLdznVbrdPTWwvnu//pvpPKDLKfUcp00f3c
Wl68c8/uK6+75rPX3Xntfdv2dnZ0f1yplTcfWJzbM79lOZ9udXS7o379CctG5ab3JSNl1LaN0zqb
WWrr5bzRMVOHlhqHFkzB2yj5gXlYq4XF1ne/f+s/vO1Djzr/oguf+cKrvvaNoSL/649+Z+jI5s0D
PL179+3VTnbfckfx45bNm6DhKaWU2rpls7uQrW7h0S37JgEBoJF2R33z9s67vrr8i+8/+ti/PPz/
/NfCHfuHkxY///jh32WaYILomCjgLkbU2ATMcPawMSrXrf1Kndrp6Fw3dEfpmeNVUfkO2TDGaF04
arovlNbdV0YbleWtw3tv+Obee75z3HlnnXzBozddf/amezOdHW7N3j3fOXlDa//R5ceeqh+03dxw
v85WfiZJ7dy2vqWml82S6ZjpZuO+I+39873vFAH44Oor/qu7G+gW/omLfm7/geEbqEoppfUdex54
xS+8/o//8Nde8fLVLw0//nHna9UxqvBt15kB5jg8P18dwOEjA+7WrcP9HP2QOxSKvg4tms2Fd3HM
wqwO/dZv8e1X179pQ/HUb/zHwlduLHlGb37JfOTa1n9/t/2en5s975SB33LYuSXbc2DyOFY6mDyg
PlHAY45Uh3hpXEblR+9WmVZaGa3brXa+7niVTSmTF59SVl1yMwVL/bc6r0hhZfIVQWy0apiD+b1f
uP6GH3zglid++NZH7u9sbGadQ8szDyzOtfPm+pnGE09rd78D3P25w1O2zR1t6ZZpLnUaSk/d9kB7
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
FIN
base64 -d > "$IMG/splash-1920x1080.png" <<'FIN'
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
T7nwguM/+7+/7wmPf1Sg9ldzw6Mf/sOv+r/8us9TRqPRq/7HdzzusY9c+8rVWxufn3g2Yc7trDag
1xz+x+/cadPLJ10z+uWXHvmzbz72is/cetSVo3UbkpP0cIgIIYSQVNCAJoQQcgDXZqSByFNjUHfj
jQuqylXGOWecG1XOGldUzpbOOruxvX12Z+eMMbOVxY3d7frA0/pla6p7/qW65583ti5yrjTGVZWt
qpG1hbH29jPH33HPlR84ccmpnU1rXWVM5YrKTD+Z0JaVLZ0tXTF1gZ0tXGGNsafO7dx76tyxjeqL
Hrf5w180+q/PLp509W5ottj7jMH5zypcuV30wT8VhbXWfOze0tjxpDLOGees2w/DmHOlueTY+IoL
Gj4JsQ+NLfzjP73t/vtPtWzl0z/t6b/+6p96zzv+4o//4Ode+u+e98iHX31oaxS6SOybv/GlL/qy
fxu0iyNHDv/CT//AtQ9+UNBelpl63+H2od7a2vzJH//eiy68IFD7IVjrFP/Gv+ycb13W/JDLim/6
rK3Xvfzo3//n4z/+wsMvfcbmE68ZbQ59ypR/jYkGOI8JIYTswy04CCGEEIJM3ccD7v+4vGNGsVGN
D5WlM25UFtaZyrmisK5whXOuGI+3z5+ryrKmpri+0wOcs8aa8hN/ae1odNETq+37nC1KU0ycKZwz
Rbldjj96+uK7zh+54vDJBx49ednW2aIoK2PKylhri6JwpiqtHZmRc6Y0rnDOmsKMzHbl7js92RhV
n/Ww8Wc+vPinm3de/S/mbbdaZ8zImrLRe216eNca46xzxtiP3mucGTlTlJWx1hXWFq5wpjKucM4d
2yguPLJpzPm5Y63/PTnKsvyWb/uvP//Tr+p01CMf8bD/+798y/T7kydP/87vve7P3vBX//qem06e
2vbrajztKY//xq//Cp8tNnDRRRf80Kte+aUv/sayXL/LsC++4z9/3ZVXXh60i6uufMC3fevXvPK7
/1fQXmJy64nql//h/Nd+2lanoy47Zp/9mI1nP2bDGLM9Me+4pfznmyf/9OHyzTdPdrixMyGEEELy
hgY0IYQQQuRR40BOf1UccsVm5ZwztqiMK2y1++F7zjpnzfjcuZPGLH+k376TvcKSdsZZZ9zO7W8c
u53RxU+dTE5VVVnZUemqSVXZwmxYN3Gj289deGLn6CVbZ688dP+lm6ePbZy1pqqMLc3IOlvaSeFs
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
73zfX77jPS0HXHnFmfYK5ZE87WlPavntW9/27r7lz3Puu++Be+69v63fp9w0oFlFzGr5vb/a+99/
+vxLv/uRf/MbF//4/dOLewGaPXPSvfJrTt9wJZdmhBBCCLHJTu4ACCGEEEKCMaun9XRP3ERk5ryr
XDVz4sRVfjb1rvLOVyf2D/Vrn9q3/pOuv/YiznvvxM0POf/InecfufOek5ddffVjr732uivPnJlU
la/PT2fTiVTOTaZ1PXF+Jm4iVT2ZeO9dVc/k2L2XzlyYHbv+1NlrTlzw4qa1c5WbeXG+OjaZ3Ht2
9vKn7XzjS3f/y58dr5zzvtl3Poz09HFxzu16V9WVOC/ia6knUlUitfipdw+fv9RP32h4kQ9++I4f
/JGf/8Ef+bkzl5985jOe8jmf9Slf8iWff9WVVwxr8Dkf/6z/659//Xf/l1dvOuDjnv7k9hZe/4a3
Det6zhve+JfPf96zWg74uKc/6Y477x7TRQvtRvBnfvpLb3nvn8To98lPfkKMZgG5/5z/hbfs/sJb
dk8ekxc8fuclT5q85Ik7z3ns5OTQjxW87oz7ri89+bU/eT5omIQQQgghENCAJoQQQkhQtti4kTuf
Xqp399yJ43VdO+fEV66a1ydXrq587WRySkS8a6149iJOvHgn7iifZsN66adevPj5BxxWzvndi+fu
uuPmu+/8wBVXPerG657w6KuuOn3ycpk9Us9mM6mmEz8RX0s9827qK+erSmTi/G59/M7zV1ycTR5z
6uypyWzmq8pVtfezutqZ+IcfvviKlx7/g/fX77qzcpVs3MTZiXi5/Jivaz+rRaqJ+FqkdnMTXpz3
9e7M3fPw7jYhVv6RAPfI2Ut/8Zb3/MVb3vMd/+FHjh+vnnjT9S976Yv/3t/98ptu6re/8De84h+8
+qd/5a57zzX+9nGPvb7l3A/fetvd99zXq7sV3vyWd8g3fHXLAY9/3A1j2m8nanl1C1GTwuTinrz+
g9PXf3AqcmlSyTOun7zwpslLbpr8jSfvXHmq363wk56884XPPfY/3hmipno8KS96QgghhFiH7/Mi
hBBCiD4avJF5NfDswlR2az+pvXgv822g/fxj92Rnr3bV8atF1veA9rJpG4u1DjccsfhjJ+L9vhMt
3tcPPXD3e9//lje/6y3v+sidd104Xu+c2XG1q2e1l726mno3qw8+LdBX4p0X99Clk7edPXN+etyJ
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
4w/tnnrUyXO1r5135y7WT7+2ftRl/t5zlTsqzJ5v/lE7kSc/5vK96dTXzrtJ7Zx3dS0y8c5L7WRy
Ybf+8D0XRLz3q6O80Zpf+uy2ThPjrrvuuWqzk3vD9dctN9hxeqwe9ju/+wcvefELNh194w3XN/78
kUfOPfpR12w66/GPaz6rO4977JYW2h3wkTz08CPXX//oTb/91V/7vW/99v8Yr/d1bn+wftFNGz9b
78pT7hnXT95zR5iNj59x/aTdw23xfx+55Fv2j74mUKX2O26bPfaqjRb5mZN8ciOEFAtteULMwgpo
QgghpBDGP9Nnt0V867cyd5N9vSeX7qwmx/yBYXq0bYZUfjZzx6+uTj9BRMS75Z0zDj4t7+jDCTfE
sL5Vx9H3vinO1QLoo5/Pm3JV/cj7Zve+fuf46VPH64f2Tv/cXz/nVz/4zKmfiLhpPZnVTqQ6Nz1x
od6pxXvvLuzV11xWPfEaEfGLu1XPP+rwytM711958vyl6axy3ktdV95PvEym4na9c9XOvWenH33g
0v4nA7ZkOo4777qn5bfPeubTxnchIg8++FDLb8+fb64Fvu32O1vOeuJNj7vu0deOieolL35e+wEf
ve2OMe2388EPfbTlty3OeyTef9cWc/nlTwtWFvOyp250urcG88iFtmnf4hr3on0PkL1Zaf5LafkS
QgghJUIDmhiEj7GkG5wpquHwmae12HaxFLfJCq7PflhERJw4d1SS7ERkXu3s3JmnLbS82dfeN5r9
vkvrVw84KDpe2JDDH4bUYuwuGL9z/9l7L65+6B3+kffJ8SsrNzs+qf/oY0989c3PveiPOedq2am9
m9bVw5dO1L6a1W6vdscq94Sr6uWW97N96g2XX37q1KU9P/OTqbhapPZVXc83lXays/Ohey+euzBz
zvnFbHqXOG/hHe98b8tvP/MzXh5k44Ubrn9My29vu+32xp+//+YPtjf70k/aWFXdhU/+pBe2H/D+
mz80pv12/voDH2757bOe9bSqSroKeOdtbZtOiMjf+8Rjx7b4xp3Ymcjf+8Tj24LZaEDfen9bnE+9
rrr+igC6XXtZWyMPtZrghIgIn4KUw+Ej2+EsIfagAU0IIYQQXPo8fx8dOzv7Yb/7iJtMxNeHvqr3
XsRL5erpRXfZU+T41YfbPuz7yEtbTWzbd+KgrNqv/2rpuw27QK955uJqLzK7+8/9pQe9TOp6evnx
+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
Gzw37Je8VDi9UBy9VhC9VPl89LvFI5f20Jjt3l/PLnh3dMxkZ+fP737SI7NTTtysnuzt7czqycxN
9uTUPPiqquq6vvaK00977KPPndvzbmdWV3Vd1fWk9ju1SO2r2UyOHTv2no8+8ND5XRG3/6mMKyou
K7CW8qoA2w7zDz989od+9DVb2hH5p9/8j978ht/68i/5rNOnjrX3e3xHPu3ln/C6P/3Vb/kX37y1
2df8zK+1/Pbs2fM/8VO/srWRG2+47r//4g99//f+22c982mNB5w+dfJLv+Rz/vB3f/qbv/Frtpq5
Fy9e+vFX/cLWTkfjL13affXP/Pf2g3Z2dr7tW77hN3/1x77w8z9jZ6fZ873xhsd827d8wy/+7A9c
deUVLU199me9/AmPv7G9ux9/7e49HT5h77oz7ke++vSvf+NlX/jcY1dt3lLjipPuc5997JdfcdlP
/P3T7bsqz7nnrH/l67aby3/x4dmff2B7DfKnPn3nf/6zy7/rS0+98AmTqnXMn3XD5Ns/7+Tv/dPL
Pv3jtrvqr37D9ghJVvh3nxASAU23Fk2xEijclY/a/o65UH2Z6CdW66PbHdRAgGxSDWvPHhsO6hFp
sqT6dzQkNDf81G4tRz0jRuCu8Z/9T456zgh6drd2eOfz01/+cQIY24yuO+mwrg4fNN3yt8O6ao/B
LR/iln669vP5h++5o7Pc4W+dE3HHTzz16+X0Y/30krhFf8qJeOdrL8dcfXb2odfU00cOG1zoyy3+
6CCKw2/cUWtLOfUV2a/908vktLvpa6U6KX5X3ETEO+ems+n/+aw3fcLVH92b1Vccu3TF8fPXXnni
9987+1e/8UhViYir69nnvvgZz3jCddPdSzs7VeV85X0l3lXeSV35qfP1zrHqlf/rvR+955ENBvRB
NfdKbKuftbgWcGt2J04c/8u/+O0Tx493VOSee+79rd/+/bf95bsffPChRx4555ycvuzUVVdc8dzn
PvNvftanPeXJT+zYzvd9/49+/w9vsXovu+z07/7mTzz2xsd0bPOuu+/7ize//c677r3//gdPnjxx
zTVXPe0pN73wBc85dqxrxe73fv9P/OAP//TWw2557x+3/Panf+7X/913fF9rA15ETp08+bu/9ZOP
f9wNXQK7cPHiW976rg/ccusDDzy0u7d35rLLrrvu2mc/++nPePqTq2q7vfu//vC1//if/Outh33x
8459z5ef6hLPnNrLe++Yvf+u+sHzfr6Dx1Wn3dWn3dOuq5594xbnd4Vv+ZULv/GOLftQz3nG9ZNf
/8bLthWyH3F+17/7Y/UH7p49dME/csk7cZedkCtOuic/unrm9ZOt21If8md/Pf2HrzkPsbZPGkKi
zgJ1M66ZYLlmnySdAuj5NpvhHYWjf3dDAhz6Bq8+jUc9I3js6w8Ww06Oes4gOne0duDwM6Mzusfh
DeQ4027b6XuyaEDH7YoGdPgmYvQ4zoDuffRQaEDHOqdzi2lSGH5air5oQJdlQA/uzS+cO9KAbv+d
W/v9wk3sqLZ0yYA+9KEXDWhxlRPZueaFx57wZbPdR6Sa7MfuFrKpZ9XxK+r73zL92P8QV+2XQssm
D3rx5rHohQ9zoldWW0vbeVST49Xjv7Y+dkb8rshEvEwqf2567Ase/76vuuktl3b9mWOXrjxx7tFX
nf7pN1/4gT86vzOR6cw/+cZrP/cTn7O7e/G4k6qSqpKJ85V4JzPnRPzs9PGdm2+792f/9H3inF/+
3ML9/9v/rsGAbl4cdjOgReS5z3n6r/3Sj7cKEph77rn3kz/tS2f19m0ZPvHFz/uZn/ovVS8vcyjv
eOf7vuLvfvNstr4v+SrjDOijEfmUl734J3/8P/UIcRD33vfAF3zx19/buuPHIf/5y0990fO27EQR
nN94x963/MqF7sf/o5cd/7bP2fJpgWE5v+u/5IfPfejeOoebsIatFW/QnmhA9whAmwE9qC8a0CEw
akD36YUGdOR+tZrE6v5ItsItOIg6sj9vkUM4FoSQzcS9Q/iGb49s3KP/nT747vrCbbJzct9UPTSo
5/+oJrPdRybXPH9y5unia3HVUiML/17ejGNTdgcbRB8tLVu+Dv/p15vzXryv5XCLDidepHLuQw9d
eX5vp/YylcnebLI79R+4cyriZ7VcfurE33j20/f2vPhq6idTP5nVbla7ma9mMqm9q0WmMnn9++8S
Wd6VZMF5Xs6t4xB2Ouyd77r5e//rj3VrMAx/62///S7us4i86c3v+MEf2V6SPJ4HH3rkX3zrv+/i
Po9gdTr92Wvf/BOv/uWYPcpsVv/zb/mOju6zBIz8tgAAwu1JREFUiPyb37z47o81bc0ejXd/bPZv
f/Nir1Ne9brdP9z2aYRh+fZfv/ihe6POjc7w+Y4Q0gbvEThwLIgmaEATQhLCP5Gm4HAWyGIddMwu
Nk2u9V2Ymz5H7/BXXpzUl2b3vmkyOe5r39isczKrdyfXf3a1c/nhZtAHhzZ60Ie/8gcbU/ijTyaU
g2MOnejlOuOFr6XfLTXivciOOOelPrTMvbhJNbtv9/KH906KyF5d1VI9fMF/+P6ZiHhfP/8ZT7nq
iqt2pzMvEy+u9m5WVzPvZt7NpNrz1YmTl7//9vs/dOeDIq5ucL39yn83iNyrSmnpiB/80Z/94R9P
4fOKyJd+xT+4574en/L3X3/gJ3/hl/5HvHhE5PyFi//wFd/24Vtvi9pLI//hu3/4D//49fHa/87v
/qHXv/Ft3Y8/v+u/7tXnb74rkdl681311736/Pndfn+zvJf/85fOv+6WRB70f/mDS7/9rk7bgxAC
AJ8ADcHBJISkggY0Cnnu/Px7QwhZQ8+NQU+kZBQd3+fbWAQ98yLTB95Rn/tQdeykSL1aHupEXOXr
PTlx9eT6z5p/dKHI0m4Uqx70ain00b+bnegjP3qtArrhlIPGj10hx06Jn7r9rau9eDdx/uzu5MG9
k1JVe3Ulk2N3PFzf/mAtIpOqetRVV12azrybzGQyk8p7X4ubeTf1MqvFe3dpJq9990cbfOclz79v
+XM/vud7X/k9/+VHYrS8yBd+8Ve/49239j3r3/w/3/uLv/LbMeIRkYcfOfuKb/r2t7/zvZHaP6B5
1Oq6/mf/4v/9kz97Y4wuf/CHf/qnXrP9gxxXePC8/7uvOvfGD0W3d9/4oenffdW5B88Pmc8X9+Qb
fuZ8gjro7/qfF3/oTy8dfMc/bWQrOiaJjigJIQQWc7dRGtCEEEIISUyH56n1WufVnx9api1F0CLi
pJ7O7vwT55z3C5tvLBziZDLbPeeuet7Ooz9ZfL2/kfPRrsfLHrQsfyCfyGEp9EKbvuPXcihevPfe
iYg7cbXbuXy+X/VB996Jn8rxC/VJL1Xtq2M7x265zz98sRbxp04eP3ny5O6s9m7ivdR1VctO7auZ
r2Z1tTf1x05e9ta/vv1j9z60/NmD62Kua+gbf7w+Hh354R/7+S/5in944WK//RA6cssHP/zCT/yc
9978sQHn1rX/9n/zPf/vd/7AbBZ4d4gP3HLr3/6Kb3zDG/8ybLNrtI3FhYsXX/FN/+oXfjlklXdd
19/5XT/0vd//qmGnP3TBf92rz7/qdbt1nPVV7eVVr9v9uleff+jC8A4uTeUbf+78d/3Pi9M4W4Y8
cN6/4mfOv/K1u1FaJ4QQQgiBIaUBbc69J2QsvCjyQN1J2QS5ArrtwjGwq/bTNv62uQja117c9OGb
/QNvmxw/I/Vs5Zz9HS6qarb3yOS6T5lc+fF+bkqLP6p4XvagD0qhl/fXWDSWe+S6fpYX8XLyBpnv
BjLf/tl7mTvTUk39pK4mUjmZ7Lz5w5fmsV15+RXVzqm9aT0T52WnFjf1Us/33/Di3PF7Hz77xnd9
0K2O26by543h9kht88HvevcHXvQ3vvCnfvqX+rS2nf/n//3uz/mCr33w4VFe3qt/+r9/2Vd+8zvf
9b4gIU2n01f+5C/97b/zjTF33ljZ52Ujs9nsX/3b7/k//vm/u6/zZs0t3P6xu77m6//lq35q1CBO
Z/Iff+/iV7/q3PtDb8fx/rvqv/vKc//x9wIYx97LK1+7+xU/du7NHw5sQv/uu/e+8L+d/eP3J91p
ejtJH5L4REbKhbM/H9S+ODjkILACmhBV8N6ZHQVDoCBEWwQQXNWYZQl2U7VthyLog8N27/gjuXiv
TI6L1N7t10YvGOnOeTfb25vc8HmTy58iUotU+6XMh77zvgd9WAp9+HmD63Ftc6I3HeC987W4SXXy
sfVsV5wTEefcgqFee5E9qXZ2Jvefl3fctjvfL+RRV19bz6q9qavraub8TKQWNxNfezetpdo58dq3
33z+4q5fNNUbZFz7yfo3gbh4ae87vvMHX/LJX/gLv/TrI5uq6/oHfvDHn/XcT3/Nz/+ul06fOtjO
u979/i/7ym/6//zr//ShDw93jWez+vd+/8++4Ev+0X/47h8+f/7C+KhC8Tu/9yef/YV//ydf8yuD
i9AvXdr9sVf9/Od90de98U1harrfcuvsi37w7Lf8yoUgu0LffFf9Lb9y4Yt+8OxbPxLSL373x2Z/
91Xn/vHPnn/7R8c267289gPTL/vRc//0Fy/c/Yiq279mAgnN8UoMvODwAdqHQ0CIHtyVj3p6yu70
9wPb9NCzxyaUakx7dtpwRL9Ik+XVv6PeZ7ihPfVoPOYZUQJ3K/8ddHLsc0bQp7umYzufn+UOECEA
gDthWikH9Lb4gO/WfjKsq5bfrV+jy7eyo1LewyMPD3AHP3YHBzhxlRO/c/Vzj930v033zjpXeVmr
BhaR2VSqnUrq6Ud/rT7/QZFK5h8D6Ba7dOsBHPW2Ja9GFrb6cE68ry67afKEvzObXXLzQoF5QbZ4
5yqZ7f7jZ7zxpjP3PeaKnbd96IHv/d2PicjOZPKpL3nZlWeunM0uHZvIzsRVrq6kds5LPT198tT7
br31T978ThHnfb3Q42H589K3yyEt+uuLu46s/aQlr20c26k+5WUv/Ptf83c++aUv6XjKnD/4wz99
1U/+3Fve9t7aB/Cd13HOferLX/IVX/b5L//kF1122emOZ33wQx/9/T/485/9+d/42B13j+n9lvf+
cctvf/rnfv3ffcd/HdP+1Vdf+dVf+cVf8Hmf/vSnPanjKXffc98v/+rv/Nwv/uadd94zpusWXviE
yRd/wrFPe/rODVf2q5K546H6T26e/sbb98L6zo08/THVV7zg+Kc/Y+ema/oFecs99f98z96vvHXv
ow9sstoBrBSjFdAQBnSwdLPPk04BNB/UL/bEmfbvbsQ7vaLl1rPhIXGEj339CWPAmVHPGUSvt8QN
OzlnXUjyBob3G1kmlKsZvzMa0GaapgG97Qga0L176tF41DNixO4a/9n/5KjnjIAGdOJmaEAP66o9
DLd2yMLdbMk9XvKg3ZJ5Pf93JSJzD/r4E76kuvYT60sPSbXT0Kf34mupjjm/O7v9N+uzt4ibiJ81
eNANd1bnpDngDSyttvatYFeJr3du/AJ31fNme+fdQppOfO2qU3L+G5/+2secPnvD1Sf+82/f+tYP
Pywij7762k960cvq6W41kUnlK5Fq4ncqcbP6+LHJuQvnfvPPXnfp4qXl8udDu9mv/Hc5MN+8Jtxu
QA95iq2c3HD9tU9+0uOe8XFPff4nfPwNNzzmUddee+21V0+ns/vuv/+B+x/80K0fede73vP+999y
y4c+eve9D0uIeucu7OzsvOiFz3n+85711Kfc9NSn3HTttVdddtnpy06fmk5n586df+Tsuds/dtdf
//WHb/7rD73ujW/96EfvSBJUsHXCE55w40v/xguf+YynfNzTn3z9Yx595vLLLrvslHPuwoVL586d
v+POuz/8kdvf//5b3vCmv3zPez9Q14H3ytjEU6+rnvvYybNumNx0TXX9le5Rl1enj8uJHScil6b+
/K7ce7a+8yF/6/31e+6YvfP22QfuThTYIjdeWX3ikyZPu27yxEdVN11TXXnKnT4up4+72sv5XX9h
z99/zn/k/vrD99Xvv7N+04em95wNu8tNHMwtd4P2RAO6RwA0oNvPiZNb0hQCQgO6IAM6h41MAxqk
MxrQZpqmAb3tiHINaFktzguJVveWBnT74XoMaAkTA8CdEN6AloXnkoQG9NJR44qgK+e8uMnJE0/9
On/yRj89L26yn8RhS3O71dfe7VQym93xu/XDfyWuWk59pRR6PZGFH21K0R/+j6woWZ187OSmr5zV
MydOnDv8beX8rp/ccPy+f/iUNzzmzOzuh85/52/eOvPiff28Z7/oiY9/yt6lszuVuEqc+Kqqdyo/
cXJssvP7r3/dHXffK7Ky+QZK+XMrbrmdTWoi3IiSkd14IlEBGF9zy92gPSEY0ACTZLAB3Tt2GtAD
G496RkwDWmv8ATqiAY15Zt7mzf1F5h7QhBBCBoOwCiLpGfU83bmd9d/6pv8s/OuwlvfI3/VHB9Te
i/Ozi7u3/qqbnvPu2NH+zgcfRSh+3kjl/J73svO4L5486uW+nh/mDho+LAr2hz85+FpsZV5wPGfx
swaPfrB2lhPnqute5qvjztfzbTcWD5nV7lHHH9pxu5Vzv/uOe6e1F+9Pnjh5/WMevzudejk285Pa
V7VUs7ram8qxE5e/6d3vvuPue51zS6XMq+7zRjFzX+Ed+/f5I01BIWmWDMD4AoRA4OEsIYQQYyS6
sdOAJoSQBPBh3TaFjG9DnW8ulgqIl37h145b8KB9LeLqi3dPb/v1ncmOr/28KNgtHrif10TEz3Yv
Ta77jOOP/9tu54xIPa8untvLftG9PvKTD3+84hW2/nB/awwnfja56kXV5U+vdw+qs1fKf/3ssZc9
dObksffe9sjbbj1XifPeP+7GJx0/dWY6m9biZlLNvJvVbjr1x46f+aub3/e+D9wiTg62TVgpdl41
7lckXa95bv5lA6HKn1u+bezUpEVrNS9C1MHL0DYcX0IIiQsNaEJKgU9VpDvFzRaAhNOGEKq38R50
Bx9zkym69N1qO36DZ+p97UWmD71v7/bf3jl+WryI83MH+qBG+MCLdpVzVX3prFz53J0n/r3JmY8T
PxPv97Per5Ve3NXi0FA+/Oei77z8tVQA7b1Uztfu9E3Voz95tnuuqtx815DDr8r52ldXHN+96fKz
l2bud95576yuvcjOzs71Nzz10qU9kUnt3Exk5qtpLcdPXP7RO25927vfJW6hLrxZ+g4+/uYRiMaY
qWXGrjWTCCGNpJveEBcSRBDpKCxdMgrOFkIKgQZ0X3h7VAuHLgqUlRCyjfH3iUaPdPUXq/W8yxtx
LNRBi/fipve+aXrH7++cuNx7J1KL98uWpxNxzjmZTPylR/yxqydP+IqdG/+WO3alyEzEi6v229uv
X140m1eM5qavI4vae3EiM3fqhuqxf6uWStzMi3P+yIJ1zotze7PJ408/8IRHyRs+8NAtd83Ln+vr
H/PEE6euvrQ7rWVHZOJkMpvJzvHL77jvzje//S11Pdv3t4+E6rD5xuo36/9MUP48vh29hcN6Iydj
ABhxgBAIIfDwThEBikrKoOkj4AnRjy/sM4kIWYZXAInE4YfCBZxj7U2t/3b5J94vfBrhhh96EXd4
lhfvxHkvbu/uP3NuZ+fGvzm99Ihztcj8wwbnHyHoRZyvRLzIZCL1pbqW6poXusufXN/3xtkDb5f6
kog7sKHrw208DsqjO4rjxHnxdXXZkyY3/i0/OSWzS+J2RMS7xTSdeNmR6fOvu+/+hy/84bvvcM55
8ceOnbjxpo+/NN0V7yrvvIjU/tTJy+974Pa3vPV1e3u7Ik7qBvd53axfdpkBl0G8oRFCSGkA/jEi
JBGc/cQkNKAJIaQn9tzdURnZk4NAsjTR9r9Z/s/qQXJgNh/8pj5445ff33lDqt27/uiYzCbX/83Z
3gXnanGTg2f+/fMO2qtEfL37sExOTW74vMlVz5098I76ob/ys3P7R8z97v3qZne0cNh4cTgRJ74W
L5NrXlJd9+ninNS74iYic7t88dB6d7bz1DP3PeWai7/whx8+d2HXVc7X/vFPfN7xU9fsXnp4p3JT
792sPn36zH0P3P7Wt712b++SyPyDBxvc562bb4wofwbkYEAVoE5bEgoOPdHCiLlqb5rby4gQQmJC
A5oQQggpEEWvHLSF6g+qlVcPPiiCXvagV+qga++qvbv+1NeXdm78/NlsKn7q3GR/V4w1y9tPdqSe
1tNH5NijJzd+fnXtS/wj75899F5/4Xbxs0Mn2nm3v52HX96J+rC0Wur5srU6eZ179MsnVz6nnp6T
uvZSuaPi6aOMaqmq+sLLbnrgde/84F/fdm9VVXU9u+qqGx/z2GddunjeuZ3a1/V0etnpy++868Pv
esefT6fd3Odem290BWT/jZYGAac9PQxSIJz2hBBCSFm4Kx/19JTdmegnVuuj2x3UwNhecy3ktvfb
cES/YNOk1r+XIXGt+hlBSZBClMDdyn8HnRz7nBH07G7t8D7n5zV0AvUeoBmMJqL3tmqpdvURtvTW
/uvG63X5zra0Ecfh8YfHLP5kobbZORHnxO9c/ZzJ475YqmN+96JMdpp2l3YLG5DMvBc3OVHtnJTZ
eX/utvrcB+tzH5aL93i/tzUd50RO3Ti58uPlimfIzhV+97yrKhHnxbuDmPc3ChGZuPrcpfqTb7z3
+tlbf+N173bOe+93jp181gu+8Pjxy+rZbuW883Ly1Mm7bn/3ze99k/f+wH0+jHxxw+ftm2+MK38O
Yiq5tXaCXxkgNjQ9OCIo0yB1FOn6C9TT6GaCZZx3wnTtveG43oEnzrR/dwMDXPlTG5aeDQ+JI3zs
o175xkhhbC9rx3Y7OdfdYFy/w8/OcWbe5tX9qdwGK6CLZ2wNnKIaOqKXqNMs2RzmxRKJQMJyfLqy
bg4moHF4Nm3EsXjIWh300Yn+4J/eSzV94F3+0gPHHv8l7tSNs0uPuIkTcd7NDWy3utG0mzgn4ndn
ly45V7nLnzS54qnV9Jzs3u8v3CuX7vZ7D0wvPlDV58TPxNfei5tU3p2oTlwrpx5bXf4kOXWjm5yo
9y74vfOuqg5CkbnNfVirXXl/adffdObCdfL+//nm94rUc3P5cU/55J1Tj9q9dNaJVJPjlfMfvPmN
t936Vwf5LNc+H/1rqPvclUgTI8aVuR5qgusfw2ckhKTHiPtsmGTCRu2I04OQ9PC66wEroLFaH9f0
0LNVFkEXXAE95CTtFdADz+ncYpoUhp+Woq+mwzs3kd24BSmCLqQCWtaKoINUQHc5wq0d5VZ/5xp+
tVwH7dbuiPNTJ05qt3P5scd/QXXVJ9R756WeSjXxh/a19865NZfbH3jHXsRV7rhMjleu8vXMT8/L
7KKX2rmZ916qibhj7tgZ507U9Z6fnZfaL5RjH/yncov11nvT+swJ//zL3/H2d/zJQ2cvOVfVfnb9
TS+88cmftHvpYRF/8vhlexfuu/Xm1z54/+1y5CEv+8v7/zPCfe5U/hzv+Tv9HSZgj1yWkK1gTBJW
QEdtxo4B3SmA5oOgK6AH9TWi9jZOblrLh4e/+D2iCD0+ViugM94MMSugtbadpSejBnTcrmCbpgG9
7QhEA3pQRzSgQ0ADeuvhNKATnx+miUQdHj6muOVvx3TVzYBePXDZcm7ciGPh5/se9MHRS0e6ufvr
j133suq6T5HJZbO9c64SJxPxIvO9LaqGfP1hW96L99558c5Vlchk32J2Xrz34sVPxXsnlXdLZvkK
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
AO54A9qfvNLNHlOEAAAAAElFTkSuQmCC
FIN
cp "$IMG/logo.png" "$I/usr/share/plymouth/themes/flashos/logo.png"
# Pastille de version sous le logo (dessinée pour la v13 ; autre version : texte simple)
PASTILLE_VERSION=v1.0.3
base64 -d > "$IMG/version.png" <<'FIN'
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
FIN
if [ "$VERSION_FLASHOS" = "$PASTILLE_VERSION" ]; then
  cp "$IMG/version.png" "$I/usr/share/plymouth/themes/flashos/version.png"
fi
mkdir -p "$I/usr/share/flashos"
cp "$IMG/splash-1920x1080.png" "$I/usr/share/flashos/demarrage.png"

cat > "$I/usr/share/plymouth/themes/flashos/flashos.plymouth" <<'FIN'
[Plymouth Theme]
Name=SoleOS
Description=Démarrage SoleOS
ModuleName=script

[script]
ImageDir=/usr/share/plymouth/themes/flashos
ScriptFile=/usr/share/plymouth/themes/flashos/flashos.script
FIN

cat > "$I/usr/share/plymouth/themes/flashos/flashos.script" <<'FIN'
Window.SetBackgroundTopColor(0.10, 0.14, 0.25);
Window.SetBackgroundBottomColor(0.06, 0.09, 0.16);
largeur = Window.GetWidth();
hauteur = Window.GetHeight();
logo.image = Image("logo.png");
if (logo.image.GetWidth() > largeur * 0.5) {
  logo.image = logo.image.Scale(largeur * 0.5, logo.image.GetHeight() * largeur * 0.5 / logo.image.GetWidth());
}
logo.sprite = Sprite(logo.image);
logo.sprite.SetX(Window.GetX() + largeur / 2 - logo.image.GetWidth() / 2);
logo.sprite.SetY(Window.GetY() + hauteur / 2 - logo.image.GetHeight() / 2);
// Version installée ou démarrée, dans une pastille jaune sous le logo
pastille.image = Image("version.png");
pastille.sprite = Sprite(pastille.image);
pastille.sprite.SetX(Window.GetX() + largeur / 2 - pastille.image.GetWidth() / 2);
pastille.sprite.SetY(Window.GetY() + hauteur / 2 + logo.image.GetHeight() / 2 + 4);
pastille.sprite.SetZ(10);
temps = 0;
fun pulsation() {
  temps = temps + 1;
  logo.sprite.SetOpacity(0.75 + 0.25 * Math.Sin(temps / 18));
  // Pastille redessinée en continu : reste visible quand la carte graphique prend le relais
  pastille.sprite.SetOpacity(0.99 + 0.01 * (temps % 2));
}
Plymouth.SetRefreshFunction(pulsation);

// --- Demande du code (ouverture des données chiffrées) ---------------------
// Le texte de la question vient de flashos-securite (« Entrez votre code »).
fun placer(sprite, image, y) {
  sprite.SetX(Window.GetX() + largeur / 2 - image.GetWidth() / 2);
  sprite.SetY(Window.GetY() + y);
}
y_saisie = hauteur / 2 + logo.image.GetHeight() / 2 + pastille.image.GetHeight() + hauteur * 0.05;
invite.sprite = Sprite();
puces.sprite = Sprite();
message.sprite = Sprite();
fun afficher_saisie(texte, nombre) {
  invite.image = Image.Text(texte, 0.95, 0.96, 0.98, 1, "Sans 22");
  invite.sprite.SetImage(invite.image);
  placer(invite.sprite, invite.image, y_saisie);
  invite.sprite.SetOpacity(1);
  points = "";
  for (i = 0; i < nombre && i < 40; i++) points = points + "●  ";
  if (nombre == 0) points = "_";
  puces.image = Image.Text(points, 1, 1, 1, 1, "Sans 26");
  puces.sprite.SetImage(puces.image);
  placer(puces.sprite, puces.image, y_saisie + invite.image.GetHeight() + 22);
  puces.sprite.SetOpacity(1);
}
fun afficher_normal() {
  invite.sprite.SetOpacity(0);
  puces.sprite.SetOpacity(0);
}
fun afficher_message(texte) {
  message.image = Image.Text(texte, 1.0, 0.75, 0.56, 1, "Sans 17");
  message.sprite.SetImage(message.image);
  placer(message.sprite, message.image, y_saisie + 130);
  message.sprite.SetOpacity(1);
}
Plymouth.SetDisplayPasswordFunction(afficher_saisie);
Plymouth.SetDisplayNormalFunction(afficher_normal);
Plymouth.SetMessageFunction(afficher_message);
FIN
# Pastille dessinée pour une autre version : la version écrite simplement, en jaune
if [ ! -f "$I/usr/share/plymouth/themes/flashos/version.png" ]; then
  sed -i "s/Image(\"version.png\")/Image.Text(\"$VERSION_FLASHOS\", 1.0, 0.76, 0.24, 1, \"Sans Bold 18\")/" \
    "$I/usr/share/plymouth/themes/flashos/flashos.script"
fi

#  Pas de texte de connexion à l'écran
mkdir -p "$I/etc/skel"
touch "$I/etc/skel/.hushlogin"

#  Menus de démarrage de la clé : logo SoleOS, aucun choix affiché
#  (Échap au démarrage fait apparaître le menu de secours).
cat > "$C/hooks/normal/9100-flashos-demarrage.hook.binary" <<FIN
#!/bin/sh
IMG="$IMG"
FIN
cat >> "$C/hooks/normal/9100-flashos-demarrage.hook.binary" <<'FIN'
for f in $(find . -name 'splash*.png' 2>/dev/null); do
  case "$f" in
    *isolinux*|*syslinux*) cp "$IMG/splash-640x480.png" "$f" ;;
    *) cp "$IMG/splash-1920x1080.png" "$f" ;;
  esac
done
# --- Menu GRUB de la clé (démarrage UEFI) : image SoleOS immédiate, aucun menu, aucun bip
noyau=""; initrd=""
for f in boot/grub/grub.cfg $(find boot/grub -name '*.cfg' 2>/dev/null); do
  [ -f "$f" ] || continue
  [ -n "$noyau" ] || noyau=$(grep -m1 -E '^[[:space:]]*linux[[:space:]].*boot=live' "$f" | sed -E 's/^[[:space:]]*linux[[:space:]]+//')
  [ -n "$initrd" ] || initrd=$(grep -m1 -E '^[[:space:]]*initrd[[:space:]]' "$f" | sed -E 's/^[[:space:]]*initrd[[:space:]]+//')
done
if [ -n "$noyau" ] && [ -n "$initrd" ] && [ -f boot/grub/grub.cfg ]; then
  cp boot/grub/grub.cfg boot/grub/grub-debian.cfg
  cp "$IMG/splash-1920x1080.png" boot/grub/flashos.png
  secours=$(echo "$noyau" | sed -E 's/ (quiet|splash)\b//g')
  {
    echo '# SoleOS : démarrage direct (Échap au démarrage pour le menu de secours)'
    echo 'set default=0'
    echo 'set timeout_style=hidden'
    echo 'set timeout=1'
    echo 'insmod all_video'
    echo 'insmod gfxterm'
    echo 'insmod png'
    echo 'set gfxmode=auto'
    echo 'set gfxpayload=keep'
    echo 'terminal_output gfxterm'
    echo 'background_image -m stretch ($root)/boot/grub/flashos.png'
    echo 'menuentry "SoleOS" {'
    echo "  linux $noyau"
    echo "  initrd $initrd"
    echo '}'
    echo 'menuentry "SoleOS (mode de secours : affichage simple)" {'
    echo "  linux $secours nomodeset"
    echo "  initrd $initrd"
    echo '}'
    echo 'menuentry "Menu de démarrage d origine" {'
    echo '  configfile ($root)/boot/grub/grub-debian.cfg'
    echo '}'
  } > boot/grub/grub.cfg
  # Plus de signal sonore dans les menus d'origine
  grep -rlE '^[[:space:]]*play[[:space:]]' boot/grub 2>/dev/null | xargs -r sed -i -E 's/^([[:space:]]*)play[[:space:]].*/\1true/'
else
  for f in $(find boot/grub -name 'grub.cfg' 2>/dev/null); do
    printf '\nset timeout=1\nset timeout_style=hidden\n' >> "$f"
  done
fi
for f in isolinux/isolinux.cfg syslinux/syslinux.cfg; do
  [ -f "$f" ] && sed -i -E '/^[[:space:]]*(timeout|prompt)[[:space:]]/Id' "$f" && printf 'prompt 0\ntimeout 1\n' >> "$f"
done
grep -rlI -e 'Debian' -e 'Live system' boot/grub isolinux syslinux 2>/dev/null \
  | xargs -r sed -i 's/Debian GNU\/Linux Live system/SoleOS/g; s/Debian GNU\/Linux/SoleOS/g; s/Live system/SoleOS/g'
exit 0
FIN
chmod 755 "$C/hooks/normal/9100-flashos-demarrage.hook.binary"

# --- 6. Réglages finaux dans le système (exécutés pendant la construction) --
cat > "$C/hooks/normal/9000-flashos.hook.chroot" <<'FIN'
#!/bin/sh
set -e
chmod 755 /usr/local/bin/flashos-* /usr/local/sbin/flashos-getty /usr/local/sbin/flashos-installer /usr/local/sbin/flashos-systeme /usr/local/sbin/flashos-securite /usr/local/sbin/flashos-energie /usr/local/sbin/flashos-son
# Sécurité : chiffrement avec la puce TPM, écran verrouillé.
# (Installés ici pour un message clair si un nom de paquet change.)
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd-cryptsetup \
  || echo "SoleOS : paquet systemd-cryptsetup absent (outils peut-être inclus dans systemd)."
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends gtklock tpm2-tools \
  || { echo "SoleOS : impossible d'installer gtklock / tpm2-tools." >&2; exit 1; }
# Miniatures des photos et des vidéos dans Fichiers (nom du paquet selon la version de Debian)
# Pilotes libres des imprimantes plus anciennes : Brother, Epson, Samsung/Xerox, HP/Minolta (GDI)
for p in printer-driver-brlaser printer-driver-escpr printer-driver-splix printer-driver-foo2zjs; do
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$p" >/dev/null 2>&1 || true
done
for p in libgdk-pixbuf2.0-bin libgdk-pixbuf-2.0-bin ffmpegthumbnailer; do
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$p" >/dev/null 2>&1 || true
done
for p in $(apt-cache pkgnames libtss2- | grep -E '^libtss2-(esys|mu|rc|tcti-device|sys)' | sort -u); do
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$p" || true
done
for b in /usr/bin/systemd-cryptenroll /usr/bin/gtklock /usr/sbin/cryptsetup; do
  [ -x "$b" ] || { echo "SoleOS : $b manquant." >&2; exit 1; }
done
ls /usr/lib/systemd/systemd-cryptsetup /usr/bin/systemd-cryptsetup >/dev/null 2>&1 \
  || { echo "SoleOS : systemd-cryptsetup manquant." >&2; exit 1; }
# Écran verrouillé en français (gtklock n'a pas de traduction française : 4 phrases)
# (traductions de l'écran verrouillé : dossier « locale », installé plus haut)
# Écran verrouillé : le code (vrais chiffres : Maj ou pavé numérique) ou la clé de secours
cat > /etc/pam.d/gtklock <<'PAM'
# SoleOS : vérification par flashos-securite (code de l'utilisateur, attente croissante après des erreurs)
auth sufficient pam_exec.so quiet expose_authtok /usr/local/libexec/flashos-verifier-code
auth requisite pam_deny.so
account include common-account
PAM
systemctl enable nftables flashos-compte.service flashos-verrnum.service apparmor earlyoom power-profiles-daemon thermald flashos-energie.service flashos-son.service >/dev/null 2>&1 || true
# Mémoire presque pleine (moins de 5 % de mémoire et 10 % de mémoire compressée libres) :
# on ferme d'abord un onglet ou une appli, jamais l'interface SoleOS
cat > /etc/default/earlyoom <<'OOM'
EARLYOOM_ARGS="-m 5 -s 10 -r 0 --prefer (^|/)(chromium|chrome|soffice\.bin)$ --avoid (^|/)(labwc|Xwayland|flashos-accueil|systemd|pipewire|wireplumber|gtklock|NetworkManager)$"
OOM
# Nouveaux fichiers : lisibles par leur seul propriétaire (ANSSI R36)
sed -i 's/^UMASK[[:space:]].*/UMASK\t\t077/' /etc/login.defs
if grep -q '^HOME_MODE' /etc/login.defs; then sed -i 's/^HOME_MODE.*/HOME_MODE\t0700/' /etc/login.defs; else echo 'HOME_MODE	0700' >> /etc/login.defs; fi
# Système installé : lecture seule (overlay), mises à jour A/B en arrière-plan, validation au démarrage
mkdir -p /flashos/demarrage
# Polices SoleOS (Space Grotesk, JetBrains Mono) prises en compte tout de suite
fc-cache -f >/dev/null 2>&1 || true
grep -qx overlay /etc/initramfs-tools/modules 2>/dev/null || echo overlay >> /etc/initramfs-tools/modules
systemctl enable flashos-confirmer.timer flashos-maj.timer flashos-imprimantes-restaurer.service flashos-imprimantes-sauver.path
systemctl enable avahi-daemon bluetooth >/dev/null 2>&1 || true
# Impression : le service CUPS démarre à la première impression (ou à l'ouverture d'une fenêtre
# d'impression), pas au démarrage de l'ordinateur
# (dans cet ordre : « disable cups.service » désactive aussi le socket, cité dans son « Also= »)
systemctl disable cups.service >/dev/null 2>&1 || true
systemctl enable cups.socket cups.path >/dev/null 2>&1 || true
# Vitesse : SSD entretenu chaque semaine (TRIM, le chiffrement le laisse passer), sans ralentir le reste
systemctl enable fstrim.timer >/dev/null 2>&1 || true
# Vitesse au démarrage : services lancés seulement quand on s'en sert
# - podman (espace Debian de Distrobox, sans conteneur système) : rien à relancer au démarrage
# - udisks2 (clés USB, disques) : démarré automatiquement par Fichiers dès qu'il en a besoin
for u in podman.service podman-restart.service podman-clean-transient.service podman-auto-update.timer podman-auto-update.service udisks2.service; do
  systemctl disable "$u" >/dev/null 2>&1 || true
done
# Applications de la boutique : accès aux partages réseau (NAS) ouverts dans Fichiers
flatpak override --system --filesystem=xdg-run/gvfs --filesystem=xdg-run/gvfsd >/dev/null 2>&1 || true
# Navigateurs de la boutique (Chrome, Chromium, Edge, Brave…) : « Installer l'application »
# (sites en applications, PWA) crée ses raccourcis dans les applications de l'utilisateur (v1.0.1)
for n in com.google.Chrome org.chromium.Chromium com.microsoft.Edge com.brave.Browser com.vivaldi.Vivaldi io.github.ungoogled_software.ungoogled_chromium; do
  mkdir -p /var/lib/flatpak/overrides
  printf '[Context]\nfilesystems=xdg-data/applications:create;xdg-data/icons:create;\n' > "/var/lib/flatpak/overrides/$n"
done
# Clavier et souris Bluetooth dès le démarrage (association automatique, attente avant la session)
systemctl enable flashos-deverrouiller.service flashos-bluetooth.service flashos-peripheriques.service flashos-heure.service >/dev/null 2>&1 || true
# Apparence SoleOS (thème sombre, accent, icônes) : prise en compte des réglages par défaut
glib-compile-schemas /usr/share/glib-2.0/schemas >/dev/null 2>&1 || true
# (v13) LibreOffice et les partages réseau (NAS, PC Windows) : sans « X-GIO-NoFuse », Fichiers lui
# passe un chemin ordinaire vers le partage, au lieu d'une adresse smb:// qu'il ne sait pas ouvrir
sed -i 's/^X-GIO-NoFuse=true/#X-GIO-NoFuse=true/' /usr/share/applications/libreoffice-*.desktop 2>/dev/null || true
# (v13.1) Pas d'AFP (ancien protocole Apple) : avec un NAS Synology, les suppressions « réussies »
# n'étaient pas faites (les fichiers revenaient). Les NAS restent visibles et ouverts en SMB.
rm -f /usr/share/gvfs/mounts/afp.mount /usr/share/gvfs/mounts/afp-browse.mount
# (SoleOS v1) Fichiers : le sous-menu « Scripts » du clic droit s'appelle « Raccourcis »
python3 - <<'PYNAU' || true
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
PYNAU
# Clavier français sur un portable QWERTY (sans la touche < > à gauche du W) : variante
# « fr(flashos_ltgt) », < = AltGr + W, > = AltGr + X (Réglages → Langue et clavier)
if [ -f /usr/share/X11/xkb/symbols/fr ] && ! grep -q flashos_ltgt /usr/share/X11/xkb/symbols/fr; then
  cat >> /usr/share/X11/xkb/symbols/fr <<'XKB'

partial alphanumeric_keys
xkb_symbols "flashos_ltgt" {
    include "fr(basic)"
    name[Group1]="French (AZERTY, < > with AltGr+W/X)";
    key <AB01> { [ w, W, less, guillemotleft ] };
    key <AB02> { [ x, X, greater, guillemotright ] };
};
XKB
fi
# Pas d'indexation permanente des fichiers (Nautilus cherche directement dans les dossiers)
for f in /etc/xdg/autostart/*localsearch* /etc/xdg/autostart/tracker-miner*; do rm -f "$f"; done
for u in localsearch-3.service localsearch-control-3.service tracker-miner-fs-3.service tracker-extract-3.service; do
  if [ -e "/usr/lib/systemd/user/$u" ]; then systemctl --global mask "$u" >/dev/null 2>&1 || true; fi
done
true
# Les mises à jour se font par SoleOS (copie A/B), pas par les tâches apt habituelles
systemctl mask apt-daily.timer apt-daily-upgrade.timer >/dev/null 2>&1 || true
# Démarrage rapide et silencieux une fois installé
[ -f /etc/default/grub ] || { [ -f /usr/share/grub/default/grub ] && cp /usr/share/grub/default/grub /etc/default/grub; }
if [ -f /etc/default/grub ]; then
  sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub
  sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT="quiet splash loglevel=3 vt.global_cursor_default=0"/' /etc/default/grub
  sed -i '/^#\?GRUB_\(TIMEOUT\|TIMEOUT_STYLE\|BACKGROUND\|TERMINAL_OUTPUT\|GFXMODE\|GFXPAYLOAD_LINUX\|DISABLE_OS_PROBER\)=/d' /etc/default/grub
  cat >> /etc/default/grub <<'GRUB'
# SoleOS : image dès l'allumage, pas de menu (Échap pour le menu de secours)
GRUB_TIMEOUT=1
GRUB_TIMEOUT_STYLE=hidden
GRUB_BACKGROUND=/usr/share/flashos/demarrage.png
GRUB_TERMINAL_OUTPUT=gfxterm
GRUB_GFXMODE=auto
GRUB_GFXPAYLOAD_LINUX=keep
GRUB_DISABLE_OS_PROBER=true
GRUB
  grep -q '^GRUB_DISTRIBUTOR=' /etc/default/grub && sed -i 's/^GRUB_DISTRIBUTOR=.*/GRUB_DISTRIBUTOR="SoleOS"/' /etc/default/grub
fi
# Installateur aux couleurs de SoleOS
if [ -d /etc/calamares/branding/debian ]; then
  rm -rf /etc/calamares/branding/flashos
  cp -a /etc/calamares/branding/debian /etc/calamares/branding/flashos
  B=/etc/calamares/branding/flashos/branding.desc
  sed -i 's/Debian GNU\/Linux/SoleOS/g; s/Debian/SoleOS/g' "$B"
  sed -i 's/^componentName:.*/componentName:  flashos/' "$B"
  sed -i 's/^branding:.*/branding: flashos/' /etc/calamares/settings.conf
fi
# Français par défaut
# Les six langues de SoleOS (français, anglais, italien, espagnol, allemand, chinois)
for l in fr_FR en_US it_IT es_ES de_DE zh_CN; do
  sed -i "s/^# *$l.UTF-8 UTF-8/$l.UTF-8 UTF-8/" /etc/locale.gen
  grep -q "^$l.UTF-8 UTF-8" /etc/locale.gen || echo "$l.UTF-8 UTF-8" >> /etc/locale.gen
done
locale-gen >/dev/null 2>&1 || true
# Système installé : français, clavier AZERTY, heure de Paris (la clé les règle elle-même au démarrage)
printf 'LANG=fr_FR.UTF-8\nLANGUAGE=fr_FR:fr\n' > /etc/default/locale
cat > /etc/default/keyboard <<'CLAVIER'
XKBMODEL="pc105"
XKBLAYOUT="fr"
XKBVARIANT=""
XKBOPTIONS=""
BACKSPACE="guess"
CLAVIER
ln -sf /usr/share/zoneinfo/Europe/Paris /etc/localtime
echo Europe/Paris > /etc/timezone
# Horloge matérielle en temps universel (Windows la règle à l'heure locale : décalage de 1 à 2 h),
# puis remise à l'heure automatique par Internet
printf '0.0 0 0.0\n0\nUTC\n' > /etc/adjtime
systemctl enable systemd-timesyncd >/dev/null 2>&1 || true
# Matériel des vrais PC (Wi-Fi, son, vidéo) : chaque paquet est facultatif
# (Liste choisie plutôt que « tous les firmwares » : certains, comme firmware-b43-installer,
#  téléchargent des fichiers sur Internet pendant l'installation et font échouer la construction.)
for p in firmware-linux-nonfree firmware-misc-nonfree firmware-amd-graphics firmware-intel-graphics \
         firmware-iwlwifi firmware-realtek firmware-atheros firmware-brcm80211 firmware-mediatek \
         firmware-libertas firmware-intel-sound firmware-sof-signed firmware-cirrus \
         alsa-ucm-conf intel-media-va-driver mesa-va-drivers mesa-vulkan-drivers \
         intel-microcode amd64-microcode; do
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$p" \
    || echo "SoleOS : paquet $p indisponible, ignoré."
done
# Numérisation : NAPS2 (complet : plusieurs pages, recto-verso, PDF, reconnaissance du texte),
# paquet officiel vérifié par son empreinte ; à défaut, Simple Scan
NAPS2_URL=https://github.com/cyanfish/naps2/releases/download/v8.6.0/naps2-8.6.0-linux-x64.deb
NAPS2_SHA=bc618ce0da3817409757e828b23c03992530951a769d1debe7291e48db9b3626
if curl -fsSL --retry 3 -o /tmp/naps2.deb "$NAPS2_URL" && echo "$NAPS2_SHA  /tmp/naps2.deb" | sha256sum -c - >/dev/null; then
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends /tmp/naps2.deb \
    || DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends simple-scan
else
  echo "SoleOS : NAPS2 indisponible, Simple Scan à la place."
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends simple-scan || true
fi
rm -f /tmp/naps2.deb
# Commande « docker » (dans la Console) : la même chose avec Podman, sans service administrateur ;
# pas de message « Emulate Docker CLI » à chaque commande
mkdir -p /etc/containers && touch /etc/containers/nodocker
# Thème d'icônes Papirus allégé : un seul thème (pas les variantes Light, Dark, ePapirus) et moins
# de tailles (ses icônes sont dessinées en SVG : nettes à toutes les tailles). 114 000 fichiers → 25 000 :
# installation et mises à jour plus rapides.
if [ -d /usr/share/icons/Papirus ]; then
  rm -rf /usr/share/icons/Papirus-Light /usr/share/icons/Papirus-Dark /usr/share/icons/ePapirus /usr/share/icons/ePapirus-Dark
  ( cd /usr/share/icons/Papirus && rm -rf 22x22 32x32 ./*@2x ./*/panel 16x16/apps 24x24/apps \
      && find 64x64 -mindepth 1 -maxdepth 1 ! -name places -exec rm -rf {} + \
      && find . -xtype l -delete )
  gtk-update-icon-cache -f -q /usr/share/icons/Papirus 2>/dev/null || true
fi
# Règle des fenêtres compatible avec la version installée
v=$(dpkg-query -W -f='${Version}' labwc 2>/dev/null || true)
if [ -n "$v" ] && dpkg --compare-versions "$v" lt 0.7.2; then
  # Ancienne version du gestionnaire d'affichage : pas de tri par type de fenêtre
  sed -i 's/ type="normal"//' /etc/xdg/labwc/rc.xml
fi
# Pilote graphique Intel chargé dès le début du démarrage
grep -qx i915 /etc/initramfs-tools/modules 2>/dev/null || echo i915 >> /etc/initramfs-tools/modules
chmod 755 /etc/initramfs-tools/hooks/zz-flashos-allege
# Écran de démarrage SoleOS
mkdir -p /etc/plymouth
printf '[Daemon]\nTheme=flashos\nShowDelay=0\n' > /etc/plymouth/plymouthd.conf
plymouth-set-default-theme flashos || true
# Certificats https : liste à jour, et vérification (sans elle, ni la boutique ni les mises à jour ne marchent)
update-ca-certificates --fresh >/dev/null 2>&1 || true
if [ ! -s /etc/ssl/certs/ca-certificates.crt ]; then
  echo "SoleOS : certificats https absents (/etc/ssl/certs/ca-certificates.crt)." >&2
  exit 1
fi
# Conteneurs (Distrobox) : plages d'identifiants réservées attribuées par useradd à la création
# du compte (les fichiers doivent exister)
touch /etc/subuid /etc/subgid
# Identité du système : SoleOS, créé par GDH, basé sur Debian
V="$(cat /etc/flashos-version 2>/dev/null || echo v1)"
rm -f /etc/os-release
cat > /etc/os-release <<OSR
PRETTY_NAME="SoleOS $V"
NAME="SoleOS"
VERSION="$V"
VERSION_ID="${V#v}"
VERSION_CODENAME=trixie
ID=flashos
ID_LIKE=debian
VENDOR_NAME="GDH"
DEBIAN_VERSION_FULL=$(cat /etc/debian_version 2>/dev/null)
OSR
mkdir -p /usr/share/doc/flashos
cat > /usr/share/doc/flashos/LICENSE <<'LICFIN'
SoleOS — Licence / License
==========================

Required Notice: Copyright © 2026 GDH (https://github.com/SoleOS-project)
Créé par / Created by Giuseppe D'Arco.

RÉSUMÉ (français) — le texte juridique qui fait foi est en anglais, plus bas
---------------------------------------------------------------------------
SoleOS est GRATUIT pour tous ceux qui ne gagnent pas d'argent avec lui :
  - les particuliers (usage personnel, familial) ;
  - les associations et structures à but non lucratif, y compris quand elles remettent
    ou revendent des ordinateurs reconditionnés avec SoleOS ;
  - les écoles, universités, administrations et services publics ;
  - les entreprises qui l'installent sur leurs propres ordinateurs, pour leur usage interne.
Toute UTILISATION COMMERCIALE exige une licence payante de GDH, par exemple :
  préinstaller SoleOS sur des ordinateurs vendus par une entreprise, le revendre,
  l'inclure dans un produit ou un service payant, vendre une assistance ou des
  personnalisations autour de SoleOS. Contact : gdarco7@gmail.com
SoleOS est fourni « tel quel », sans aucune garantie (voir « No Liability » ci-dessous).
Le nom et le logo « SoleOS » sont des signes distinctifs de GDH : ils ne sont pas couverts
par cette licence.

SUMMARY (English)
-----------------
SoleOS is FREE for anyone who does not make money with it: individuals, non-profit
organizations (including when they hand out or sell refurbished computers with SoleOS),
schools, universities, public administrations, and companies using it internally on their
own computers. Any COMMERCIAL USE requires a paid license from GDH (contact:
gdarco7@gmail.com). SoleOS is provided "as is", without any warranty. The "SoleOS" name
and logo are not licensed under these terms.

THIRD-PARTY SOFTWARE
--------------------
These terms cover only the SoleOS code (desktop, applications, installer, system tools,
build script, artwork). SoleOS is based on Debian GNU/Linux: every included third-party
program keeps its own license (see /usr/share/doc/<program>/copyright), and those
licenses prevail for those programs. Craft suite icons: © ArtCraft Team, MIT / Apache-2.0.

ADDITIONAL PERMISSIONS (granted by the licensor, on top of the license below)
----------------------------------------------------------------------------
1. Internal business use. Any organization, including a for-profit company, may install
   and use the software on computers it owns or controls, for its own internal purposes
   and those of its staff, without charge. This does not cover providing the software,
   or computers with it, to third parties for a fee.
2. Non-profit refurbishing. A non-profit organization may install the software, unmodified
   and with this notice, on computers that it gives away or sells (for example refurbished
   computers), and may provide free help to their users.
3. Commercial license. Any other use for a commercial advantage or monetary compensation
   (for example pre-installing the software on computers sold by a for-profit business,
   reselling it, including it in a paid product or service, or selling support or
   customization around it) requires a separate written license from GDH.

# PolyForm Noncommercial License 1.0.0

<https://polyformproject.org/licenses/noncommercial/1.0.0>

## Acceptance

In order to get any license under these terms, you must agree
to them as both strict obligations and conditions to all
your licenses.

## Copyright License

The licensor grants you a copyright license for the
software to do everything you might do with the software
that would otherwise infringe the licensor's copyright
in it for any permitted purpose.  However, you may
only distribute the software according to [Distribution
License](#distribution-license) and make changes or new works
based on the software according to [Changes and New Works
License](#changes-and-new-works-license).

## Distribution License

The licensor grants you an additional copyright license
to distribute copies of the software.  Your license
to distribute covers distributing the software with
changes and new works permitted by [Changes and New Works
License](#changes-and-new-works-license).

## Notices

You must ensure that anyone who gets a copy of any part of
the software from you also gets a copy of these terms or the
URL for them above, as well as copies of any plain-text lines
beginning with `Required Notice:` that the licensor provided
with the software.  For example:

> Required Notice: Copyright Yoyodyne, Inc. (http://example.com)

## Changes and New Works License

The licensor grants you an additional copyright license to
make changes and new works based on the software for any
permitted purpose.

## Patent License

The licensor grants you a patent license for the software that
covers patent claims the licensor can license, or becomes able
to license, that you would infringe by using the software.

## Noncommercial Purposes

Any noncommercial purpose is a permitted purpose.

## Personal Uses

Personal use for research, experiment, and testing for
the benefit of public knowledge, personal study, private
entertainment, hobby projects, amateur pursuits, or religious
observance, without any anticipated commercial application,
is use for a permitted purpose.

## Noncommercial Organizations

Use by any charitable organization, educational institution,
public research organization, public safety or health
organization, environmental protection organization,
or government institution is use for a permitted purpose
regardless of the source of funding or obligations resulting
from the funding.

## Fair Use

You may have "fair use" rights for the software under the
law. These terms do not limit them.

## No Other Rights

These terms do not allow you to sublicense or transfer any of
your licenses to anyone else, or prevent the licensor from
granting licenses to anyone else.  These terms do not imply
any other licenses.

## Patent Defense

If you make any written claim that the software infringes or
contributes to infringement of any patent, your patent license
for the software granted under these terms ends immediately. If
your company makes such a claim, your patent license ends
immediately for work on behalf of your company.

## Violations

The first time you are notified in writing that you have
violated any of these terms, or done anything with the software
not covered by your licenses, your licenses can nonetheless
continue if you come into full compliance with these terms,
and take practical steps to correct past violations, within
32 days of receiving notice.  Otherwise, all your licenses
end immediately.

## No Liability

***As far as the law allows, the software comes as is, without
any warranty or condition, and the licensor will not be liable
to you for any damages arising out of these terms or the use
or nature of the software, under any kind of legal claim.***

## Definitions

The **licensor** is the individual or entity offering these
terms, and the **software** is the software the licensor makes
available under these terms.

**You** refers to the individual or entity agreeing to these
terms.

**Your company** is any legal entity, sole proprietorship,
or other kind of organization that you work for, plus all
organizations that have control over, are under the control of,
or are under common control with that organization.  **Control**
means ownership of substantially all the assets of an entity,
or the power to direct its management and policies by vote,
contract, or otherwise.  Control can be direct or indirect.

**Your licenses** are all the licenses granted to you for the
software under these terms.

**Use** means anything you do with the software requiring one
of your licenses.LICFIN
cat > /usr/share/doc/flashos/AUTHORS <<'AUT'
SoleOS
Créé par Giuseppe D'Arco
© 2026 GDH

Les programmes SoleOS (accueil, applications, installateur, outils système, fonds d'écran) sont
distribués sous licence non commerciale (voir LICENSE) : gratuits pour les particuliers, les
associations, les écoles, les administrations et l'usage interne des entreprises ; toute
utilisation commerciale demande une licence de GDH.
SoleOS est basé sur Debian GNU/Linux ; chaque logiciel inclus garde sa propre licence
(voir /usr/share/doc/<logiciel>/copyright).
Icônes de la suite Craft : © ArtCraft Team, licences MIT / Apache-2.0.
AUT
update-initramfs -u -k all
# Démarrage plus rapide sur la clé : bibliothèques et caches déjà à jour dans l'image
# (sinon ldconfig et consorts tournent à chaque démarrage : environ 3 s sur clé USB)
ldconfig
touch /etc/.updated /var/.updated
FIN
chmod 755 "$C/hooks/normal/9000-flashos.hook.chroot"

# --- 7. Construction ----------------------------------------------------------
echo
echo "==> Construction de l'ISO SoleOS. Comptez 30 à 60 minutes (téléchargements compris)."
echo "    Journal complet : $ATELIER/construction.log"
if ! lb build 2>&1 | tee "$ATELIER/construction.log"; then
  echo "!!  La construction a échoué. Les dernières lignes du journal :"; tail -25 "$ATELIER/construction.log"; exit 1
fi

ISO="$(ls -1 "$ATELIER"/flashos*.iso 2>/dev/null | head -1)"
if [ -z "$ISO" ]; then
  echo "!!  Aucune ISO produite. Les dernières lignes du journal :"; tail -25 "$ATELIER/construction.log"; exit 1
fi
TAILLE_GO=$(( $(du -BG "$ISO" | cut -f1 | tr -dc '0-9') + 1 ))
if [ "$(place_libre "$MAISON")" -gt "$TAILLE_GO" ]; then
  cp "$ISO" "$MAISON/SoleOS-$VERSION_FLASHOS.iso"; chown "$UTILISATEUR:" "$MAISON/SoleOS-$VERSION_FLASHOS.iso"; ISO="$MAISON/SoleOS-$VERSION_FLASHOS.iso"
fi
# Empreinte SHA-256 : permet de vérifier que l'ISO n'a pas été modifiée (à publier avec elle)
(cd "$(dirname "$ISO")" && sha256sum "$(basename "$ISO")" > "$(basename "$ISO").sha256")
echo
echo "==> Terminé : $ISO ($(du -h "$ISO" | cut -f1))"
echo "    Empreinte SHA-256 : $(cut -d' ' -f1 "$ISO.sha256")"
echo "    Récupérez-le avec votre outil SCP, puis testez-le dans une nouvelle VM ou gravez-le avec Rufus."
