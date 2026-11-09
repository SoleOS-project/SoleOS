//! Choix de la Bureautique (LibreOffice) : Texte, Tableur, Présentation, PDF
//! et les derniers documents ouverts.

use gtk::prelude::*;
use std::path::{Path, PathBuf};

use crate::icones;
use crate::lancer;
use crate::tr::tr;

const BUREAUTIQUE: &[&str] = &["doc", "docx", "odt", "rtf", "xls", "xlsx", "ods", "csv", "ppt", "pptx", "odp", "pdf"];

/// LibreOffice est-il installé ?
pub fn disponible() -> bool {
    std::env::var_os("FLASHOS_TEST_BUREAUTIQUE").is_some()
        || ["/usr/bin/libreoffice", "/usr/local/bin/libreoffice"].iter().any(|c| Path::new(c).exists())
}

fn icone_pour(ext: &str) -> &'static str {
    match ext {
        "xls" | "xlsx" | "ods" | "csv" => "d-tableur",
        "ppt" | "pptx" | "odp" => "d-presentation",
        "pdf" => "d-pdf",
        _ => "d-texte",
    }
}

/// Ouvre un document : PDF avec l'application par défaut, le reste avec LibreOffice.
fn ouvrir_document(chemin: &Path) {
    let c = chemin.to_string_lossy().to_string();
    if extension(chemin) == "pdf" {
        lancer(&["xdg-open", &c]);
    } else {
        lancer(&["libreoffice", &c]);
    }
}

