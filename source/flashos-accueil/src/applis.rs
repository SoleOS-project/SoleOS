//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Applications ajoutées par l'utilisateur (Flatpak et AppImage), sélection SoleOS,
//! applications épinglées sur l'accueil, et choix de la messagerie.
//! Tout est rangé dans le dossier personnel (partition des données) : le système reste en lecture seule.
//! Partagé entre l'accueil et le programme « Applications ».

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use crate::tr::{n_, tr, trv};

// =====================================================================
//  Emplacements
// =====================================================================

pub fn maison() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn config(nom: &str) -> PathBuf {
    maison().join(".config/flashos").join(nom)
}

fn ecrire(chemin: &Path, texte: &str) {
    if let Some(d) = chemin.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(chemin, texte);
}

// =====================================================================
//  Sélection SoleOS (applications vérifiées, toutes sur Flathub)
// =====================================================================

pub struct Proposee {
    pub id: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
    pub categorie: &'static str,
}

const fn p(id: &'static str, nom: &'static str, description: &'static str, categorie: &'static str) -> Proposee {
    Proposee { id, nom, description, categorie }
}

pub const CATEGORIES: &[&str] = &[
    n_("Suite Craft"),
    n_("Photos et création"),
    n_("Vidéo et musique"),
    n_("Communication"),
    n_("Documents"),
    n_("Travail"),
    n_("Sécurité et outils"),
    n_("Enfants et école"),
    n_("Jeux"),
    n_("Autres navigateurs"),
];

pub const SELECTION: &[Proposee] = &[
    p("org.gimp.GIMP", "GIMP", n_("Retoucher ses photos"), n_("Photos et création")),
    p("org.kde.krita", "Krita", n_("Dessin et peinture"), n_("Photos et création")),
    p("org.inkscape.Inkscape", "Inkscape", n_("Logos, affiches, dessins vectoriels"), n_("Photos et création")),
    p("org.gnome.Shotwell", "Shotwell", n_("Ranger et retrouver ses photos"), n_("Photos et création")),
    p("org.kde.kdenlive", "Kdenlive", n_("Montage vidéo"), n_("Vidéo et musique")),
    p("org.audacityteam.Audacity", "Audacity", n_("Enregistrer et modifier du son"), n_("Vidéo et musique")),
    p("com.spotify.Client", "Spotify", n_("Musique en ligne"), n_("Vidéo et musique")),
    p("us.zoom.Zoom", "Zoom", n_("Réunions en visio"), n_("Communication")),
    p("org.signal.Signal", "Signal", n_("Messagerie privée"), n_("Communication")),
    p("org.telegram.desktop", "Telegram", n_("Messagerie"), n_("Communication")),
    p("com.discordapp.Discord", "Discord", n_("Discussions et appels en groupe"), n_("Communication")),
    p("org.mozilla.Thunderbird", "Thunderbird", n_("Logiciel de messagerie (mails)"), n_("Communication")),
    p("org.kde.okular", "Okular", n_("Lire et annoter les PDF"), n_("Documents")),
    p("com.github.jeromerobert.pdfarranger", "PDF Arranger", n_("Fusionner, découper, réordonner des PDF"), n_("Documents")),
    p("com.calibre_ebook.calibre", "Calibre", n_("Bibliothèque de livres numériques"), n_("Documents")),
    p("com.github.johnfactotum.Foliate", "Foliate", n_("Liseuse de livres numériques"), n_("Documents")),
    p("org.remmina.Remmina", n_("Bureau à distance"), n_("Utiliser un PC ou un serveur Windows à distance"), n_("Travail")),
    p("com.bitwarden.desktop", "Bitwarden", n_("Coffre-fort de mots de passe"), n_("Sécurité et outils")),
    p("org.keepassxc.KeePassXC", "KeePassXC", n_("Mots de passe, sans compte en ligne"), n_("Sécurité et outils")),
    p("org.gnome.World.PikaBackup", "Pika Backup", n_("Sauvegarder ses fichiers sur un disque USB"), n_("Sécurité et outils")),
    p("org.gnome.Calculator", n_("Calculatrice"), n_("Calculs simples et scientifiques"), n_("Sécurité et outils")),
    p("org.kde.gcompris", "GCompris", n_("Activités éducatives de 2 à 10 ans"), n_("Enfants et école")),
    p("org.tuxpaint.Tuxpaint", "Tux Paint", n_("Dessin pour les enfants"), n_("Enfants et école")),
    p("org.geogebra.GeoGebra", "GeoGebra", n_("Mathématiques : géométrie, courbes"), n_("Enfants et école")),
    p("org.stellarium.Stellarium", "Stellarium", n_("Le ciel étoilé"), n_("Enfants et école")),
    p("com.valvesoftware.Steam", "Steam", n_("Boutique et bibliothèque de jeux"), n_("Jeux")),
    p("net.supertuxkart.SuperTuxKart", "SuperTuxKart", n_("Course de karts"), n_("Jeux")),
    p("org.gnome.Aisleriot", n_("Solitaire"), n_("Jeux de cartes"), n_("Jeux")),
    p("org.gnome.Sudoku", "Sudoku", n_("Grilles de sudoku"), n_("Jeux")),
    p("org.mozilla.firefox", "Firefox", n_("Navigateur web"), n_("Autres navigateurs")),
    p("com.google.Chrome", "Google Chrome", n_("Navigateur web de Google"), n_("Autres navigateurs")),
];

// =====================================================================
//  Suite Craft (ArtCraft) : applications libres (MIT / Apache-2.0) écrites en Rust,
//  installées depuis leurs versions publiées (fichiers AppImage sur GitHub)
// =====================================================================

/// (code, nom, description)
pub const CRAFT: &[(&str, &str, &str)] = &[
    ("wordcraft", "WordCraft", n_("Traitement de texte (fichiers Word .docx)")),
    ("gridcraft", "GridCraft", n_("Tableur (fichiers Excel .xlsx)")),
    ("deckcraft", "DeckCraft", n_("Présentations (diaporamas)")),
    ("pdfcraft", "PdfCraft", n_("Lire, organiser et protéger des PDF")),
    ("photocraft", "PhotoCraft", n_("Retouche photo avec calques")),
    ("lightcraft", "LightCraft", n_("Photothèque et développement RAW")),
    ("vectorcraft", "VectorCraft", n_("Dessin vectoriel, logos")),
    ("designcraft", "DesignCraft", n_("Mise en page et publication")),
    ("figcraft", "FigCraft", n_("Maquettes d'écrans et d'applis")),
    ("filmcraft", "FilmCraft", n_("Montage vidéo")),
    ("effectcraft", "EffectCraft", n_("Animations et effets visuels")),
    ("soundcraft", "SoundCraft", n_("Studio d'enregistrement audio")),
    ("cadcraft", "CADCraft", n_("Dessin technique (CAO)")),
];

