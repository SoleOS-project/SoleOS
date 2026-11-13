//! SoleOS – « Remettre le navigateur à neuf » (publicités, pages qui s'ouvrent seules…).
//!
//! Les causes habituelles : notifications acceptées sur un site douteux, extension
//! malveillante, page d'accueil ou moteur de recherche détournés. On repart d'un profil
//! Chromium neuf en gardant favoris, mots de passe, historique et connexions aux sites.
//! L'ancien profil est gardé 30 jours (dossier caché), au cas où.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};
use crate::tr::{tr, trv};

/// Fichiers repris dans le nouveau profil (chemins relatifs au dossier « Default »)
const GARDES: &[&str] = &[
    "Bookmarks",
    "Login Data",
    "Login Data-journal",
    "Login Data For Account",
    "History",
    "Favicons",
    "Top Sites",
    "Network/Cookies",
    "Network/Cookies-journal",
];

const JOURS_SAUVEGARDE: u64 = 30;

fn dossier_chromium() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("chromium"))
}

/// Ce qui peut produire des publicités : (extensions installées, sites autorisés à envoyer des notifications)
pub fn etat() -> (usize, usize) {
    let Some(d) = dossier_chromium() else { return (0, 0) };
    let profil = d.join("Default");
    let extensions = std::fs::read_dir(profil.join("Extensions"))
        .map(|l| l.flatten().filter(|e| e.file_name() != "Temp" && e.path().is_dir()).count())
        .unwrap_or(0);
    let preferences = std::fs::read_to_string(profil.join("Preferences")).unwrap_or_default();
    (extensions, notifications_autorisees(&preferences))
}

/// Nombre de sites autorisés (« setting »: 1) dans profile.content_settings.exceptions.notifications
pub fn notifications_autorisees(preferences: &str) -> usize {
    let Some(debut) = preferences.find("\"notifications\":{") else { return 0 };
    let texte = &preferences[debut + "\"notifications\":".len()..];
    // Fin de l'objet : accolade correspondante
    let mut profondeur = 0i32;
    let mut fin = texte.len();
    for (i, c) in texte.char_indices() {
        match c {
            '{' => profondeur += 1,
            '}' => {
                profondeur -= 1;
                if profondeur == 0 {
                    fin = i;
                    break;
                }
            }
            _ => {}
        }
    }
    texte[..fin].matches("\"setting\":1").count()
}

fn copier(source: &Path, cible: &Path) {
    if source.is_file() {
        if let Some(p) = cible.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::copy(source, cible);
    }
}

/// Préférences du profil neuf : celles de SoleOS (page d'accueil, barre de favoris, barre de titre)
fn preferences_neuves() -> String {
    // Page d'accueil selon la langue (Bing en chinois : Google n'est pas joignable partout)
    let web = crate::tr::courante().web;
    r#"{"browser":{"custom_chrome_frame":false,"check_default_browser":false,"show_home_button":true},"homepage":"WEB","homepage_is_newtabpage":false,"session":{"restore_on_startup":4,"startup_urls":["WEB"]},"bookmark_bar":{"show_on_all_tabs":true},"performance_tuning":{"high_efficiency_mode":{"state":2}}}"#.replace("WEB", web)
}

/// Remet le navigateur à neuf. Renvoie un texte pour l'utilisateur.
pub fn remettre_a_neuf() -> Result<String, String> {
    let d = dossier_chromium().ok_or_else(|| tr("Dossier personnel introuvable."))?;
    // Le navigateur doit être fermé
    let _ = Command::new("pkill").args(["-u", &uid(), "-x", "chromium"]).status();
    for _ in 0..30 {
        let encore = Command::new("pgrep").args(["-u", &uid(), "-x", "chromium"]).status().map(|s| s.success()).unwrap_or(false);
        if !encore {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = Command::new("pkill").args(["-9", "-u", &uid(), "-x", "chromium"]).status();

    let parent = d.parent().ok_or_else(|| tr("Dossier introuvable."))?.to_path_buf();
    nettoyer_sauvegardes(&parent);
    if d.exists() {
        let horodatage = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0);
        let sauvegarde = parent.join(format!(".chromium-avant-remise-a-neuf-{horodatage}"));
        std::fs::rename(&d, &sauvegarde).map_err(|e| trv("Impossible de mettre l'ancien profil de côté ({e}).", &[("e", &e.to_string())]))?;
        let ancien = sauvegarde.join("Default");
        let neuf = d.join("Default");
        std::fs::create_dir_all(&neuf).map_err(|e| e.to_string())?;
        for f in GARDES {
            copier(&ancien.join(f), &neuf.join(f));
        }
        std::fs::write(neuf.join("Preferences"), preferences_neuves()).map_err(|e| e.to_string())?;
        // Pas d'écrans de premier lancement ni de nouvel import des favoris SoleOS
        let _ = std::fs::write(d.join("First Run"), "");
    }
    Ok(tr("Navigateur remis à neuf : extensions, notifications des sites et réglages effacés. Favoris, mots de passe et historique sont gardés."))
}

fn nettoyer_sauvegardes(parent: &Path) {
    let Ok(liste) = std::fs::read_dir(parent) else { return };
    let limite = Duration::from_secs(JOURS_SAUVEGARDE * 24 * 3600);
    for e in liste.flatten() {
        if !e.file_name().to_string_lossy().starts_with(".chromium-avant-remise-a-neuf-") {
            continue;
        }
        let vieux = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|a| a > limite).unwrap_or(false);
        if vieux {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

fn uid() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("Uid:")).and_then(|l| l.split_whitespace().nth(1).map(str::to_string)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compte_les_notifications() {
        let p = r#"{"profile":{"content_settings":{"exceptions":{"notifications":{"https://pub.example,*":{"last_modified":"1","setting":1},"https://a.fr,*":{"setting":2},"https://b.example,*":{"setting":1}},"popups":{"x":{"setting":1}}}}}}"#;
        assert_eq!(notifications_autorisees(p), 2);
        assert_eq!(notifications_autorisees("{}"), 0);
    }
}