fn extension(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Décode les %xx d'une adresse file://
fn decoder(url: &str) -> Option<PathBuf> {
    let reste = url.strip_prefix("file://")?;
    let o = reste.as_bytes();
    let mut v = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        if o[i] == b'%' && i + 2 < o.len() {
            if let Ok(n) = u8::from_str_radix(std::str::from_utf8(&o[i + 1..i + 3]).ok()?, 16) {
                v.push(n);
                i += 3;
                continue;
            }
        }
        v.push(o[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8(v).ok()?))
}

/// Documents récents (fichier des récents de GTK, rempli par LibreOffice, Fichiers…)
pub fn recents(nombre: usize) -> Vec<(PathBuf, String)> {
    let Some(maison) = std::env::var_os("HOME") else { return Vec::new() };
    let fichier = PathBuf::from(maison).join(".local/share/recently-used.xbel");
    let Ok(texte) = std::fs::read_to_string(fichier) else { return Vec::new() };
    let mut liste: Vec<(PathBuf, String)> = Vec::new();
    for morceau in texte.split("<bookmark ").skip(1) {
        let attribut = |nom: &str| -> Option<String> {
            let debut = morceau.find(&format!("{nom}=\""))? + nom.len() + 2;
            let fin = morceau[debut..].find('"')? + debut;
            Some(morceau[debut..fin].replace("&amp;", "&"))
        };
        let Some(chemin) = attribut("href").and_then(|h| decoder(&h)) else { continue };
        if !BUREAUTIQUE.contains(&extension(&chemin).as_str()) || !chemin.exists() {
            continue;
        }
        let quand = attribut("modified").or_else(|| attribut("visited")).unwrap_or_default();
        liste.retain(|(c, _)| *c != chemin);
        liste.push((chemin, quand));
    }
    liste.sort_by(|a, b| b.1.cmp(&a.1));
    liste.truncate(nombre);
    liste
}

/// « aujourd'hui », « hier », sinon la date
fn quand_lisible(iso: &str) -> String {
    let Ok(d) = gtk::glib::DateTime::from_iso8601(iso, None) else { return String::new() };
    let Ok(d) = d.to_local() else { return String::new() };
    let Ok(maintenant) = gtk::glib::DateTime::now_local() else { return String::new() };
    let jour = |x: &gtk::glib::DateTime| (x.year(), x.day_of_year());
    if jour(&d) == jour(&maintenant) {
        return tr("aujourd'hui");
    }
    if let Ok(h) = maintenant.add_days(-1) {
        if jour(&d) == jour(&h) {
            return tr("hier");
        }
    }
    d.format("%x").map(|s| s.to_string()).unwrap_or_default()
}

fn carte(icone: &str, nom: &str, detail: &str) -> gtk::Button {
    let ligne = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let image = icones::image(icone, 44);
    image.set_margin_bottom(4);
    ligne.append(&image);
    let n = gtk::Label::new(Some(nom));
    n.add_css_class("nom-doc");
    let d = gtk::Label::new(Some(detail));
    d.add_css_class("detail-doc");
    ligne.append(&n);
    ligne.append(&d);
    let b = gtk::Button::builder().child(&ligne).build();
    b.add_css_class("carte-doc");
    b
}

/// Remplit la bulle (appelé à chaque ouverture : les documents récents changent).
/// `ouverture` lance l'animation « Ouverture… » sur la tuile.
pub fn remplir(menu: &gtk::Box, bulle: &gtk::Popover, ouverture: impl Fn() + Clone + 'static) {
    while let Some(e) = menu.first_child() {
        menu.remove(&e);
    }
    let titre = gtk::Label::new(Some(&tr("Bureautique")));
    titre.add_css_class("titre-bureautique");
    titre.set_xalign(0.0);
    menu.append(&titre);
    let sous = gtk::Label::new(Some(&tr("Créer un document ou en ouvrir un")));
    sous.add_css_class("titre-menu");
    sous.set_xalign(0.0);
    menu.append(&sous);

    let grille = gtk::Grid::new();
    grille.set_row_spacing(8);
    grille.set_column_spacing(8);
    grille.set_column_homogeneous(true);
    let choix: [(&str, String, String, &str); 4] = [
        ("d-texte", tr("Texte"), tr(".docx, .odt"), "--writer"),
        ("d-tableur", tr("Tableur"), tr(".xlsx, .ods, .csv"), "--calc"),
        ("d-presentation", tr("Présentation"), tr(".pptx, .odp"), "--impress"),
        ("d-pdf", tr("PDF"), tr("lire un PDF"), ""),
    ];
    for (i, (icone, nom, detail, option)) in choix.into_iter().enumerate() {
        let b = carte(icone, &nom, &detail);
        let (bulle, ouverture) = (bulle.clone(), ouverture.clone());
        b.connect_clicked(move |_| {
            bulle.popdown();
            if option.is_empty() {
                choisir_pdf();
            } else {
                lancer(&["libreoffice", option]);
                ouverture();
            }
        });
        grille.attach(&b, i as i32, 0, 1, 1);
    }
    menu.append(&grille);

    // Petits écrans : moins de documents récents, pour que la bulle tienne au-dessus des tuiles
    let hauteur = bulle
        .parent()
        .and_then(|p| p.root())
        .and_downcast::<gtk::Window>()
        .and_then(|f| crate::theme::taille_ecran(&f))
        .map(|(_, h)| h)
        .unwrap_or(768);
    let docs = recents(if hauteur < 560 { 0 } else if hauteur < 700 { 2 } else { 4 });
    if !docs.is_empty() {
        let t = gtk::Label::new(Some(&tr("Documents récents")));
        t.add_css_class("titre-menu");
        t.set_xalign(0.0);
        t.set_margin_top(6);
        menu.append(&t);
        let liste = gtk::Grid::new();
        liste.set_column_spacing(6);
        liste.set_column_homogeneous(true);
        for (i, (chemin, quand)) in docs.into_iter().enumerate() {
            let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            ligne.append(&icones::image(icone_pour(&extension(&chemin)), 24));
            let nom = gtk::Label::new(Some(&chemin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            nom.set_xalign(0.0);
            nom.set_hexpand(true);
            nom.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            nom.set_max_width_chars(18);
            nom.add_css_class("texte-menu");
            ligne.append(&nom);
            let q = gtk::Label::new(Some(&quand_lisible(&quand)));
            q.add_css_class("detail-doc");
            ligne.append(&q);
            let b = gtk::Button::builder().child(&ligne).build();
            b.add_css_class("menu");
            let (bulle, ouverture) = (bulle.clone(), ouverture.clone());
            b.connect_clicked(move |_| {
                bulle.popdown();
                ouvrir_document(&chemin);
                if extension(&chemin) != "pdf" {
                    ouverture();
                }
            });
            liste.attach(&b, (i % 2) as i32, (i / 2) as i32, 1, 1);
        }
        menu.append(&liste);
    }
}

/// Choix d'un fichier PDF (dossier Documents), ouvert avec l'application par défaut
fn choisir_pdf() {
    let dialogue = gtk::FileChooserNative::new(Some(&tr("Ouvrir un PDF")), None::<&gtk::Window>, gtk::FileChooserAction::Open, Some(&tr("Ouvrir")), Some(&tr("Annuler")));
    let filtre = gtk::FileFilter::new();
    filtre.set_name(Some(&tr("Documents PDF")));
    filtre.add_suffix("pdf");
    filtre.add_mime_type("application/pdf");
    dialogue.add_filter(&filtre);
    if let Some(dossier) = gtk::glib::user_special_dir(gtk::glib::UserDirectory::Documents) {
        let _ = dialogue.set_current_folder(Some(&gtk::gio::File::for_path(dossier)));
    }
    dialogue.connect_response(|d, r| {
        if r == gtk::ResponseType::Accept {
            if let Some(p) = d.file().and_then(|f| f.path()) {
                ouvrir_document(&p);
            }
        }
        d.destroy();
    });
    dialogue.show();
    // garder le dialogue en vie jusqu'à la réponse
    std::mem::forget(dialogue);
}
