//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Sauvegarde en un clic (SoleOS v1) : les dossiers personnels (Documents, Images, Musique,
//! Vidéos, Téléchargements, Bureau) sont copiés vers une clé USB, un disque ou un dossier du NAS.
//! - « Sauvegarder maintenant », ou automatiquement chaque semaine quand la destination est là ;
//! - rien n'est jamais effacé dans la sauvegarde (un fichier supprimé par erreur s'y retrouve) ;
//! - « Restaurer » ouvre la sauvegarde dans Fichiers : on y reprend ce qu'on veut.
//! Rien ne tourne en permanence : un contrôle léger toutes les heures.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::glib;

/// Dossier créé dans la destination
pub const DOSSIER: &str = "Sauvegarde SoleOS";

fn config() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("flashos"))
}

/// Destination choisie (dossier où « Sauvegarde SoleOS » est créé)
pub fn destination() -> Option<PathBuf> {
    let t = std::fs::read_to_string(config()?.join("sauvegarde-destination")).ok()?;
    let t = t.trim();
    (!t.is_empty()).then(|| PathBuf::from(t))
}

pub fn choisir_destination(d: &Path) {
    if let Some(c) = config() {
        let _ = std::fs::create_dir_all(&c);
        let _ = std::fs::write(c.join("sauvegarde-destination"), d.to_string_lossy().as_bytes());
    }
}

pub fn automatique() -> bool {
    config().map(|c| c.join("sauvegarde-auto").exists()).unwrap_or(false)
}

pub fn choisir_automatique(oui: bool) {
    if let Some(c) = config() {
        let _ = std::fs::create_dir_all(&c);
        let f = c.join("sauvegarde-auto");
        if oui {
            let _ = std::fs::write(f, "oui\n");
        } else {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// Date de la dernière sauvegarde réussie (secondes depuis 1970)
pub fn derniere() -> Option<i64> {
    std::fs::read_to_string(config()?.join("sauvegarde-derniere")).ok()?.trim().parse().ok()
}

/// « mardi 4 novembre à 18:20 », dans la langue de l'ordinateur
pub fn derniere_texte() -> Option<String> {
    let t = derniere()?;
    let d = glib::DateTime::from_unix_local(t).ok()?;
    d.format(&crate::tr::tr("%A %-e %B à %H:%M")).ok().map(|s| s.to_string())
}

/// Dossiers personnels sauvegardés (ceux qui existent)
fn sources() -> Vec<PathBuf> {
    let maison = glib::home_dir();
    let mut v = Vec::new();
    for d in [
        glib::UserDirectory::Documents,
        glib::UserDirectory::Pictures,
        glib::UserDirectory::Music,
        glib::UserDirectory::Videos,
        glib::UserDirectory::Downloads,
        glib::UserDirectory::Desktop,
    ] {
        if let Some(p) = glib::user_special_dir(d) {
            if p != maison && p.is_dir() && !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

/// La destination est-elle là (clé branchée, NAS ouvert) et accessible en écriture ?
pub fn disponible() -> Option<PathBuf> {
    let d = destination()?;
    if !d.is_dir() {
        return None;
    }
    let essai = d.join(".soleos-essai");
    std::fs::write(&essai, b"ok").ok()?;
    let _ = std::fs::remove_file(essai);
    Some(d)
}

/// Copie (bloquante, à lancer en arrière-plan). Rien n'est effacé dans la sauvegarde.
pub fn sauvegarder() -> Result<(), String> {
    // Une seule sauvegarde à la fois (bouton et sauvegarde automatique)
    static EN_COURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if EN_COURS.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(crate::tr::tr("Une sauvegarde est déjà en cours."));
    }
    let r = sauvegarder_vraiment();
    EN_COURS.store(false, std::sync::atomic::Ordering::SeqCst);
    r
}

fn sauvegarder_vraiment() -> Result<(), String> {
    let d = disponible().ok_or_else(|| crate::tr::tr("La destination n'est pas disponible (clé débranchée ou NAS non ouvert ?)."))?;
    let cible = d.join(DOSSIER);
    std::fs::create_dir_all(&cible).map_err(|e| e.to_string())?;
    let mut erreurs = Vec::new();
    for s in sources() {
        let nom = s.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // -r -t : dossiers et dates (fonctionne aussi sur les clés USB en FAT/exFAT) ;
        // les liens (raccourcis du bureau) ne sont pas suivis : pas de copie en double
        let st = Command::new("rsync")
            .args(["-rt", "--modify-window=2", "--exclude=.cache", "--exclude=.Trash-*"])
            .arg(format!("{}/", s.display()))
            .arg(cible.join(&nom))
            .output();
        match st {
            Ok(o) if o.status.success() || o.status.code() == Some(24) => {}
            Ok(o) => erreurs.push(format!("{nom} : {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").trim())),
            Err(e) => erreurs.push(e.to_string()),
        }
    }
    if !erreurs.is_empty() {
        return Err(erreurs.join(" ; "));
    }
    if let Some(c) = config() {
        let _ = std::fs::write(c.join("sauvegarde-derniere"), glib::DateTime::now_utc().map(|d| d.to_unix()).unwrap_or(0).to_string());
    }
    Ok(())
}

/// Sauvegarde automatique due ? (activée, plus de 7 jours, destination présente)
pub fn due() -> bool {
    if !automatique() || disponible().is_none() {
        return false;
    }
    let maintenant = glib::DateTime::now_utc().map(|d| d.to_unix()).unwrap_or(0);
    derniere().map(|t| maintenant - t > 7 * 24 * 3600).unwrap_or(true)
}

/// Ouvre la sauvegarde dans Fichiers (pour reprendre un fichier)
pub fn ouvrir() {
    if let Some(d) = destination() {
        let p = d.join(DOSSIER);
        let chemin = if p.is_dir() { p } else { d };
        crate::lancer(&["nautilus", "--new-window", &chemin.to_string_lossy()]);
    }
}
