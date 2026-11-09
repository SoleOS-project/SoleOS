//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Raccourcis sur le bureau (v13.1), comme Windows et macOS : le contenu du dossier « Bureau »
//! s'affiche en icônes sur le fond d'écran, en colonnes, à droite de la barre magique.
//! - double-clic (ou Entrée) : ouvrir ;
//! - clic droit (ou appui long) : Ouvrir, Retirer du bureau (raccourci) ou Mettre à la corbeille ;
//! - un fichier ou un dossier glissé depuis Fichiers devient un raccourci (le fichier ne bouge pas) ;
//! - « Ajouter au bureau » dans l'éclair (applications) et dans Fichiers (clic droit → Scripts).
//! Le dossier est surveillé : un changement s'affiche tout de suite. Rien ne tourne sinon.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::icones;
use crate::tr::tr;

/// Dossier « Bureau » de l'utilisateur (créé s'il manque)
pub fn dossier() -> PathBuf {
    let d = glib::user_special_dir(glib::UserDirectory::Desktop)
        .filter(|d| *d != glib::home_dir())
        .unwrap_or_else(|| glib::home_dir().join(tr("Bureau")));
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Nom libre dans le dossier Bureau (« Nom », « Nom (2) »…)
fn nom_libre(base: &str) -> PathBuf {
    let d = dossier();
    let (tige, ext) = match base.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() && e.len() <= 8 => (t.to_string(), format!(".{e}")),
        _ => (base.to_string(), String::new()),
    };
    let mut chemin = d.join(base);
    let mut i = 2;
    while chemin.symlink_metadata().is_ok() {
        chemin = d.join(format!("{tige} ({i}){ext}"));
        i += 1;
    }
    chemin
}

/// Valeur protégée pour la ligne Exec= d'un lanceur
fn echapper(a: &str) -> String {
    if a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@".contains(c)) {
        a.to_string()
    } else {
        format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$"))
    }
}

/// « Ajouter au bureau » pour une application installée
pub fn ajouter_appli(id: &str, nom: &str, icone: Option<&Path>, commande: &[String]) -> std::io::Result<()> {
    let propre: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let chemin = dossier().join(format!("flashos-{propre}.desktop"));
    let exec = commande.iter().map(|a| echapper(a)).collect::<Vec<_>>().join(" ");
    let mut texte = format!("[Desktop Entry]\nType=Application\nName={nom}\nExec={exec}\nX-FlashOS-Appli={id}\n");
    if let Some(i) = icone {
        texte.push_str(&format!("Icon={}\n", i.display()));
    }
    std::fs::write(&chemin, texte)
}

/// Un fichier ou dossier déposé : un lien vers lui (un lanceur pour une adresse réseau sans chemin)
pub fn ajouter_fichier(f: &gio::File) -> std::io::Result<()> {
    let nom = f.basename().map(|b| b.to_string_lossy().to_string()).filter(|n| !n.is_empty() && n != "/").unwrap_or_else(|| tr("Raccourci"));
    // Dossier réseau (smb://…) : on garde l'adresse, pas le chemin « monté » (qui disparaît
    // quand le NAS est déconnecté) ; le double-clic reconnecte le NAS tout seul
    let chemin_local = if f.uri_scheme().map(|s| s == "file").unwrap_or(false) { f.path() } else { None };
    match chemin_local {
        Some(p) => {
            // Déjà sur le bureau : rien à faire
            if p.parent() == Some(dossier().as_path()) {
                return Ok(());
            }
            std::os::unix::fs::symlink(&p, nom_libre(&nom))
        }
        None => {
            let chemin = nom_libre(&format!("{nom}.desktop"));
            std::fs::write(chemin, format!("[Desktop Entry]\nType=Link\nName={nom}\nURL={}\nIcon=folder-remote\n", f.uri()))
        }
    }
}

/// Lecture minimale d'un lanceur .desktop
fn champ(texte: &str, cle: &str) -> Option<String> {
    texte.lines().find_map(|l| l.strip_prefix(&format!("{cle}=")).map(|v| v.trim().to_string()))
}

#[derive(Clone)]
enum Genre {
    /// Application SoleOS (identifiant)
    Appli(String),
    /// Autre lanceur .desktop
    Lanceur,
    /// Adresse (réseau, site)
    Lien(String),
    Dossier,
    Fichier,
}