/// Identifiant d'une appli Craft dans la boutique : « craft:wordcraft »
pub fn code_craft(id: &str) -> Option<&str> {
    id.strip_prefix("craft:").filter(|c| CRAFT.iter().any(|(x, _, _)| x == c))
}

fn icone_craft(code: &str) -> Option<PathBuf> {
    let f = PathBuf::from("/usr/share/flashos/craft").join(format!("{code}.png"));
    f.exists().then_some(f)
}

/// Dernière version publiée (« 0.11.0 ») : service de GitHub, sinon redirection de la page « latest »
fn version_craft(code: &str) -> Result<String, String> {
    let api = Command::new("curl")
        .args(["-fsSL", "-m", "30", "-H", "Accept: application/vnd.github+json"])
        .arg(format!("https://api.github.com/repos/storytold/{code}/releases/latest"))
        .output();
    if let Ok(o) = api {
        let t = String::from_utf8_lossy(&o.stdout);
        if let Some(debut) = t.find("\"tag_name\"") {
            let reste = &t[debut + 10..];
            if let Some(v) = reste.split('"').nth(1) {
                let v = v.trim_start_matches('v');
                if !v.is_empty() {
                    return Ok(v.to_string());
                }
            }
        }
    }
    let o = Command::new("curl")
        .args(["-sS", "-o", "/dev/null", "-m", "30", "-w", "%{redirect_url}"])
        .arg(format!("https://github.com/storytold/{code}/releases/latest"))
        .output()
        .map_err(|_| tr("curl est introuvable."))?;
    let url = String::from_utf8_lossy(&o.stdout).to_string();
    match url.rsplit_once("/tag/") {
        Some((_, v)) if !v.trim().is_empty() => Ok(v.trim().trim_start_matches('v').to_string()),
        _ => Err(tr("Pas de connexion à Internet : impossible de joindre GitHub.")),
    }
}

/// Télécharge et installe une appli Craft (AppImage rangée dans ~/Applications/<code>.AppImage)
pub fn installer_craft(code: &str) -> Result<(), String> {
    let dossier = dossier_appimages().join(".telechargements");
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let fichier = dossier.join(format!("{code}.AppImage"));
    let _ = std::fs::remove_file(&fichier);
    // Pour les tests : dossier local à la place de GitHub
    if let Some(depot) = std::env::var_os("FLASHOS_CRAFT_DEPOT") {
        std::fs::copy(PathBuf::from(depot).join(format!("{code}.AppImage")), &fichier)
            .map_err(|_| "Fichier de test introuvable.".to_string())?;
    } else {
        let v = version_craft(code)?;
        let adresse = format!("https://github.com/storytold/{code}/releases/download/v{v}/{code}-{v}-linux-x86_64.AppImage");
        let ok = Command::new("curl")
            .args(["-fsSL", "--retry", "2", "-m", "1800", "-o"])
            .arg(&fichier)
            .arg(&adresse)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&fichier);
            return Err(tr("Le téléchargement a échoué. Vérifiez la connexion à Internet et réessayez."));
        }
    }
    ajouter_appimage(&fichier).map(|_| ())
}

pub fn proposee(id: &str) -> Option<&'static Proposee> {
    SELECTION.iter().find(|p| p.id == id)
}

// =====================================================================
//  Applications installées
// =====================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum Genre {
    Flatpak,
    AppImage(PathBuf),
    /// Site web ajouté à l'accueil (s'ouvre dans sa propre fenêtre, sans barre d'adresse)
    Web(String),
    /// Raccourci créé par le navigateur (menu ⋮ → « Installer la page en tant qu'application »)
    Lanceur(PathBuf, Vec<String>),
    /// Livrée avec SoleOS (calculatrice…) : toujours là, ne se désinstalle pas
    Systeme(&'static str),
}

/// Applications livrées avec SoleOS : (identifiant, nom, commande, icône)
pub const APPLIS_SYSTEME: &[(&str, &str, &str, &str)] = &[
    ("org.gnome.Calculator", n_("Calculatrice"), "gnome-calculator", "/usr/share/icons/hicolor/scalable/apps/org.gnome.Calculator.svg"),
    ("org.gnome.Loupe", n_("Images"), "loupe", "/usr/share/icons/hicolor/scalable/apps/org.gnome.Loupe.svg"),
    ("vlc", "VLC", "vlc", "/usr/share/icons/hicolor/256x256/apps/vlc.png"),
    ("com.naps2.Naps2", n_("Numériser"), "naps2", "/usr/share/icons/hicolor/128x128/apps/com.naps2.Naps2.png"),
    // v1.0.2 : formater une clé USB (aussi dans Fichiers : clic droit sur la clé → Formater…)
    ("org.gnome.DiskUtility", n_("Disques"), "gnome-disks", "/usr/share/icons/hicolor/scalable/apps/org.gnome.DiskUtility.svg"),
];

#[derive(Clone, Debug)]
pub struct Installee {
    pub id: String,
    pub nom: String,
    pub icone: Option<PathBuf>,
    pub genre: Genre,
}

