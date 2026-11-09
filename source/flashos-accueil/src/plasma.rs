//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS v13 (bureau KDE Plasma) : ce que les Réglages SoleOS confient à Plasma.
//!
//! Wi-Fi, son, Bluetooth, écran, imprimantes, batterie : les modules de réglage de Plasma
//! (ouverts dans leur propre fenêtre). Fond d'écran, clavier, langue, délai de verrouillage :
//! écrits dans la configuration de Plasma, appliqués tout de suite quand c'est possible.

use std::process::Command;

/// Session Plasma ? (FLASHOS_PLASMA=1 pour les essais)
pub fn actif() -> bool {
    if std::env::var_os("FLASHOS_PLASMA").is_some() {
        return true;
    }
    std::env::var("XDG_CURRENT_DESKTOP").map(|d| d.to_uppercase().contains("KDE")).unwrap_or(false)
}

/// Module de réglage de Plasma pour une carte des Réglages SoleOS
pub fn module_pour(carte: &str) -> Option<&'static str> {
    Some(match carte {
        "wifi" => "kcm_networkmanagement",
        "son" => "kcm_pulseaudio",
        "bluetooth" => "kcm_bluetooth",
        "ecran" | "taille" => "kcm_kscreen",
        "imprimantes" => "kcm_printer_manager",
        "batterie" => "kcm_powerdevilprofilesconfig",
        _ => return None,
    })
}

/// Ouvre un module de réglage de Plasma dans sa fenêtre
pub fn ouvrir_module(module: &str) {
    if std::env::var_os("FLASHOS_DEBUG").is_some() {
        eprintln!("plasma : module {module}");
    }
    let programme = if commande_existe("kcmshell6") { "kcmshell6" } else { "systemsettings" };
    lancer(Command::new(programme).arg(module));
}

fn commande_existe(nom: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(nom).is_file()))
        .unwrap_or(false)
}

/// Lance sans attendre (le processus est récupéré dans un fil pour ne pas laisser de zombie)
fn lancer(c: &mut Command) {
    if let Ok(mut enfant) = c.spawn() {
        std::thread::spawn(move || enfant.wait());
    }
}

fn ecrire(fichier: &str, groupe: &str, cle: &str, valeur: &str) {
    let _ = Command::new("kwriteconfig6")
        .args(["--file", fichier, "--group", groupe, "--key", cle, valeur])
        .status();
}

/// Fond d'écran du bureau (image)
pub fn fond(chemin: &std::path::Path) {
    lancer(Command::new("plasma-apply-wallpaperimage").arg(chemin));
    // Le même fond sur l'écran verrouillé
    let uri = format!("file://{}", chemin.display());
    let _ = Command::new("kwriteconfig6")
        .args(["--file", "kscreenlockerrc", "--group", "Greeter", "--group", "Wallpaper", "--group", "org.kde.image",
               "--group", "General", "--key", "Image", &uri])
        .status();
}

/// Clavier de la session : écrit dans Plasma et appliqué tout de suite
pub fn clavier(disposition: &str, variante: &str) {
    ecrire("kxkbrc", "Layout", "Use", "true");
    ecrire("kxkbrc", "Layout", "LayoutList", disposition);
    ecrire("kxkbrc", "Layout", "VariantList", variante);
    let _ = Command::new("dbus-send")
        .args(["--session", "--type=signal", "/Layouts", "org.kde.keyboard.reloadConfig"])
        .status();
}

/// Langue de Plasma (prise en compte à la prochaine ouverture de session)
pub fn langue(locale: &str, language: &str) {
    ecrire("plasma-localerc", "Formats", "LANG", locale);
    ecrire("plasma-localerc", "Translations", "LANGUAGE", language);
}

/// Verrouillage automatique après `minutes` sans activité (0 : jamais)
pub fn delai_verrouillage(minutes: u32) {
    ecrire("kscreenlockerrc", "Daemon", "Autolock", if minutes == 0 { "false" } else { "true" });
    if minutes > 0 {
        ecrire("kscreenlockerrc", "Daemon", "Timeout", &minutes.to_string());
    }
    ecrire("kscreenlockerrc", "Daemon", "LockOnResume", "true");
    // Le verrouillage relit sa configuration
    let _ = Command::new("dbus-send")
        .args(["--session", "--type=signal", "/ScreenSaver", "org.kde.screensaver.configChanged"])
        .status();
}

/// Verrouille l'écran tout de suite
pub fn verrouiller() {
    lancer(Command::new("loginctl").arg("lock-session"));
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn modules() {
        assert_eq!(module_pour("wifi"), Some("kcm_networkmanagement"));
        assert_eq!(module_pour("taille"), Some("kcm_kscreen"));
        assert_eq!(module_pour("fond"), None);
        assert_eq!(module_pour("langue"), None);
        assert_eq!(module_pour("securite"), None);
    }
}