#[derive(Clone)]
struct Element {
    chemin: PathBuf,
    nom: String,
    genre: Genre,
    icone: Option<String>,
    /// Lien ou lanceur : le retirer ne touche jamais au fichier d'origine
    raccourci: bool,
}

fn lire(chemin: &Path) -> Option<Element> {
    let nom_fichier = chemin.file_name()?.to_string_lossy().to_string();
    if nom_fichier.starts_with('.') {
        return None;
    }
    let lien = chemin.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false);
    if nom_fichier.ends_with(".desktop") {
        let texte = std::fs::read_to_string(chemin).ok()?;
        let nom = champ(&texte, "Name").unwrap_or_else(|| nom_fichier.trim_end_matches(".desktop").to_string());
        let icone = champ(&texte, "Icon");
        let genre = if let Some(id) = champ(&texte, "X-FlashOS-Appli") {
            Genre::Appli(id)
        } else if champ(&texte, "Type").as_deref() == Some("Link") {
            Genre::Lien(champ(&texte, "URL").unwrap_or_default())
        } else {
            Genre::Lanceur
        };
        return Some(Element { chemin: chemin.to_path_buf(), nom, genre, icone, raccourci: true });
    }
    let genre = if chemin.is_dir() { Genre::Dossier } else { Genre::Fichier };
    Some(Element { chemin: chemin.to_path_buf(), nom: nom_fichier, genre, icone: None, raccourci: lien })
}

/// Icône SoleOS selon le type de document
fn icone_document(nom: &str) -> Option<&'static str> {
    let ext = nom.rsplit_once('.').map(|(_, e)| e.to_lowercase())?;
    Some(match ext.as_str() {
        "doc" | "docx" | "odt" | "rtf" | "txt" | "md" => "d-texte",
        "xls" | "xlsx" | "ods" | "csv" => "d-tableur",
        "ppt" | "pptx" | "odp" => "d-presentation",
        "pdf" => "d-pdf",
        _ => return None,
    })
}

fn image_element(e: &Element) -> gtk::Image {
    const T: i32 = 52;
    match &e.genre {
        Genre::Dossier => return icones::image("fichiers", T),
        Genre::Lien(_) => return icones::image("r-filaire", T),
        Genre::Appli(_) | Genre::Lanceur => {
            if let Some(i) = &e.icone {
                if i.starts_with('/') {
                    if let Ok(t) = gdk::Texture::from_filename(i) {
                        let im = gtk::Image::from_paintable(Some(&t));
                        im.set_pixel_size(T);
                        return im;
                    }
                } else if let Some(im) = icones::appli(None, &[i.as_str()], T) {
                    return im;
                }
            }
            return icones::image("appli", T);
        }
        Genre::Fichier => {}
    }
    if let Some(n) = icone_document(&e.nom) {
        return icones::image(n, T);
    }
    // Photos : la photo elle-même, en petit (seulement les fichiers raisonnables)
    let ext = e.nom.rsplit_once('.').map(|(_, x)| x.to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") && std::fs::metadata(&e.chemin).map(|m| m.len() < 8_000_000).unwrap_or(false) {
        if let Ok(pb) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&e.chemin, T * 2, T * 2, true) {
            let im = gtk::Image::from_paintable(Some(&gdk::Texture::for_pixbuf(&pb)));
            im.set_pixel_size(T);
            im.add_css_class("miniature-bureau");
            return im;
        }
    }
    // Sinon l'icône du type de fichier (thème d'icônes)
    if let Ok(info) = gio::File::for_path(&e.chemin).query_info("standard::icon", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE) {
        if let Some(gi) = info.icon() {
            let im = gtk::Image::from_gicon(&gi);
            im.set_pixel_size(T);
            return im;
        }
    }
    icones::image("appli", T)
}

pub struct Bureau {
    pub racine: gtk::FlowBox,
    _moniteur: Option<gio::FileMonitor>,
    ouvrir_appli: Box<dyn Fn(&str)>,
}

