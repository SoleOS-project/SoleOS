//! État de la machine : réseau, batterie, son, alimentation.
//! Tout est lu dans les fichiers du noyau (/sys, /proc) ou via des outils
//! standards (wpctl pour le son, systemctl pour l'alimentation).

use std::fs;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Reseau {
    /// Wi-Fi connecté, qualité du signal en %.
    Wifi(u8),
    /// Câble réseau branché.
    Filaire,
    /// Aucune connexion.
    Aucun,
}

/// Wi-Fi allumé ? (None : pas de gestionnaire de réseau)
pub fn wifi_actif() -> Option<bool> {
    let sortie = std::process::Command::new("nmcli").args(["radio", "wifi"]).env("LC_ALL", "C.UTF-8").output().ok()?;
    if !sortie.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&sortie.stdout).trim() == "enabled")
}

/// Allume ou éteint le Wi-Fi
pub fn allumer_wifi(oui: bool) {
    let _ = std::process::Command::new("nmcli").args(["radio", "wifi", if oui { "on" } else { "off" }]).status();
}

pub fn reseau() -> Reseau {
    // Wi-Fi : /proc/net/wireless liste les cartes et la qualité du lien.
    if let Ok(contenu) = fs::read_to_string("/proc/net/wireless") {
        for ligne in contenu.lines().skip(2) {
            let champs: Vec<&str> = ligne.split_whitespace().collect();
            if champs.len() > 2 {
                let lien: f32 = champs[2].trim_end_matches('.').parse().unwrap_or(0.0);
                let carte = champs[0].trim_end_matches(':');
                if lien > 0.0 && etat_carte(carte) == "up" {
                    return Reseau::Wifi(((lien / 70.0) * 100.0).clamp(0.0, 100.0) as u8);
                }
            }
        }
    }
    // Câble : une carte non Wi-Fi active.
    if let Ok(cartes) = fs::read_dir("/sys/class/net") {
        for carte in cartes.flatten() {
            let nom = carte.file_name().to_string_lossy().to_string();
            if nom == "lo" || carte.path().join("wireless").exists() {
                continue;
            }
            let virtuelle = fs::read_link(carte.path())
                .map(|c| c.to_string_lossy().contains("/virtual/"))
                .unwrap_or(false);
            if !virtuelle && etat_carte(&nom) == "up" {
                return Reseau::Filaire;
            }
        }
    }
    Reseau::Aucun
}

