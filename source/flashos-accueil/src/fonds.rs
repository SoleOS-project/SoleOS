//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Fonds d'écran de l'accueil : fonds SoleOS (images fournies), photo personnelle,
//! miniatures pour la page « Fond d'écran » des Réglages.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gdk, gdk_pixbuf, glib};
use crate::tr::{n_, tr};

/// Un fond SoleOS : identifiant, nom affiché
pub struct Fond {
    pub id: &'static str,
    pub nom: &'static str,
}

pub const FONDS: &[Fond] = &[
    // « eclair » : identifiant gardé (choix déjà enregistrés) ; le fond est le Soleil de SoleOS
    Fond { id: "eclair", nom: n_("Soleil") },
    Fond { id: "circuit", nom: n_("Circuit") },
    Fond { id: "synapse", nom: n_("Synapse") },
    Fond { id: "onde", nom: n_("Onde") },
    Fond { id: "aurore", nom: n_("Aurore") },
    Fond { id: "nuit", nom: n_("Nuit (uni)") },
];

/// Fond choisi à la première ouverture
pub const PAR_DEFAUT: &str = "eclair";

const DOSSIER: &str = "/usr/share/flashos/fonds";

fn dossier_systeme() -> PathBuf {
    std::env::var_os("FLASHOS_FONDS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DOSSIER))
}

/// Image d'un fond SoleOS (None pour « Nuit », dessiné par un dégradé)
pub fn image(id: &str) -> Option<PathBuf> {
    let p = dossier_systeme().join(format!("flashos-{id}.jpg"));
    p.is_file().then_some(p)
}

/// Petite image d'un fond SoleOS (pour la page des Réglages)
pub fn vignette(id: &str) -> Option<PathBuf> {
    let p = dossier_systeme().join("vignettes").join(format!("flashos-{id}.jpg"));
    if p.is_file() { Some(p) } else { image(id) }
}

fn dossier_reglages() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("flashos"))
}

/// Identifiant du fond choisi : un fond SoleOS ou « perso »
pub fn choisi() -> String {
    let nom = dossier_reglages()
        .and_then(|d| std::fs::read_to_string(d.join("fond")).ok())
        .unwrap_or_default();
    let nom = nom.trim();
    if nom == "perso" && perso().is_some() {
        return "perso".into();
    }
    // Anciens noms (jusqu'à la v10) : « fond-nuit », « fond-aube »…
    let nom = match nom {
        "fond-nuit" => "nuit",
        n if n.starts_with("fond-") => PAR_DEFAUT,
        n => n,
    };
    if FONDS.iter().any(|f| f.id == nom) && (nom == "nuit" || image(nom).is_some()) {
        nom.to_string()
    } else if image(PAR_DEFAUT).is_some() {
        PAR_DEFAUT.into()
    } else {
        "nuit".into()
    }
}

pub fn enregistrer(id: &str) {
    if let Some(d) = dossier_reglages() {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("fond"), id);
    }
}

/// Photo personnelle (copie dans ~/.config/flashos, « fond-perso-<moment>.<ext> »)
pub fn perso() -> Option<PathBuf> {
    let d = dossier_reglages()?;
    let mut liste: Vec<PathBuf> = std::fs::read_dir(&d)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("fond-perso-")).unwrap_or(false))
        .filter(|p| p.is_file())
        .collect();
    liste.sort();
    liste.pop()
}

/// Copie une photo comme fond personnel (l'ancienne est supprimée)
pub fn importer(source: &Path) -> Result<PathBuf, String> {
    let d = dossier_reglages().ok_or_else(|| tr("Dossier personnel introuvable."))?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let ext = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "jpg".into());
    let dest = d.join(format!("fond-perso-{t}.{ext}"));
    let ancienne = perso();
    std::fs::copy(source, &dest).map_err(|_| tr("La photo n'a pas pu être copiée."))?;
    if let Some(a) = ancienne.filter(|a| *a != dest) {
        let _ = std::fs::remove_file(a);
    }
    Ok(dest)
}