impl Installee {
    pub fn commande(&self) -> Vec<String> {
        match &self.genre {
            Genre::Flatpak => vec!["flatpak".into(), "run".into(), self.id.clone()],
            Genre::AppImage(f) => vec![f.to_string_lossy().to_string()],
            Genre::Systeme(c) => vec![c.to_string()],
            Genre::Web(adresse) => vec!["flashos-web".into(), format!("--app={adresse}")],
            Genre::Lanceur(_, c) => c.clone(),
        }
    }
    /// Morceaux permettant de reconnaître ses fenêtres (identifiant de fenêtre Wayland)
    pub fn motifs(&self) -> Vec<String> {
        // Fenêtres du navigateur en mode « appli » : « chrome-<site>__-Default », « chrome-<appli>-Default »
        match &self.genre {
            Genre::Web(adresse) => return vec![format!("chrome-{}", hote(adresse).to_lowercase())],
            Genre::Lanceur(fichier, c) => {
                if let Some(a) = c.iter().find_map(|x| x.strip_prefix("--app-id=")) {
                    return vec![format!("chrome-{}", a.to_lowercase()), format!("crx_{}", a.to_lowercase())];
                }
                // Application d'un conteneur : sa classe de fenêtre, sinon le nom du programme lancé
                let texte = std::fs::read_to_string(fichier).unwrap_or_default();
                let mut m: Vec<String> = texte
                    .lines()
                    .find_map(|l| l.strip_prefix("StartupWMClass="))
                    .map(|c| vec![c.trim().to_lowercase()])
                    .unwrap_or_default();
                if let Some(p) = c.iter().skip_while(|x| *x != "--").nth(1) {
                    if let Some(n) = Path::new(p).file_name() {
                        m.push(n.to_string_lossy().to_lowercase());
                    }
                }
                if !m.is_empty() {
                    return m;
                }
            }
            _ => {}
        }
        let mut m = vec![self.id.to_lowercase()];
        if let Some(dernier) = self.id.rsplit('.').next() {
            if dernier.len() >= 3 {
                m.push(dernier.to_lowercase());
            }
        }
        m
    }
}

/// Lit un fichier .desktop : (nom, icône, NoDisplay)
pub fn lire_desktop(texte: &str) -> (String, String, bool) {
    let mut nom = String::new();
    let mut nom_fr = String::new();
    let mut icone = String::new();
    let mut cache = false;
    let mut dans_entree = false;
    for l in texte.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            dans_entree = l == "[Desktop Entry]";
            continue;
        }
        if !dans_entree {
            continue;
        }
        if let Some(v) = l.strip_prefix("Name=") {
            nom = v.to_string();
        } else if let Some(v) = l.strip_prefix("Name[fr]=") {
            nom_fr = v.to_string();
        } else if let Some(v) = l.strip_prefix("Icon=") {
            icone = v.to_string();
        } else if l == "NoDisplay=true" {
            cache = true;
        }
    }
    (if nom_fr.is_empty() { nom } else { nom_fr }, icone, cache)
}

fn dossier_flatpak() -> PathBuf {
    maison().join(".local/share/flatpak")
}

/// Cherche l'icône d'une application dans les icônes exportées par Flatpak
pub fn icone_flatpak(nom_icone: &str) -> Option<PathBuf> {
    if nom_icone.is_empty() {
        return None;
    }
    if nom_icone.starts_with('/') {
        return Some(PathBuf::from(nom_icone)).filter(|p| p.exists());
    }
    let base = dossier_flatpak().join("exports/share/icons/hicolor");
    for taille in ["256x256", "128x128", "scalable", "512x512", "96x96", "64x64", "48x48"] {
        for ext in ["png", "svg"] {
            let f = base.join(taille).join("apps").join(format!("{nom_icone}.{ext}"));
            if f.exists() {
                return Some(f);
            }
        }
    }
    None
}

/// Icône d'une appli de la sélection, avant installation (catalogue téléchargé par Flatpak)
pub fn icone_catalogue(id: &str) -> Option<PathBuf> {
    if let Some(code) = code_craft(id) {
        return icone_craft(code);
    }
    let base = dossier_flatpak().join("appstream/flathub/x86_64/active/icons");
    for taille in ["128x128", "64x64"] {
        let f = base.join(taille).join(format!("{id}.png"));
        if f.exists() {
            return Some(f);
        }
    }
    None
}

fn dossier_appimages() -> PathBuf {
    maison().join("Applications")
}

fn infos_appimages() -> PathBuf {
    maison().join(".local/share/flashos/appimages")
}