fn etat_carte(nom: &str) -> String {
    fs::read_to_string(format!("/sys/class/net/{nom}/operstate"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Niveau de batterie en % et « en charge », ou None sur un PC fixe.
pub fn batterie() -> Option<(u8, bool)> {
    // Bancs d'essai : batterie simulée (SOLEOS_TEST_BATTERIE=71)
    if let Some(n) = std::env::var("SOLEOS_TEST_BATTERIE").ok().and_then(|v| v.parse::<u8>().ok()) {
        return Some((n.min(100), std::env::var_os("SOLEOS_TEST_SECTEUR").is_some()));
    }
    // Chargeur branché (v1.0.1) : aussi quand la batterie est pleine ou que le PC limite la
    // charge (« Not charging »), et d'après le chargeur lui-même (secteur, USB-C)
    batterie_dans(std::path::Path::new("/sys/class/power_supply"))
}

fn batterie_dans(dossier: &std::path::Path) -> Option<(u8, bool)> {
    let mut secteur = false;
    let mut batterie = None;
    for source in fs::read_dir(dossier).ok()?.flatten() {
        let p = source.path();
        let type_ = fs::read_to_string(p.join("type")).unwrap_or_default();
        match type_.trim() {
            "Mains" | "USB" => {
                if fs::read_to_string(p.join("online")).map(|o| o.trim() == "1").unwrap_or(false) {
                    secteur = true;
                }
            }
            "Battery" if batterie.is_none() => {
                // (batterie de souris ou de casque Bluetooth : « scope = Device », ignorée)
                if fs::read_to_string(p.join("scope")).map(|s| s.trim() == "Device").unwrap_or(false) {
                    continue;
                }
                let Some(niveau) = fs::read_to_string(p.join("capacity")).ok().and_then(|c| c.trim().parse::<u8>().ok()) else { continue };
                let statut = fs::read_to_string(p.join("status")).unwrap_or_default();
                batterie = Some((niveau.min(100), matches!(statut.trim(), "Charging" | "Full" | "Not charging")));
            }
            _ => {}
        }
    }
    batterie.map(|(n, c)| (n, c || secteur))
}

/// Volume (0 à 100) et « coupé », ou None si le son n'est pas disponible.
pub fn volume() -> Option<(u8, bool)> {
    let sortie = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).env("LC_ALL", "C.UTF-8").output().ok()?;
    if !sortie.status.success() {
        return None;
    }
    // Exemple : « Volume: 0.40 [MUTED] »
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let valeur: f32 = texte.split_whitespace().nth(1)?.parse().ok()?;
    Some(((valeur * 100.0).round().clamp(0.0, 150.0) as u8, texte.contains("MUTED")))
}

pub fn regler_volume(pourcent: u8) {
    let _ = Command::new("wpctl")
        .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", &format!("{pourcent}%")])
        .status();
}

pub fn couper_son(couper: bool) {
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", if couper { "1" } else { "0" }])
        .status();
}

/// Batterie présente : chargeur branché ? (None sur un PC fixe)
pub fn secteur() -> Option<bool> {
    let mut batterie = false;
    let mut branche = false;
    for source in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = source.path();
        let type_ = fs::read_to_string(p.join("type")).unwrap_or_default();
        match type_.trim() {
            "Battery" => {
                if fs::read_to_string(p.join("scope")).map(|s| s.trim() != "Device").unwrap_or(true) {
                    batterie = true;
                }
            }
            "Mains" | "USB" | "USB_C" | "USB_PD" => {
                if fs::read_to_string(p.join("online")).map(|s| s.trim() == "1").unwrap_or(false) {
                    branche = true;
                }
            }
            _ => {}
        }
    }
    batterie.then_some(branche)
}

fn lire_nombre(p: &std::path::Path, nom: &str) -> Option<f64> {
    fs::read_to_string(p.join(nom)).ok()?.trim().parse().ok()
}

/// Consommation (en watts) et autonomie restante estimée (en heures, sur batterie)
pub fn consommation() -> Option<(f64, Option<f64>)> {
    for source in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = source.path();
        if fs::read_to_string(p.join("type")).unwrap_or_default().trim() != "Battery" {
            continue;
        }
        if fs::read_to_string(p.join("scope")).map(|s| s.trim() == "Device").unwrap_or(false) {
            continue;
        }
        let tension = lire_nombre(&p, "voltage_now").map(|v| v / 1e6);
        // power_now (µW), sinon courant (µA) × tension (µV)
        let watts = lire_nombre(&p, "power_now")
            .map(|w| w / 1e6)
            .or_else(|| Some(lire_nombre(&p, "current_now")? / 1e6 * tension?))?;
        if watts <= 0.0 {
            return Some((0.0, None));
        }
        let decharge = fs::read_to_string(p.join("status")).map(|s| s.trim() == "Discharging").unwrap_or(false);
        // Énergie restante : energy_now (µWh), sinon charge_now (µAh) × tension
        let energie = lire_nombre(&p, "energy_now")
            .map(|e| e / 1e6)
            .or_else(|| Some(lire_nombre(&p, "charge_now")? / 1e6 * tension?));
        let heures = if decharge { energie.map(|e| e / watts).filter(|h| *h < 48.0) } else { None };
        return Some((watts, heures));
    }
    None
}

fn fichier_option_luminosite() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("soleos").join("luminosite-batterie"))
}