impl Bureau {
    /// `ouvrir_appli` : ouvre une application SoleOS par son identifiant
    pub fn nouveau(ouvrir_appli: impl Fn(&str) + 'static) -> Rc<Self> {
        let racine = gtk::FlowBox::new();
        racine.add_css_class("icones-bureau");
        racine.set_orientation(gtk::Orientation::Vertical);
        racine.set_selection_mode(gtk::SelectionMode::None);
        racine.set_halign(gtk::Align::Start);
        racine.set_valign(gtk::Align::Start);
        racine.set_homogeneous(true);
        racine.set_column_spacing(4);
        racine.set_row_spacing(4);
        // Rangées par colonne : autant que la hauteur de l'écran le permet
        let hauteur = gdk::Display::default()
            .and_then(|d| d.monitors().item(0))
            .and_downcast::<gdk::Monitor>()
            .map(|m| m.geometry().height())
            .unwrap_or(768);
        racine.set_max_children_per_line((((hauteur - 56 - 60) / 120).max(3)) as u32);
        racine.set_min_children_per_line(1);
        let moniteur = gio::File::for_path(dossier()).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE).ok();
        let b = Rc::new(Bureau { racine, _moniteur: moniteur, ouvrir_appli: Box::new(ouvrir_appli) });
        if let Some(m) = &b._moniteur {
            let moi = Rc::downgrade(&b);
            // Plusieurs changements d'un coup : un seul rafraîchissement
            let prevu = Rc::new(std::cell::Cell::new(false));
            m.connect_changed(move |_, _, _, _| {
                if prevu.replace(true) {
                    return;
                }
                let (moi, prevu) = (moi.clone(), prevu.clone());
                glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                    prevu.set(false);
                    if let Some(b) = moi.upgrade() {
                        b.remplir();
                    }
                });
            });
        }
        b.remplir();
        b
    }

    pub fn remplir(self: &Rc<Self>) {
        while let Some(e) = self.racine.first_child() {
            self.racine.remove(&e);
        }
        let mut elements: Vec<Element> =
            std::fs::read_dir(dossier()).map(|d| d.flatten().filter_map(|e| lire(&e.path())).collect()).unwrap_or_default();
        // Applications d'abord, puis dossiers, puis fichiers ; par nom
        elements.sort_by_key(|e| {
            let rang = match e.genre {
                Genre::Appli(_) | Genre::Lanceur => 0,
                Genre::Lien(_) | Genre::Dossier => 1,
                Genre::Fichier => 2,
            };
            (rang, e.nom.to_lowercase())
        });
        for e in elements {
            self.racine.insert(&self.icone(e), -1);
        }
    }

    fn ouvrir(&self, e: &Element) {
        match &e.genre {
            Genre::Appli(id) => (self.ouvrir_appli)(id),
            // Adresse réseau (NAS, serveur) : Fichiers s'en charge et connecte le partage au besoin
            Genre::Lien(url) if !url.is_empty() && !url.starts_with("http") => crate::lancer(&["nautilus", "--new-window", url]),
            Genre::Lien(url) if !url.is_empty() => crate::lancer(&["gio", "open", url]),
            Genre::Lanceur => crate::lancer(&["gio", "launch", &e.chemin.to_string_lossy()]),
            _ => crate::lancer(&["gio", "open", &e.chemin.to_string_lossy()]),
        }
    }

    fn icone(self: &Rc<Self>, e: Element) -> gtk::Widget {
        let boite = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let image = image_element(&e);
        image.set_halign(gtk::Align::Center);
        boite.append(&image);
        let nom = gtk::Label::new(Some(&e.nom));
        nom.set_wrap(true);
        nom.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        nom.set_lines(2);
        nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
        nom.set_max_width_chars(11);
        nom.set_justify(gtk::Justification::Center);
        nom.add_css_class("nom-icone-bureau");
        boite.append(&nom);
        if e.raccourci && !matches!(e.genre, Genre::Appli(_) | Genre::Lanceur) {
            boite.set_tooltip_text(Some(&format!("{}\n{}", e.nom, tr("Raccourci"))));
        } else {
            boite.set_tooltip_text(Some(&e.nom));
        }
        let b = gtk::Button::builder().child(&boite).build();
        b.add_css_class("icone-bureau");
        // Double-clic : ouvrir (un simple clic sélectionne, comme sous Windows)
        let double = gtk::GestureClick::new();
        double.set_button(1);
        double.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let (moi, e2) = (Rc::downgrade(self), e.clone());
            double.connect_pressed(move |g, n, _, _| {
                if let Some(w) = g.widget() {
                    w.grab_focus();
                }
                if n == 2 {
                    if let Some(b) = moi.upgrade() {
                        b.ouvrir(&e2);
                    }
                }
            });
        }
        b.add_controller(double);
        // Entrée au clavier
        {
            let (moi, e2) = (Rc::downgrade(self), e.clone());
            let touches = gtk::EventControllerKey::new();
            touches.connect_key_pressed(move |_, t, _, _| {
                if matches!(t, gdk::Key::Return | gdk::Key::KP_Enter) {
                    if let Some(b) = moi.upgrade() {
                        b.ouvrir(&e2);
                    }
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            b.add_controller(touches);
        }
        // Menu : Ouvrir, Retirer du bureau / Mettre à la corbeille
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let bulle = gtk::Popover::new();
        bulle.set_child(Some(&menu));
        bulle.set_parent(&b);
        {
            let (moi, e2) = (Rc::downgrade(self), e.clone());
            menu.append(&crate::entree_menu(&tr("Ouvrir"), &bulle, move || {
                if let Some(b) = moi.upgrade() {
                    b.ouvrir(&e2);
                }
            }));
        }
        // Renommer (v13.2) : le nom affiché ; pour un raccourci, jamais le fichier d'origine.
        // Petite fenêtre au premier plan (le fond d'écran, lui, ne reçoit pas le clavier).
        {
            let (e2, b2) = (e.clone(), b.clone());
            menu.append(&crate::entree_menu(&tr("Renommer"), &bulle, move || {
                if let Some(app) = b2.root().and_downcast::<gtk::Window>().and_then(|f| f.application()) {
                    fenetre_renommer(&app, &e2);
                }
            }));
        }
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let chemin = e.chemin.clone();
        if e.raccourci {
            // Seul le raccourci disparaît, jamais le fichier d'origine
            menu.append(&crate::entree_menu(&tr("Retirer du bureau"), &bulle, move || {
                let _ = std::fs::remove_file(&chemin);
            }));
        } else {
            let c = crate::entree_menu(&tr("Mettre à la corbeille"), &bulle, move || {
                let _ = gio::File::for_path(&chemin).trash(gio::Cancellable::NONE);
            });
            c.add_css_class("danger");
            menu.append(&c);
        }
        crate::attacher_menu_contextuel(&b, &bulle);
        b.upcast()
    }
}

/// Fenêtre « Renommer » (couche du dessus, clavier pris) : Entrée valide, Échap annule
fn fenetre_renommer(app: &gtk::Application, e: &Element) {
    use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
    let f = gtk::Window::builder().application(app).decorated(false).title(tr("Renommer")).build();
    f.add_css_class("fenetre-bas");
    f.add_css_class("flashos");
    f.init_layer_shell();
    f.set_layer(Layer::Overlay);
    f.set_namespace(Some("flashos-renommer"));
    f.set_keyboard_mode(KeyboardMode::Exclusive);
    let carte = gtk::Box::new(gtk::Orientation::Vertical, 12);
    carte.add_css_class("carte-renommer");
    let titre = gtk::Label::new(Some(&tr("Renommer")));
    titre.add_css_class("titre-menu");
    titre.set_xalign(0.0);
    carte.append(&titre);
    let champ = gtk::Entry::new();
    champ.set_width_chars(30);
    champ.set_text(&e.nom);
    carte.append(&champ);
    let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    rangee.set_halign(gtk::Align::End);
    let annuler = gtk::Button::with_label(&tr("Annuler"));
    annuler.add_css_class("secondaire");
    let ok = gtk::Button::with_label(&tr("Renommer"));
    ok.add_css_class("principal");
    rangee.append(&annuler);
    rangee.append(&ok);
    carte.append(&rangee);
    f.set_child(Some(&carte));
    let valider = {
        let (e2, ch, f2) = (e.clone(), champ.clone(), f.clone());
        move || {
            let nouveau = ch.text().trim().to_string();
            if !nouveau.is_empty() && nouveau != e2.nom {
                let _ = renommer(&e2, &nouveau);
            }
            f2.destroy();
        }
    };
    let v2 = valider.clone();
    champ.connect_activate(move |_| v2());
    ok.connect_clicked(move |_| valider());
    let f2 = f.clone();
    annuler.connect_clicked(move |_| f2.destroy());
    let touches = gtk::EventControllerKey::new();
    let f2 = f.clone();
    touches.connect_key_pressed(move |_, t, _, _| {
        if t == gdk::Key::Escape {
            f2.destroy();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    f.add_controller(touches);
    f.present();
    champ.grab_focus();
    // Le nom sans l'extension est sélectionné, comme sous Windows
    let nom = e.nom.clone();
    let fin = nom.rfind('.').filter(|&i| i > 0 && !matches!(e.genre, Genre::Dossier)).map(|i| nom[..i].chars().count()).unwrap_or(nom.chars().count());
    champ.select_region(0, fin as i32);
}

/// Nouveau nom : lanceur → champ Name= ; lien ou fichier → le nom dans le dossier Bureau
/// (l'extension est gardée si on l'a oubliée : « Rapport » → « Rapport.docx »)
fn renommer(e: &Element, nouveau: &str) -> std::io::Result<()> {
    let nouveau = nouveau.replace('/', "-");
    if e.chemin.extension().map(|x| x == "desktop").unwrap_or(false) {
        let texte = std::fs::read_to_string(&e.chemin)?;
        let mut fait = false;
        let lignes: Vec<String> = texte
            .lines()
            .map(|l| {
                if !fait && l.starts_with("Name=") {
                    fait = true;
                    format!("Name={nouveau}")
                } else {
                    l.to_string()
                }
            })
            .collect();
        return std::fs::write(&e.chemin, lignes.join("\n") + "\n");
    }
    let mut nom = nouveau.clone();
    if let Some(ext) = e.nom.rsplit_once('.').map(|(_, x)| x).filter(|_| matches!(e.genre, Genre::Fichier)) {
        if !nom.to_lowercase().ends_with(&format!(".{}", ext.to_lowercase())) {
            nom = format!("{nom}.{ext}");
        }
    }
    let cible = dossier().join(&nom);
    if cible.symlink_metadata().is_ok() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "existe"));
    }
    std::fs::rename(&e.chemin, cible)
}

/// Le fond d'écran accepte les fichiers et dossiers glissés depuis Fichiers
pub fn accepter_depots(cible: &impl IsA<gtk::Widget>) {
    let depot = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY | gdk::DragAction::LINK);
    depot.connect_drop(|_, valeur, _, _| {
        let Ok(liste) = valeur.get::<gdk::FileList>() else { return false };
        for f in liste.files() {
            let _ = ajouter_fichier(&f);
        }
        true
    });
    cible.add_controller(depot);
}