pub fn installees() -> Vec<Installee> {
    let mut liste = Vec::new();
    // Flatpak (installation de l'utilisateur)
    // Une seule tuile par application : Calibre, par exemple, déclare aussi « E-book viewer »,
    // « E-book editor »… qu'on retrouve dans l'application elle-même.
    let applis_installees: Vec<String> = std::fs::read_dir(dossier_flatpak().join("app"))
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    let dossier = dossier_flatpak().join("exports/share/applications");
    if let Ok(entrees) = std::fs::read_dir(&dossier) {
        for e in entrees.flatten() {
            let nom_fichier = e.file_name().to_string_lossy().to_string();
            let Some(id) = nom_fichier.strip_suffix(".desktop") else { continue };
            if !applis_installees.iter().any(|a| a == id) {
                continue;
            }
            let texte = std::fs::read_to_string(e.path()).unwrap_or_default();
            let (nom, icone, cache) = lire_desktop(&texte);
            if cache || nom.is_empty() {
                continue;
            }
            // Nom français de la sélection s'il existe (ex. « Bureau à distance »)
            let nom = proposee(id).map(|p| p.nom.to_string()).unwrap_or(nom);
            liste.push(Installee { id: id.to_string(), nom, icone: icone_flatpak(&icone), genre: Genre::Flatpak });
        }
    }
    // AppImage
    if let Ok(entrees) = std::fs::read_dir(infos_appimages()) {
        for e in entrees.flatten() {
            let texte = std::fs::read_to_string(e.path()).unwrap_or_default();
            let champ = |c: &str| texte.lines().find_map(|l| l.strip_prefix(&format!("{c}="))).unwrap_or("").to_string();
            let fichier = PathBuf::from(champ("fichier"));
            if !fichier.exists() {
                continue;
            }
            let icone = PathBuf::from(champ("icone"));
            liste.push(Installee {
                id: e.file_name().to_string_lossy().to_string(),
                nom: champ("nom"),
                icone: Some(icone).filter(|i| i.exists()),
                genre: Genre::AppImage(fichier),
            });
        }
    }
    // Sites web ajoutés à l'accueil
    if let Ok(entrees) = std::fs::read_dir(infos_sites()) {
        for e in entrees.flatten() {
            let nom_fichier = e.file_name().to_string_lossy().to_string();
            if nom_fichier.ends_with(".png") {
                continue;
            }
            let texte = std::fs::read_to_string(e.path()).unwrap_or_default();
            let champ = |c: &str| texte.lines().find_map(|l| l.strip_prefix(&format!("{c}="))).unwrap_or("").to_string();
            let adresse = champ("adresse");
            if adresse.is_empty() {
                continue;
            }
            let icone = infos_sites().join(format!("{nom_fichier}.png"));
            liste.push(Installee {
                id: nom_fichier,
                nom: champ("nom"),
                icone: Some(icone).filter(|i| i.exists()),
                genre: Genre::Web(adresse),
            });
        }
    }
    // Raccourcis d'applications web créés par le navigateur
    if let Ok(entrees) = std::fs::read_dir(dossier_lanceurs()) {
        for e in entrees.flatten() {
            let nom_fichier = e.file_name().to_string_lossy().to_string();
            let Some(stem) = nom_fichier.strip_suffix(".desktop") else { continue };
            let texte = std::fs::read_to_string(e.path()).unwrap_or_default();
            // Raccourcis du navigateur (« chrome-… ») et applications d'un conteneur (Distrobox)
            let conteneur = texte.contains("distrobox-enter") || texte.contains("distrobox enter");
            if !stem.starts_with("chrome-") && !conteneur {
                continue;
            }
            let (nom, icone, cache) = lire_desktop(&texte);
            let Some(exec) = texte.lines().find_map(|l| l.strip_prefix("Exec=")) else { continue };
            // Console du conteneur (s'ouvre dans un terminal) : elle reste accessible par la Console
            if cache || nom.is_empty() || texte.lines().any(|l| l.trim() == "Terminal=true") {
                continue;
            }
            // « Gimp (on flashos-debian) » → « Gimp »
            let nom = match nom.rfind(" (on ") {
                Some(i) if conteneur && nom.ends_with(')') => nom[..i].to_string(),
                _ => nom,
            };
            let commande: Vec<String> = exec
                .split_whitespace()
                .filter(|m| !m.starts_with('%'))
                .map(|m| m.trim_matches('"').to_string())
                .collect();
            if commande.is_empty() {
                continue;
            }
            liste.push(Installee {
                id: stem.to_string(),
                nom,
                icone: icone_theme_utilisateur(&icone),
                genre: Genre::Lanceur(e.path(), commande),
            });
        }
    }
    // Livrées avec SoleOS (si présentes sur ce système)
    for (id, nom, commande, icone) in APPLIS_SYSTEME {
        let present = ["/usr/bin", "/bin"].iter().any(|d| std::path::Path::new(d).join(commande).exists());
        if present && !liste.iter().any(|a| a.id == *id) {
            liste.push(Installee {
                id: id.to_string(),
                nom: tr(nom),
                icone: Some(PathBuf::from(icone)).filter(|i| i.exists()),
                genre: Genre::Systeme(commande),
            });
        }
    }
    liste.sort_by_key(|a| a.nom.to_lowercase());
    liste
}

pub fn est_installee(id: &str) -> bool {
    if let Some(code) = code_craft(id) {
        return infos_appimages().join(code).exists() && dossier_appimages().join(format!("{code}.AppImage")).exists();
    }
    dossier_flatpak().join("app").join(id).join("current").exists()
}

// =====================================================================
//  Flatpak
// =====================================================================

pub const DEPOT: &str = "flathub";
pub const ADRESSE_DEPOT: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

fn flatpak(args: &[&str]) -> Result<String, String> {
    match Command::new("flatpak").args(args).env("LC_ALL", "C").output() {
        Ok(s) if s.status.success() => Ok(String::from_utf8_lossy(&s.stdout).to_string()),
        Ok(s) => {
            let e = String::from_utf8_lossy(&s.stderr).trim().to_string();
            Err(if e.is_empty() { String::from_utf8_lossy(&s.stdout).trim().to_string() } else { e })
        }
        Err(_) => Err(tr("Flatpak n'est pas installé sur ce système.")),
    }
}

/// Ajoute le dépôt Flathub pour l'utilisateur (une seule fois). Nécessite Internet.
pub fn preparer_depot() -> Result<(), String> {
    if flatpak(&["--user", "remotes", "--columns=name"]).map(|r| r.lines().any(|l| l.trim() == DEPOT)).unwrap_or(false) {
        return Ok(());
    }
    // Pour les tests : dépôt local à la place de Flathub
    let adresse = std::env::var("FLASHOS_DEPOT_TEST").unwrap_or_else(|_| ADRESSE_DEPOT.to_string());
    let mut args = vec!["--user", "remote-add", "--if-not-exists"];
    if std::env::var_os("FLASHOS_DEPOT_TEST").is_some() {
        args.push("--no-gpg-verify");
    }
    args.extend([DEPOT, adresse.as_str()]);
    flatpak(&args).map(|_| ()).map_err(|e| {
        // Message clair, et le détail technique pour l'assistance
        let detail = e.lines().last().unwrap_or("").trim().to_string();
        if detail.contains("resolve") || detail.contains("Could not connect") || detail.contains("Network") {
            tr("Pas de connexion à Internet : impossible de joindre la boutique.")
        } else {
            trv("Impossible de joindre la boutique. Détail : {detail}", &[("detail", &detail)])
        }
    })
}

/// Met à jour le catalogue (noms, icônes) : sans effet si hors ligne
pub fn rafraichir_catalogue() {
    let _ = flatpak(&["--user", "update", "--appstream", DEPOT]);
}

pub fn installer(id: &str) -> Result<(), String> {
    if let Some(code) = code_craft(id) {
        return installer_craft(code);
    }
    preparer_depot()?;
    flatpak(&["--user", "install", "-y", "--noninteractive", DEPOT, id])
        .map(|_| adapter_lanceurs())
        .map_err(|e| {
            if e.contains("No remote refs") || e.contains("Nothing matches") {
                tr("Application introuvable dans la boutique.")
            } else if e.contains("not enough") || e.contains("No space") {
                tr("Pas assez de place sur le disque.")
            } else {
                tr("L'installation a échoué. Vérifiez la connexion à Internet et réessayez.")
            }
        })
}

// =====================================================================
//  Fichiers des partages réseau (NAS) ouverts par les applications de la boutique
// =====================================================================

const MARQUE_ADAPTE: &str = "X-FlashOS-Adapte=true";

/// Dossier des lanceurs de l'utilisateur (prioritaires sur ceux exportés par Flatpak)
fn dossier_lanceurs() -> PathBuf {
    maison().join(".local/share/applications")
}