/// Option « baisser la luminosité sur batterie »
pub fn option_luminosite() -> bool {
    fichier_option_luminosite().map(|f| f.exists()).unwrap_or(false)
}

pub fn choisir_option_luminosite(oui: bool) {
    if let Some(f) = fichier_option_luminosite() {
        if oui {
            if let Some(d) = f.parent() {
                let _ = fs::create_dir_all(d);
            }
            let _ = fs::write(f, "oui\n");
        } else {
            let _ = fs::remove_file(f);
        }
    }
}

pub fn luminosite_actuelle() -> Option<u32> {
    // Seulement un vrai rétroéclairage d'écran (pas les voyants du clavier)
    if fs::read_dir("/sys/class/backlight").map(|mut d| d.next().is_none()).unwrap_or(true) {
        return None;
    }
    let o = Command::new("brightnessctl").args(["-m", "-c", "backlight"]).output().ok()?;
    // « intel_backlight,backlight,12000,63%,19200 »
    String::from_utf8_lossy(&o.stdout).lines().next()?.split(',').nth(3)?.trim_end_matches('%').parse().ok()
}

/// Chargeur débranché : luminosité baissée de 30 % ; rebranché : luminosité d'avant
pub fn luminosite_selon_alimentation(secteur: bool) {
    if !option_luminosite() {
        return;
    }
    let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") else { return };
    let memo = std::path::PathBuf::from(d).join("soleos-luminosite-avant");
    if secteur {
        if let Some(v) = fs::read_to_string(&memo).ok().and_then(|t| t.trim().parse::<u32>().ok()) {
            let _ = Command::new("brightnessctl").args(["-q", "set", &format!("{v}%")]).status();
        }
        let _ = fs::remove_file(&memo);
    } else if let Some(v) = luminosite_actuelle() {
        let _ = fs::write(&memo, v.to_string());
        let nouvelle = ((v as f32) * 0.7).round().max(10.0) as u32;
        let _ = Command::new("brightnessctl").args(["-q", "set", &format!("{nouvelle}%")]).status();
    }
}

/// Prénom donné à l'installation (champ « nom complet » du compte), s'il a été choisi
pub fn prenom() -> Option<String> {
    if let Ok(p) = std::env::var("SOLEOS_TEST_PRENOM") {
        return Some(p).filter(|p| !p.is_empty());
    }
    let nom = gtk::glib::real_name().to_string_lossy().split(',').next().unwrap_or("").trim().to_string();
    let compte = gtk::glib::user_name().to_string_lossy().to_string();
    // Compte pas encore personnalisé : « SoleOS » (clé USB, avant l'assistant de bienvenue)
    if nom.is_empty() || nom == "Unknown" || nom.eq_ignore_ascii_case(&compte) || nom.eq_ignore_ascii_case("soleos") || nom.eq_ignore_ascii_case("soleos") {
        return None;
    }
    Some(nom)
}

/// Luminosité de l'écran intégré (5 % au minimum : l'écran ne devient jamais noir)
pub fn regler_luminosite(pourcent: u32) {
    let v = pourcent.clamp(5, 100);
    let _ = Command::new("brightnessctl").args(["-q", "-c", "backlight", "set", &format!("{v}%")]).status();
}

/// Une carte Bluetooth est présente
pub fn bluetooth_present() -> bool {
    fs::read_dir("/sys/class/bluetooth").map(|mut d| d.next().is_some()).unwrap_or(false)
}

/// Mode nuit (v13.1) : écran plus chaud (4 000 K) tant qu'il est activé ; choix gardé ici
fn fichier_mode_nuit() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("soleos/mode-nuit"))
}

pub fn mode_nuit() -> bool {
    fichier_mode_nuit().map(|f| f.exists()).unwrap_or(false)
}