/// Style CSS de la fenêtre d'accueil pour un fond donné
pub fn style(id: &str) -> String {
    let base = "window.accueil { background-color: #10172a; background-size: cover; background-position: center;";
    if id == "perso" {
        if let Some(p) = perso() {
            let uri = gtk::gio::File::for_path(&p).uri();
            // Photo assombrie, avec un voile plus marqué au centre (heure, recherche et tuiles lisibles
            // même sur une photo très claire)
            return format!(
                "{base} background-image: radial-gradient(ellipse at 50% 48%, alpha(black, 0.42), alpha(black, 0.10) 70%), \
                 linear-gradient(alpha(black, 0.18), alpha(black, 0.38)), url(\"{uri}\"); }}"
            );
        }
    }
    if let Some(p) = image(id) {
        let uri = gtk::gio::File::for_path(&p).uri();
        return format!("{base} background-image: url(\"{uri}\"); }}");
    }
    format!("{base} background-image: radial-gradient(circle at 30% 20%, #26375f, #10172a 70%); }}")
}

/// Applique un fond à l'écran d'accueil (feuille de style dédiée, remplacée à chaque changement)
pub fn appliquer(id: &str) {
    thread_local! { static FOURNI: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) }; }
    let css = style(id);
    FOURNI.with(|f| {
        let mut f = f.borrow_mut();
        let fournisseur = f.get_or_insert_with(|| {
            let fp = gtk::CssProvider::new();
            if let Some(ecran) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(&ecran, &fp, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
            }
            fp
        });
        #[allow(deprecated)]
        fournisseur.load_from_data(&css);
    });
}

fn est_image(p: &Path) -> bool {
    matches!(
        p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" | "tif" | "tiff" | "avif" | "heic" | "heif")
    )
}

/// Dossier « Images » de l'utilisateur
pub fn dossier_images() -> Option<PathBuf> {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Images")))
        .filter(|d| d.is_dir())
}

/// Photos du dossier Images (et de ses sous-dossiers directs), les plus récentes d'abord
pub fn mes_images(max: usize) -> Vec<PathBuf> {
    let Some(racine) = dossier_images() else { return Vec::new() };
    let mut trouvees: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let mut dossiers = vec![(racine, 0)];
    while let Some((d, profondeur)) = dossiers.pop() {
        let Ok(entrees) = std::fs::read_dir(&d) else { continue };
        for e in entrees.flatten() {
            let p = e.path();
            let nom = e.file_name().to_string_lossy().to_string();
            if nom.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                if profondeur < 1 {
                    dossiers.push((p, profondeur + 1));
                }
            } else if est_image(&p) {
                let date = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                trouvees.push((date, p));
            }
        }
    }
    trouvees.sort_by(|a, b| b.0.cmp(&a.0));
    trouvees.into_iter().take(max).map(|(_, p)| p).collect()
}

/// Miniature d'une photo (fichier PNG dans ~/.cache/flashos/miniatures), créée si besoin.
/// Peut être appelée hors du fil principal.
pub fn miniature(source: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?
        .join("flashos")
        .join("miniatures");
    let date = std::fs::metadata(source).and_then(|m| m.modified()).ok();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    date.hash(&mut h);
    let dest = cache.join(format!("{:016x}.png", h.finish()));
    if dest.is_file() {
        return Some(dest);
    }
    std::fs::create_dir_all(&cache).ok()?;
    let pb = gdk_pixbuf::Pixbuf::from_file_at_scale(source, 320, 180, true).ok()?;
    // Photos d'appareil : orientation EXIF respectée
    let pb = pb.apply_embedded_orientation().unwrap_or(pb);
    pb.savev(&dest, "png", &[]).ok()?;
    Some(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anciens_noms_et_style() {
        assert!(style("nuit").contains("radial-gradient"));
        assert!(est_image(Path::new("/a/b.JPG")));
        assert!(!est_image(Path::new("/a/b.txt")));
    }
}