/// Lanceur adapté : les fichiers sont passés par leur chemin (et non par une adresse smb://…),
/// pour que les fichiers d'un NAS ouverts dans Fichiers s'ouvrent aussi dans VLC & co.
/// Seulement pour les applications qui n'ouvrent pas de liens web (navigateurs, messageries…).
pub fn lanceur_adapte(contenu: &str) -> Option<String> {
    if !contenu.contains("@@u %U @@") && !contenu.contains("@@u %u @@") {
        return None;
    }
    if contenu.lines().any(|l| l.starts_with("MimeType=") && l.contains("x-scheme-handler/")) {
        return None;
    }
    let mut nouveau = contenu.replace("@@u %U @@", "@@ %F @@").replace("@@u %u @@", "@@ %f @@");
    // Marque, juste après l'en-tête
    nouveau = nouveau.replacen("[Desktop Entry]", &format!("[Desktop Entry]\n{MARQUE_ADAPTE}"), 1);
    Some(nouveau)
}

/// Crée (ou retire) les lanceurs adaptés de toutes les applications Flatpak installées
pub fn adapter_lanceurs() {
    let exports = dossier_flatpak().join("exports/share/applications");
    let dest = dossier_lanceurs();
    let _ = std::fs::create_dir_all(&dest);
    let mut gardes = Vec::new();
    if let Ok(liste) = std::fs::read_dir(&exports) {
        for e in liste.flatten() {
            let nom = e.file_name().to_string_lossy().to_string();
            if !nom.ends_with(".desktop") {
                continue;
            }
            let Ok(contenu) = std::fs::read_to_string(e.path()) else { continue };
            if let Some(n) = lanceur_adapte(&contenu) {
                let cible = dest.join(&nom);
                let existant = std::fs::read_to_string(&cible).unwrap_or_default();
                // Un lanceur créé par l'utilisateur lui-même n'est jamais remplacé
                if existant.is_empty() || existant.contains(MARQUE_ADAPTE) {
                    if existant != n {
                        let _ = std::fs::write(&cible, n);
                    }
                    gardes.push(nom);
                }
            }
        }
    }
    // Applications désinstallées : leur lanceur adapté disparaît aussi
    if let Ok(liste) = std::fs::read_dir(&dest) {
        for e in liste.flatten() {
            let nom = e.file_name().to_string_lossy().to_string();
            if gardes.contains(&nom) {
                continue;
            }
            if std::fs::read_to_string(e.path()).map(|c| c.contains(MARQUE_ADAPTE)).unwrap_or(false) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

pub fn desinstaller(appli: &Installee) -> Result<(), String> {
    retirer_epingle(&appli.id);
    match &appli.genre {
        Genre::Flatpak => {
            flatpak(&["--user", "uninstall", "-y", "--noninteractive", &appli.id]).map(|_| ())?;
            // Bases communes devenues inutiles : place libérée
            let _ = flatpak(&["--user", "uninstall", "-y", "--noninteractive", "--unused"]);
            adapter_lanceurs();
            Ok(())
        }
        Genre::AppImage(f) => {
            let _ = std::fs::remove_file(f);
            let _ = std::fs::remove_file(infos_appimages().join(&appli.id));
            let _ = std::fs::remove_file(infos_appimages().join(format!("{}.icone", appli.id)));
            let _ = std::fs::remove_file(dossier_lanceurs().join(format!("flashos-appimage-{}.desktop", appli.id)));
            Ok(())
        }
        Genre::Systeme(_) => Err(tr("Cette application fait partie de SoleOS.")),
        Genre::Web(_) => {
            let _ = std::fs::remove_file(infos_sites().join(&appli.id));
            let _ = std::fs::remove_file(infos_sites().join(format!("{}.png", appli.id)));
            Ok(())
        }
        Genre::Lanceur(fichier, c) => {
            // Application d'un conteneur : retirée de la liste (le logiciel reste dans l'espace Debian)
            if let Some(i) = c.iter().position(|x| x == "-n" || x == "--name") {
                if let (Some(nom), Some(stem)) = (c.get(i + 1), fichier.file_stem()) {
                    let appli = stem.to_string_lossy().trim_start_matches(&format!("{nom}-")).to_string();
                    let _ = distrobox(&["enter", nom, "--", "distrobox-export", "--app", &appli, "--delete"]);
                }
            }
            if fichier.exists() {
                std::fs::remove_file(fichier).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct Resultat {
    pub id: String,
    pub nom: String,
    pub description: String,
}

/// Recherche dans tout Flathub (catalogue local de Flatpak, mis à jour à l'ouverture)
pub fn chercher(terme: &str) -> Vec<Resultat> {
    let sortie = flatpak(&["--user", "search", "--columns=name,description,application,remotes", terme]).unwrap_or_default();
    analyser_recherche(&sortie)
}

pub fn analyser_recherche(sortie: &str) -> Vec<Resultat> {
    let mut v: Vec<Resultat> = Vec::new();
    for l in sortie.lines() {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 3 || c[2].trim().is_empty() || c[2].contains(' ') {
            continue;
        }
        let id = c[2].trim().to_string();
        // Uniquement les applications (pas les bibliothèques, pilotes, extensions…)
        if id.ends_with(".Locale") || id.ends_with(".Debug") || id.contains(".Platform") || id.contains(".Sdk") || id.contains(".Extension.") {
            continue;
        }
        if v.iter().any(|r| r.id == id) {
            continue;
        }
        v.push(Resultat { id, nom: c[0].trim().to_string(), description: c[1].trim().to_string() });
    }
    v.truncate(40);
    v
}

/// Mise à jour silencieuse des applications (appelée par la mise à jour automatique)
pub fn mettre_a_jour() -> Result<(), String> {
    flatpak(&["--user", "update", "-y", "--noninteractive"]).map(|_| ())
}

// =====================================================================
//  AppImage
// =====================================================================

/// Range un fichier .AppImage dans ~/Applications et l'ajoute à la liste (nom et icône extraits)
pub fn ajouter_appimage(source: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    if !source.is_file() {
        return Err(tr("Fichier introuvable."));
    }
    let dossier = dossier_appimages();
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let nom_fichier = source.file_name().unwrap().to_string_lossy().to_string();
    let cible = dossier.join(&nom_fichier);
    if source != cible {
        if std::fs::rename(source, &cible).is_err() {
            std::fs::copy(source, &cible).map_err(|_| tr("Copie impossible (place disque ?)"))?;
        }
    }
    let _ = std::fs::set_permissions(&cible, std::fs::Permissions::from_mode(0o755));

    // Nom et icône : extraits de l'AppImage elle-même (sans l'exécuter vraiment)
    let id = nom_fichier.trim_end_matches(".AppImage").trim_end_matches(".appimage").to_string();
    let infos = infos_appimages();
    let _ = std::fs::create_dir_all(&infos);
    let temp = std::env::temp_dir().join(format!("flashos-appimage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let _ = std::fs::create_dir_all(&temp);
    for motif in ["*.desktop", ".DirIcon", "*.png", "*.svg"] {
        let _ = Command::new(&cible).arg("--appimage-extract").arg(motif).current_dir(&temp).output();
    }
    let racine = temp.join("squashfs-root");
    let mut nom = id.split(['-', '_']).next().unwrap_or(&id).to_string();
    if let Ok(entrees) = std::fs::read_dir(&racine) {
        for e in entrees.flatten() {
            if e.file_name().to_string_lossy().ends_with(".desktop") {
                let (n, _, _) = lire_desktop(&std::fs::read_to_string(e.path()).unwrap_or_default());
                if !n.is_empty() {
                    nom = n;
                }
            }
        }
    }
    let icone = infos.join(format!("{id}.icone"));
    let dir_icon = std::fs::canonicalize(racine.join(".DirIcon")).ok().filter(|p| p.is_file());
    if let Some(i) = dir_icon {
        let _ = std::fs::copy(i, &icone);
    } else if let Some(i) = icone_craft(&id) {
        let _ = std::fs::copy(i, &icone);
    }
    // Lanceur pour « Ouvrir avec » dans Fichiers (types de fichiers déclarés par l'appli)
    let mut types = String::new();
    if let Ok(entrees) = std::fs::read_dir(&racine) {
        for e in entrees.flatten() {
            if e.file_name().to_string_lossy().ends_with(".desktop") {
                let texte = std::fs::read_to_string(e.path()).unwrap_or_default();
                if let Some(t) = texte.lines().find_map(|l| l.strip_prefix("MimeType=")) {
                    // Types très courants (texte simple, pages web, dossiers, liens) : laissés aux applis
                    // de SoleOS, pour qu'une appli ajoutée ne devienne pas leur appli par défaut
                    types = t
                        .split(';')
                        .map(str::trim)
                        .filter(|m| !m.is_empty())
                        .filter(|m| !matches!(*m, "text/plain" | "text/html" | "inode/directory" | "application/xhtml+xml"))
                        .filter(|m| !m.starts_with("x-scheme-handler/"))
                        .map(|m| format!("{m};"))
                        .collect();
                }
            }
        }
    }
    let lanceur = dossier_lanceurs().join(format!("flashos-appimage-{id}.desktop"));
    ecrire(
        &lanceur,
        &format!(
            "[Desktop Entry]\nType=Application\nName={nom}\nExec=\"{}\" %F\nIcon={}\nTerminal=false\n{}X-FlashOS-AppImage=true\n",
            cible.display(),
            icone.display(),
            if types.is_empty() { String::new() } else { format!("MimeType={types}\n") }
        ),
    );
    let _ = Command::new("update-desktop-database").arg(dossier_lanceurs()).status();
    let _ = std::fs::remove_dir_all(&temp);
    ecrire(
        &infos.join(&id),
        &format!("nom={nom}\nfichier={}\nicone={}\n", cible.display(), icone.display()),
    );
    Ok(nom)
}

// =====================================================================
//  Paquets .deb : installés dans un conteneur Debian (Distrobox), jamais dans le système
// =====================================================================

pub const CONTENEUR: &str = "flashos-debian";

fn distrobox(args: &[&str]) -> Result<String, String> {
    match Command::new("distrobox").args(args).env("LC_ALL", "C").output() {
        Ok(s) if s.status.success() => Ok(String::from_utf8_lossy(&s.stdout).to_string()),
        Ok(s) => {
            let e = String::from_utf8_lossy(&s.stderr).trim().to_string();
            Err(if e.is_empty() { String::from_utf8_lossy(&s.stdout).trim().to_string() } else { e })
        }
        Err(_) => Err(tr("Distrobox n'est pas installé sur ce système.")),
    }
}

/// Crée le conteneur Debian la première fois (téléchargement d'environ 50 Mo)
pub fn preparer_conteneur() -> Result<(), String> {
    let liste = distrobox(&["list", "--no-color"]).unwrap_or_default();
    if !liste.lines().any(|l| l.split('|').nth(1).map(|n| n.trim() == CONTENEUR).unwrap_or(false)) {
        let image = std::env::var("FLASHOS_CONTENEUR_IMAGE").unwrap_or_else(|_| "docker.io/library/debian:trixie".into());
        distrobox(&["create", "--yes", "--name", CONTENEUR, "--image", &image]).map_err(|e| {
            if e.contains("pinging") || e.contains("dial tcp") || e.contains("network") || e.contains("resolve") {
                tr("Pas de connexion à Internet : impossible de télécharger l'espace Debian.")
            } else {
                trv("Création de l'espace Debian impossible. Détail : {detail}", &[("detail", e.lines().last().unwrap_or(""))])
            }
        })?;
    }
    // Première entrée : préparation du conteneur (compte, droits…)
    distrobox(&["enter", CONTENEUR, "--", "true"]).map(|_| ()).map_err(|e| trv("Démarrage de l'espace Debian impossible. Détail : {detail}", &[("detail", e.lines().last().unwrap_or(""))]))
}

/// Nom et lanceurs (.desktop) contenus dans un paquet .deb
pub fn contenu_deb(fichier: &Path) -> (String, Vec<String>) {
    let nom = Command::new("dpkg-deb")
        .args(["-f"])
        .arg(fichier)
        .arg("Package")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let liste = Command::new("dpkg-deb").arg("-c").arg(fichier).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let lanceurs = liste
        .lines()
        .filter_map(|l| l.split_whitespace().last())
        .filter(|c| c.contains("/usr/share/applications/") && c.ends_with(".desktop"))
        .filter_map(|c| Path::new(c).file_stem().map(|n| n.to_string_lossy().to_string()))
        .collect();
    (nom, lanceurs)
}

/// Installe un paquet .deb dans le conteneur Debian, puis met ses applications dans « Mes applications »
pub fn installer_deb(fichier: &Path) -> Result<String, String> {
    if !fichier.is_file() {
        return Err(tr("Fichier introuvable."));
    }
    let (paquet, lanceurs) = contenu_deb(fichier);
    preparer_conteneur()?;
    // Copie dans le dossier personnel (visible depuis le conteneur, même chemin)
    let dossier = maison().join(".cache/flashos/paquets");
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let copie = dossier.join(fichier.file_name().ok_or_else(|| tr("Nom de fichier invalide."))?);
    if copie != fichier {
        std::fs::copy(fichier, &copie).map_err(|_| tr("Copie impossible (place disque ?)"))?;
    }
    let chemin = copie.to_string_lossy().to_string();
    let _ = distrobox(&["enter", CONTENEUR, "--", "sudo", "apt-get", "update", "-q"]);
    distrobox(&["enter", CONTENEUR, "--", "sudo", "env", "DEBIAN_FRONTEND=noninteractive", "apt-get", "install", "-y", "-q", &chemin])
        .map_err(|e| trv("L'installation du paquet a échoué. Détail : {detail}", &[("detail", e.lines().last().unwrap_or(""))]))?;
    let _ = std::fs::remove_file(&copie);
    for l in &lanceurs {
        let _ = distrobox(&["enter", CONTENEUR, "--", "distrobox-export", "--app", l]);
    }
    let _ = Command::new("update-desktop-database").arg(dossier_lanceurs()).status();
    Ok(if paquet.is_empty() { tr("le paquet") } else { paquet })
}

// =====================================================================
//  Sites web sur l'accueil
// =====================================================================

fn infos_sites() -> PathBuf {
    maison().join(".local/share/flashos/sites")
}

/// « https://www.exemple.fr/page » → « www.exemple.fr »
pub fn hote(adresse: &str) -> String {
    let sans = adresse.split("://").nth(1).unwrap_or(adresse);
    sans.split(['/', '?', '#']).next().unwrap_or("").to_string()
}

/// Adresse complète à partir de ce qui a été tapé (« lemonde.fr » → « https://lemonde.fr/ »)
pub fn normaliser_adresse(texte: &str) -> Option<String> {
    let t = texte.trim();
    if t.is_empty() || t.contains(' ') {
        return None;
    }
    let complete = if t.starts_with("http://") || t.starts_with("https://") { t.to_string() } else { format!("https://{t}") };
    let h = hote(&complete);
    if h.is_empty() || !h.contains('.') {
        return None;
    }
    Some(if complete.trim_end_matches(&h).is_empty() || complete.ends_with(&h) { format!("{complete}/") } else { complete })
}

/// Icône d'un raccourci du navigateur (rangée dans ~/.local/share/icons)
fn icone_theme_utilisateur(nom: &str) -> Option<PathBuf> {
    if nom.starts_with('/') {
        return Some(PathBuf::from(nom)).filter(|p| p.exists());
    }
    let base = maison().join(".local/share/icons/hicolor");
    for taille in ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32"] {
        let f = base.join(taille).join("apps").join(format!("{nom}.png"));
        if f.exists() {
            return Some(f);
        }
    }
    None
}

/// Ajoute un site web à l'accueil (son icône est récupérée en ligne si possible)
pub fn ajouter_site(nom: &str, adresse: &str) -> Result<String, String> {
    let adresse = normaliser_adresse(adresse).ok_or_else(|| tr("Adresse invalide (exemple : lemonde.fr)."))?;
    let h = hote(&adresse);
    let nom = if nom.trim().is_empty() { h.trim_start_matches("www.").to_string() } else { nom.trim().to_string() };
    let id: String = format!("site-{}", h.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>());
    let dossier = infos_sites();
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    ecrire(&dossier.join(&id), &format!("nom={nom}\nadresse={adresse}\n"));
    // Icône du site (service d'icônes de Google, 128 px) : facultative
    let icone = dossier.join(format!("{id}.png"));
    let ok = Command::new("curl")
        .args(["-fsSL", "-m", "15", "-o"])
        .arg(&icone)
        .arg(format!("https://www.google.com/s2/favicons?domain={h}&sz=128"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&icone);
    }
    epingler(&id);
    Ok(id)
}

// =====================================================================
//  Épingles (applications affichées sur l'accueil)
// =====================================================================

pub fn epingles() -> Vec<String> {
    std::fs::read_to_string(config("epingles"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn est_epinglee(id: &str) -> bool {
    epingles().iter().any(|e| e == id)
}

pub fn epingler(id: &str) {
    let mut e = epingles();
    if !e.iter().any(|x| x == id) {
        e.push(id.to_string());
    }
    ecrire(&config("epingles"), &(e.join("\n") + "\n"));
}

pub fn retirer_epingle(id: &str) {
    let e: Vec<String> = epingles().into_iter().filter(|x| x != id).collect();
    ecrire(&config("epingles"), &(e.join("\n") + "\n"));
}

// =====================================================================
//  Messagerie (tuile « Mail »)
// =====================================================================

/// (code, nom affiché, adresse, couleur de la pastille)
pub const MESSAGERIES: &[(&str, &str, &str, &str)] = &[
    ("gmail", "Gmail", "https://mail.google.com/", "#d93025"),
    ("outlook", "Outlook / Hotmail", "https://outlook.live.com/mail/", "#0a64ad"),
    ("orange", "Orange", "https://messagerie.orange.fr/", "#ff7900"),
    ("free", "Free", "https://zimbra.free.fr/", "#cd1e25"),
    ("sfr", "SFR", "https://webmail.sfr.fr/", "#e2001a"),
    ("bouygues", "Bouygues Telecom", "https://www.mail.bbox.fr/", "#009dcc"),
    ("laposte", "La Poste", "https://www.laposte.net/accueil", "#f6b500"),
    ("libero", "Libero Mail", "https://login.libero.it/", "#0e79c8"),
    ("virgilio", "Virgilio Mail", "https://mail1.virgilio.it/", "#e2001a"),
    ("gmx", "GMX", "https://www.gmx.net/", "#1c449b"),
    ("webde", "WEB.DE", "https://web.de/", "#ffd800"),
    ("tonline", "t-online", "https://email.t-online.de/", "#e20074"),
    ("qq", "QQ邮箱", "https://mail.qq.com/", "#12b7f5"),
    ("163", "网易163邮箱", "https://mail.163.com/", "#d4161d"),
    ("yahoo", "Yahoo", "https://mail.yahoo.com/", "#6001d2"),
    ("icloud", "iCloud", "https://www.icloud.com/mail/", "#6e6e73"),
    ("proton", "Proton Mail", "https://mail.proton.me/", "#6d4aff"),
];

/// Messageries proposées dans la langue choisie (les messageries d'un pays sont proposées
/// dans sa langue ; Gmail, Outlook, Yahoo, iCloud et Proton partout)
pub fn messageries_proposees() -> Vec<&'static (&'static str, &'static str, &'static str, &'static str)> {
    let langue = crate::tr::courante().code;
    let locales: &[&str] = match langue {
        "fr" => &["orange", "free", "sfr", "bouygues", "laposte"],
        "it" => &["libero", "virgilio"],
        "de" => &["gmx", "webde", "tonline"],
        "zh" => &["qq", "163"],
        _ => &[],
    };
    let toutes = ["gmail", "outlook", "yahoo", "icloud", "proton"];
    MESSAGERIES.iter().filter(|(c, _, _, _)| toutes.contains(c) || locales.contains(c)).collect()
}

/// Messageries choisies (une par ligne : adresse web ou « thunderbird »), dans l'ordre d'ajout
pub fn messageries() -> Vec<String> {
    std::fs::read_to_string(config("mail"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Ajoute une messagerie (sans doublon)
pub fn choisir_messagerie(adresse_ou_thunderbird: &str) {
    let mut m = messageries();
    if !m.iter().any(|x| x == adresse_ou_thunderbird) {
        m.push(adresse_ou_thunderbird.to_string());
    }
    ecrire(&config("mail"), &(m.join("\n") + "\n"));
}

pub fn retirer_messagerie(adresse: &str) {
    let m: Vec<String> = messageries().into_iter().filter(|x| x != adresse).collect();
    ecrire(&config("mail"), &(m.join("\n") + "\n"));
}

/// Nom affiché d'une messagerie : « Gmail », « Orange »… ou le nom du site
pub fn nom_messagerie(adresse: &str) -> String {
    if adresse == "thunderbird" {
        return "Thunderbird".into();
    }
    if let Some((_, nom, _, _)) = MESSAGERIES.iter().find(|(_, _, a, _)| *a == adresse) {
        return nom.to_string();
    }
    adresse
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .split('/')
        .next()
        .unwrap_or(adresse)
        .to_string()
}

/// Commande pour ouvrir une messagerie
pub fn commande_messagerie(adresse: &str) -> Option<Vec<String>> {
    if adresse == "thunderbird" {
        return est_installee("org.mozilla.Thunderbird")
            .then(|| vec!["flatpak".into(), "run".into(), "org.mozilla.Thunderbird".into()]);
    }
    Some(vec!["flashos-web".into(), "--new-window".into(), adresse.to_string()])
}

/// Commande de la tuile Mail quand une seule messagerie est choisie
pub fn commande_mail() -> Option<Vec<String>> {
    match messageries().as_slice() {
        [une] => commande_messagerie(une),
        _ => None,
    }
}

#[cfg(test)]
mod tests_sites {
    use super::*;
    #[test]
    fn adresses() {
        assert_eq!(normaliser_adresse("lemonde.fr").as_deref(), Some("https://lemonde.fr/"));
        assert_eq!(normaliser_adresse("https://www.ex.fr/a/b").as_deref(), Some("https://www.ex.fr/a/b"));
        assert_eq!(normaliser_adresse("bonjour"), None);
        assert_eq!(hote("https://www.ex.fr/a?b"), "www.ex.fr");
    }
}

#[cfg(test)]
mod tests_lanceurs {
    use super::*;
    #[test]
    fn adapte_vlc_mais_pas_un_navigateur() {
        let vlc = "[Desktop Entry]\nName=VLC\nExec=/usr/bin/flatpak run --command=/app/bin/vlc --file-forwarding org.videolan.VLC --started-from-file @@u %U @@\nMimeType=video/mp4;audio/mpeg;\n";
        let n = lanceur_adapte(vlc).unwrap();
        assert!(n.contains("@@ %F @@") && n.contains(MARQUE_ADAPTE));
        let nav = "[Desktop Entry]\nExec=/usr/bin/flatpak run org.mozilla.firefox @@u %u @@\nMimeType=text/html;x-scheme-handler/http;\n";
        assert!(lanceur_adapte(nav).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop() {
        let (n, i, c) = lire_desktop("[Desktop Entry]\nName=GNU Image\nName[fr]=Retouche GIMP\nIcon=org.gimp.GIMP\n[Desktop Action new]\nName=Nouveau\n");
        assert_eq!((n.as_str(), i.as_str(), c), ("Retouche GIMP", "org.gimp.GIMP", false));
        assert!(lire_desktop("[Desktop Entry]\nName=x\nNoDisplay=true\n").2);
    }

    #[test]
    fn recherche() {
        let r = analyser_recherche(
            "VLC\tVLC media player\torg.videolan.VLC\tflathub\n\
             VLC\tdoublon\torg.videolan.VLC\tflathub\n\
             Codecs\tx\torg.freedesktop.Platform.ffmpeg\tflathub\n\
             Krita\tPeinture\torg.kde.krita\tflathub\n",
        );
        let ids: Vec<&str> = r.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["org.videolan.VLC", "org.kde.krita"]);
    }

    #[test]
    fn noms_messageries() {
        assert_eq!(nom_messagerie("https://mail.google.com/"), "Gmail");
        assert_eq!(nom_messagerie("https://webmail.monentreprise.fr/login"), "webmail.monentreprise.fr");
        assert_eq!(nom_messagerie("thunderbird"), "Thunderbird");
    }

    #[test]
    fn selection_coherente() {
        for p in SELECTION {
            assert!(CATEGORIES.contains(&p.categorie), "{}", p.id);
            assert_eq!(SELECTION.iter().filter(|q| q.id == p.id).count(), 1);
        }
    }
}