fn lancer_wlsunset() {
    // Jour 4 001 K, nuit 4 000 K : la même couleur chaude à toute heure. Léger (un petit
    // programme qui ne tourne que si le mode nuit est activé).
    if let Ok(mut e) = Command::new("wlsunset").args(["-t", "4000", "-T", "4001", "-S", "06:00", "-s", "18:00"]).spawn() {
        std::thread::spawn(move || {
            let _ = e.wait();
        });
    }
}

pub fn choisir_mode_nuit(oui: bool) {
    let Some(f) = fichier_mode_nuit() else { return };
    let _ = Command::new("pkill").args(["-x", "wlsunset"]).status();
    if oui {
        if let Some(d) = f.parent() {
            let _ = fs::create_dir_all(d);
        }
        let _ = fs::write(&f, "oui\n");
        lancer_wlsunset();
    } else {
        let _ = fs::remove_file(&f);
    }
}

/// Ouverture de session : le mode nuit reprend s'il était activé
pub fn mode_nuit_au_demarrage() {
    let deja = Command::new("pgrep").args(["-x", "wlsunset"]).output().map(|o| o.status.success()).unwrap_or(false);
    if mode_nuit() && !deja {
        lancer_wlsunset();
    }
}

/// Mode grand confort (v1) : textes plus grands (125 %) et curseur plus grand, pour toutes les
/// applications ; choix gardé ici
fn fichier_grand_confort() -> Option<std::path::PathBuf> {
    fichier_mode_nuit().map(|f| f.with_file_name("grand-confort"))
}

pub fn grand_confort() -> bool {
    fichier_grand_confort().map(|f| f.exists()).unwrap_or(false)
}

pub fn choisir_grand_confort(oui: bool) {
    if let Some(f) = fichier_grand_confort() {
        if oui {
            if let Some(d) = f.parent() {
                let _ = fs::create_dir_all(d);
            }
            let _ = fs::write(&f, "oui\n");
        } else {
            let _ = fs::remove_file(&f);
        }
    }
    let (texte, curseur) = if oui { ("1.25", "40") } else { ("1.0", "24") };
    let _ = Command::new("gsettings").args(["set", "org.gnome.desktop.interface", "text-scaling-factor", texte]).status();
    let _ = Command::new("gsettings").args(["set", "org.gnome.desktop.interface", "cursor-size", curseur]).status();
}

#[cfg(test)]
mod tests_batterie {
    use super::batterie_dans;
    use std::fs;

    fn source(d: &std::path::Path, nom: &str, fichiers: &[(&str, &str)]) {
        let p = d.join(nom);
        fs::create_dir_all(&p).unwrap();
        for (f, v) in fichiers {
            fs::write(p.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn chargeur_branche() {
        let d = std::env::temp_dir().join(format!("soleos-bat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        source(&d, "BAT0", &[("type", "Battery"), ("capacity", "82"), ("status", "Discharging")]);
        source(&d, "ACAD", &[("type", "Mains"), ("online", "0")]);
        source(&d, "hid-souris", &[("type", "Battery"), ("capacity", "5"), ("status", "Discharging"), ("scope", "Device")]);
        assert_eq!(batterie_dans(&d), Some((82, false)));
        // Branché, batterie qui ne charge pas (pleine à 82 %, limite de charge) : en charge quand même
        source(&d, "ACAD", &[("type", "Mains"), ("online", "1")]);
        source(&d, "BAT0", &[("type", "Battery"), ("capacity", "82"), ("status", "Not charging")]);
        assert_eq!(batterie_dans(&d), Some((82, true)));
        // Chargeur USB-C seul
        source(&d, "ACAD", &[("type", "Mains"), ("online", "0")]);
        source(&d, "ucsi", &[("type", "USB"), ("online", "1")]);
        source(&d, "BAT0", &[("type", "Battery"), ("capacity", "40"), ("status", "Discharging")]);
        assert_eq!(batterie_dans(&d), Some((40, true)));
        let _ = fs::remove_dir_all(&d);
    }
}
