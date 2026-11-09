//! Icônes SoleOS (dessins originaux), intégrées au programme.

use gtk::{gdk, glib};

macro_rules! icone {
    ($nom:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icones/png/", $nom, ".png")).as_slice()
    };
}

pub fn donnees(nom: &str) -> Option<&'static [u8]> {
    Some(match nom {
        "bureautique" => icone!("bureautique"),
        "mail" => icone!("mail"),
        "internet" => icone!("internet"),
        "fichiers" => icone!("fichiers"),
        "wifi" => icone!("wifi"),
        "wifi-off" => icone!("wifi-off"),
        "filaire" => icone!("filaire"),
        "son" => icone!("son"),
        "son-coupe" => icone!("son-coupe"),
        "batterie" => icone!("batterie"),
        "batterie-charge" => icone!("batterie-charge"),
        "alimentation" => icone!("alimentation"),
        "console" => icone!("console"),
        "appareils" => icone!("appareils"),
        "bluetooth" => icone!("bluetooth"),
        "ecran" => icone!("ecran"),
        "logo" => icone!("logo"),
        "logo-s" => icone!("logo-s"),
        "applications" => icone!("applications"),
        "appli" => icone!("appli"),
        "reglages" => icone!("reglages"),
        "cadenas" => icone!("cadenas"),
        "taille" => icone!("taille"),
        "horloge" => icone!("horloge"),
        "image" => icone!("image"),
        "info" => icone!("info"),
        "r-alimentation" => icone!("r-alimentation"),
        "r-appareils" => icone!("r-appareils"),
        "r-batterie" => icone!("r-batterie"),
        "r-batterie-charge" => icone!("r-batterie-charge"),
        "r-bluetooth" => icone!("r-bluetooth"),
        "r-cadenas" => icone!("r-cadenas"),
        "r-console" => icone!("r-console"),
        "r-ecran" => icone!("r-ecran"),
        "r-filaire" => icone!("r-filaire"),
        "r-horloge" => icone!("r-horloge"),
        "r-image" => icone!("r-image"),
        "r-info" => icone!("r-info"),
        "r-internet" => icone!("r-internet"),
        "r-reglages" => icone!("r-reglages"),
        "r-langue" => icone!("r-langue"),
        "r-rail" => icone!("r-rail"),
        "r-son" => icone!("r-son"),
        "r-son-coupe" => icone!("r-son-coupe"),
        "r-taille" => icone!("r-taille"),
        "r-wifi" => icone!("r-wifi"),
        "r-wifi-off" => icone!("r-wifi-off"),
        "tuile-bureautique" => icone!("tuile-bureautique"),
        "d-texte" => icone!("d-texte"),
        "d-tableur" => icone!("d-tableur"),
        "d-presentation" => icone!("d-presentation"),
        "d-pdf" => icone!("d-pdf"),
        "tuile-mail" => icone!("tuile-mail"),
        "tuile-internet" => icone!("tuile-internet"),
        "tuile-fichiers" => icone!("tuile-fichiers"),
        "tuile-applications" => icone!("tuile-applications"),
        "tuile-console" => icone!("tuile-console"),
        "tuile-appli" => icone!("tuile-appli"),
        "reglages-tuile" => icone!("reglages-tuile"),
        "eclair-bouton" => icone!("eclair-bouton"),
        "veille" => icone!("veille"),
        "loupe" => icone!("loupe"),
        "r-camera" => icone!("r-camera"),
        "r-sauvegarde" => icone!("r-sauvegarde"),
        "redemarrer" => icone!("redemarrer"),
        "r-batterie-moyen" => icone!("r-batterie-moyen"),
        "r-batterie-faible" => icone!("r-batterie-faible"),
        "r-nuit" => icone!("r-nuit"),
        "r-luminosite" => icone!("r-luminosite"),
        "r-console-bas" => icone!("r-console-bas"),
        "r-calendrier" => icone!("r-calendrier"),
        "corbeille" => icone!("corbeille"),
        "corbeille-pleine" => icone!("corbeille-pleine"),
        "r-son-bas" => icone!("r-son-bas"),
        "r-son-coupe-bas" => icone!("r-son-coupe-bas"),
        "r-wifi-off-bas" => icone!("r-wifi-off-bas"),
        _ => return None,
    })
}

/// Image prête à l'emploi, à la taille demandée (en pixels).
pub fn image(nom: &str, taille: i32) -> gtk::Image {
    let image = gtk::Image::new();
    image.set_pixel_size(taille);
    changer(&image, nom);
    image
}

/// Remplace le dessin d'une image existante.
pub fn changer(image: &gtk::Image, nom: &str) {
    if let Some(octets) = donnees(nom) {
        if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_static(octets)) {
            image.set_paintable(Some(&texture));
        }
    }
}

/// Image brute (pour les images non carrées, comme le logo).
#[allow(dead_code)]
pub fn texture(nom: &str) -> Option<gdk::Texture> {
    gdk::Texture::from_bytes(&glib::Bytes::from_static(donnees(nom)?)).ok()
}

/// Thème d'icônes de l'écran, avec les icônes des applications Flatpak (système et utilisateur)
#[allow(dead_code)]
pub fn theme() -> Option<gtk::IconTheme> {
    let ecran = gdk::Display::default()?;
    let theme = gtk::IconTheme::for_display(&ecran);
    thread_local! { static FAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    if !FAIT.with(|f| f.replace(true)) {
        let mut chemins = vec![std::path::PathBuf::from("/var/lib/flatpak/exports/share/icons")];
        if let Some(h) = std::env::var_os("HOME") {
            chemins.push(std::path::PathBuf::from(&h).join(".local/share/flatpak/exports/share/icons"));
            chemins.push(std::path::PathBuf::from(h).join(".local/share/icons"));
        }
        for c in chemins.iter().filter(|c| c.is_dir()) {
            theme.add_search_path(c);
        }
    }
    Some(theme)
}

/// Icône d'une application : fichier (PNG ou SVG), sinon par nom dans le thème d'icônes
/// (nom du fichier sans extension, puis les autres noms proposés). `None` si rien ne convient.
#[allow(dead_code)]
pub fn appli(fichier: Option<&std::path::Path>, noms: &[&str], taille: i32) -> Option<gtk::Image> {
    let image = gtk::Image::new();
    image.set_pixel_size(taille);
    if let Some(f) = fichier {
        if let Ok(t) = gdk::Texture::from_filename(f) {
            image.set_paintable(Some(&t));
            return Some(image);
        }
    }
    let theme = theme()?;
    let tige = fichier.and_then(|f| f.file_stem()).map(|s| s.to_string_lossy().to_string());
    let mut candidats: Vec<String> = Vec::new();
    if let Some(t) = tige {
        candidats.push(t);
    }
    for n in noms {
        if !n.is_empty() {
            candidats.push(n.to_string());
            candidats.push(n.to_lowercase());
            // « org.videolan.VLC » → « vlc »
            if let Some(dernier) = n.rsplit('.').next() {
                candidats.push(dernier.to_lowercase());
            }
        }
    }
    let nom = candidats.into_iter().find(|n| theme.has_icon(n))?;
    image.set_icon_name(Some(&nom));
    Some(image)
}