pub const STYLE: &str = r#"
.icones-bureau { margin: 26px 0 0 118px; }
.icones-bureau flowboxchild { padding: 0; background: none; }
button.icone-bureau { background: none; background-image: none; border: 1.5px solid transparent; box-shadow: none; border-radius: 12px;
  padding: 8px 4px 6px 4px; min-width: 96px; transition: background-color 120ms ease-out; }
button.icone-bureau:hover { background-color: alpha(white, 0.10); }
button.icone-bureau:focus-visible, button.icone-bureau:focus { background-color: alpha(#FFC23D, 0.16); border-color: alpha(#FFC23D, 0.55); }
.nom-icone-bureau { color: #FFFFFF; font-size: 12.5px; font-weight: 500; text-shadow: 0 1px 3px alpha(black, 0.9); }
.miniature-bureau { border-radius: 6px; }
.carte-renommer { background-color: alpha(#0E1426, 0.97); border: 1px solid alpha(white, 0.12); border-radius: 18px; padding: 18px 20px; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echappement_exec() {
        assert_eq!(echapper("flatpak"), "flatpak");
        assert_eq!(echapper("/opt/Mon Appli/app"), "\"/opt/Mon Appli/app\"");
        assert_eq!(echapper("a$b"), "\"a\\$b\"");
    }

    #[test]
    fn types_de_documents() {
        assert_eq!(icone_document("Rapport.DOCX"), Some("d-texte"));
        assert_eq!(icone_document("budget.ods"), Some("d-tableur"));
        assert_eq!(icone_document("photo.jpg"), None);
    }
}
