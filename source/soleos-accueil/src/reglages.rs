//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – Réglages (bouton ⚙ de la barre du haut).
//!
//! Tout ce qu'on règle une fois et qu'on oublie, réuni au même endroit :
//! imprimantes et scanner, Bluetooth, deuxième écran, taille d'affichage,
//! code et verrouillage de l'écran, console (avancé).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::appareils::{self, Panneau};
use crate::icones;
use crate::securite::{self, Etat};
use crate::wifi::en_fond;
use crate::tr::{n_, tr, trv};

pub struct Reglages {
    pub racine: gtk::Box,
    /// Page « Langue et clavier »
    langue: gtk::Box,
    /// En tête du panneau : cartes Wi-Fi et Son (fournies par l'accueil)
    pub tete: gtk::Box,
    /// Fermer le panneau des Réglages (bureau v12)
    pub fermer_cb: RefCell<Option<Box<dyn Fn()>>>,
    pile: gtk::Stack,
    impr: Rc<Panneau>,
    bt: Rc<Panneau>,
    ecran: Rc<Panneau>,
    ligne_bt: gtk::Button,
    ligne_ecran: gtk::Button,
    taille: gtk::Box,
    heure: gtk::Box,
    securite: gtk::Box,
    navigateur: gtk::Box,
    ligne_navigateur: gtk::Button,
    fond: gtk::Box,
    batterie: gtk::Box,
    ligne_batterie: gtk::Button,
    /// Page et carte « Rail » (bureau avec rail seulement)
    page_rail: gtk::Box,
    ligne_rail: gtk::Button,
    /// Appliquer un nouveau réglage du rail (fourni par l'accueil)
    pub changer_rail: RefCell<Option<Box<dyn Fn(crate::rail::Reglage)>>>,
    apropos: gtk::Box,
    etat: RefCell<Etat>,
    /// Changer le fond de l'accueil (fourni par l'accueil) : identifiant d'un fond, ou « perso »
    pub changer_fond: RefCell<Option<Box<dyn Fn(&str)>>>,
    /// Bulle qui contient les réglages (refermée avant de verrouiller, d'ouvrir la console…)
    pub bulle: RefCell<Option<gtk::Popover>>,
    /// Ouvrir la console (fourni par l'accueil)
    pub console: RefCell<Option<Box<dyn Fn()>>>,
    /// Carte « Taille d'affichage » (bureau Plasma : comprise dans « Écran »)
    ligne_taille: gtk::Button,
    camera: gtk::Box,
    sauvegarde: gtk::Box,
}

/// Fuseaux horaires proposés (Europe, pays francophones, puis le reste du monde) ;
/// les noms sont traduits à l'affichage
const FUSEAUX: &[(&str, &str)] = &[
    ("Europe/Paris", n_("Paris, Bruxelles, Genève, Luxembourg")),
    ("Europe/Rome", n_("Rome, Madrid, Berlin, Vienne")),
    ("Europe/London", n_("Londres, Lisbonne, Dublin")),
    ("Europe/Athens", n_("Athènes, Helsinki, Bucarest")),
    ("Europe/Moscow", n_("Moscou, Istanbul")),
    ("Africa/Casablanca", n_("Casablanca")),
    ("Africa/Algiers", n_("Alger, Tunis")),
    ("Africa/Abidjan", n_("Dakar, Abidjan")),
    ("Africa/Lagos", n_("Douala, Kinshasa, Libreville")),
    ("Africa/Johannesburg", n_("Johannesburg, Le Caire")),
    ("Indian/Reunion", n_("La Réunion, Maurice")),
    ("Indian/Mayotte", n_("Mayotte")),
    ("America/Guadeloupe", n_("Guadeloupe, Martinique")),
    ("America/Cayenne", n_("Guyane")),
    ("America/Miquelon", n_("Saint-Pierre-et-Miquelon")),
    ("America/Toronto", n_("Montréal, Québec, Toronto")),
    ("America/New_York", n_("New York")),
    ("America/Chicago", n_("Chicago, Mexico")),
    ("America/Los_Angeles", n_("Los Angeles, Vancouver")),
    ("America/Sao_Paulo", n_("São Paulo, Buenos Aires")),
    ("Pacific/Tahiti", n_("Tahiti")),
    ("Pacific/Noumea", n_("Nouvelle-Calédonie")),
    ("Asia/Dubai", n_("Dubaï")),
    ("Asia/Kolkata", n_("Inde")),
    ("Asia/Shanghai", n_("Pékin, Shanghai")),
    ("Asia/Hong_Kong", n_("Hong Kong")),
    ("Asia/Taipei", n_("Taipei")),
    ("Asia/Singapore", n_("Singapour, Kuala Lumpur")),
    ("Asia/Tokyo", n_("Tokyo, Séoul")),
    ("Australia/Sydney", n_("Sydney")),
    ("UTC", n_("Temps universel (UTC)")),
];

/// (le texte est traduit ici : on peut passer directement le texte français)
fn etiquette(texte: &str, classe: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&tr(texte)));
    l.add_css_class(classe);
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_max_width_chars(46);
    l
}

/// Carte du panneau : tuile de couleur, nom
fn ligne(icone: &str, nom: &str) -> gtk::Button {
    let boite = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let image = icones::image(&format!("r-{icone}"), 40);
    let texte = gtk::Label::new(Some(&tr(nom)));
    texte.set_xalign(0.0);
    texte.set_hexpand(true);
    texte.set_wrap(true);
    texte.add_css_class("nom-reglage");
    boite.append(&image);
    boite.append(&texte);
    let b = gtk::Button::builder().child(&boite).build();
    b.add_css_class("carte-reglage");
    b
}

/// Affiche ou cache une carte (et sa case dans la grille)
/// Réglages → Écran (v13.2) : luminosité (écran intégré) et mode nuit, relus à chaque ouverture
fn bloc_ecran() -> gtk::Box {
    let bloc = gtk::Box::new(gtk::Orientation::Vertical, 10);
    bloc.append(&etiquette(&tr("Écran"), "nom-appareil"));
    let ligne_lumiere = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let image = icones::image("r-luminosite", 28);
    ligne_lumiere.append(&image);
    let lumiere = gtk::Scale::with_range(gtk::Orientation::Horizontal, 5.0, 100.0, 5.0);
    lumiere.set_draw_value(false);
    lumiere.set_hexpand(true);
    lumiere.set_tooltip_text(Some(&tr("Luminosité")));
    ligne_lumiere.append(&lumiere);
    bloc.append(&ligne_lumiere);
    let ligne_nuit = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    ligne_nuit.append(&icones::image("r-nuit", 28));
    let texte = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texte.set_hexpand(true);
    texte.append(&etiquette(&tr("Mode nuit"), "texte-menu"));
    texte.append(&etiquette(&tr("Couleurs plus chaudes, plus reposantes le soir."), "texte-info"));
    ligne_nuit.append(&texte);
    let nuit = gtk::Switch::new();
    nuit.set_valign(gtk::Align::Center);
    ligne_nuit.append(&nuit);
    bloc.append(&ligne_nuit);
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_margin_top(4);
    bloc.append(&sep);
    let en_cours = Rc::new(std::cell::Cell::new(false));
    {
        let en_cours = en_cours.clone();
        lumiere.connect_value_changed(move |s| {
            if en_cours.get() {
                return;
            }
            let v = s.value().round() as u32;
            std::thread::spawn(move || crate::systeme::regler_luminosite(v));
        });
    }
    {
        let en_cours = en_cours.clone();
        nuit.connect_state_set(move |_, oui| {
            if !en_cours.get() {
                crate::systeme::choisir_mode_nuit(oui);
            }
            glib::Propagation::Proceed
        });
    }
    bloc.connect_map(move |_| {
        en_cours.set(true);
        match crate::systeme::luminosite_actuelle() {
            Some(v) => {
                lumiere.set_value(v as f64);
                ligne_lumiere.set_visible(true);
            }
            None => ligne_lumiere.set_visible(false),
        }
        nuit.set_active(crate::systeme::mode_nuit());
        en_cours.set(false);
    });
    bloc
}

fn montrer(b: &gtk::Button, oui: bool) {
    b.set_visible(oui);
    if let Some(p) = b.parent() {
        p.set_visible(oui);
    }
}

/// Carte d'en-tête (Wi-Fi, Son) : tuile, titre, état ; l'accueil y ajoute ses réglages
pub fn carte_tete(icone: &str, titre: &str, etat: &gtk::Label) -> (gtk::Box, gtk::Image) {
    let carte = gtk::Box::new(gtk::Orientation::Vertical, 12);
    carte.add_css_class("carte-tete");
    carte.set_hexpand(true);
    let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let image = icones::image(icone, 44);
    let textes = gtk::Box::new(gtk::Orientation::Vertical, 2);
    textes.set_hexpand(true);
    textes.set_valign(gtk::Align::Center);
    let t = gtk::Label::new(Some(&tr(titre)));
    t.add_css_class("titre-carte");
    t.set_xalign(0.0);
    etat.add_css_class("texte-info");
    etat.set_xalign(0.0);
    textes.append(&t);
    textes.append(etat);
    ligne.append(&image);
    ligne.append(&textes);
    carte.append(&ligne);
    (carte, image)
}

/// Disque de l'ordinateur : taille totale et place libre pour vos fichiers (en octets)
fn disque() -> Option<(u64, u64)> {
    let lire = |prog: &str, args: &[&str]| {
        std::process::Command::new(prog).args(args).output().ok().map(|s| String::from_utf8_lossy(&s.stdout).to_string())
    };
    // Dossier personnel → partition (éventuellement chiffrée) → disque
    let source = lire("findmnt", &["-no", "SOURCE", "-T", &std::env::var("HOME").unwrap_or_else(|_| "/".into())])?;
    let source = source.trim().split('[').next().unwrap_or("").trim().to_string();
    let arbre = lire("lsblk", &["-bnso", "TYPE,SIZE", &source])?;
    let total = arbre.lines().filter_map(|l| {
        let mut c = l.split_whitespace();
        (c.next()? == "disk").then(|| c.next()?.parse::<u64>().ok()).flatten()
    }).next()?;
    let libre = lire("df", &["-B1", "--output=avail", &std::env::var("HOME").unwrap_or_else(|_| "/".into())])?
        .lines().nth(1)?.trim().parse::<u64>().ok()?;
    Some((total, libre))
}

fn vider(b: &gtk::Box) {
    while let Some(e) = b.first_child() {
        b.remove(&e);
    }
}

fn petit(texte: &str, principal: bool) -> gtk::Button {
    let b = gtk::Button::with_label(&tr(texte));
    b.add_css_class("petit");
    if principal {
        b.add_css_class("principal");
    }
    b
}

fn saisie_secrete(indice: &str) -> gtk::PasswordEntry {
    let e = gtk::PasswordEntry::new();
    e.set_show_peek_icon(true);
    e.set_property("placeholder-text", tr(indice));
    e.add_css_class("champ");
    e
}

/// Hauteur maximale de la bulle des réglages : l'écran moins la barre du haut et une marge
fn hauteur_bulle() -> i32 {
    gdk::Display::default()
        .and_then(|d| d.monitors().item(0))
        .and_downcast::<gdk::Monitor>()
        .map(|m| (m.geometry().height() - 110).clamp(300, 900))
        .unwrap_or(600)
}

impl Reglages {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 6);
        racine.set_size_request(480, -1);
        let pile = gtk::Stack::new();
        // Pas d'animation de taille : la bulle change de taille d'un coup (plus sûr)
        pile.set_transition_type(gtk::StackTransitionType::None);
        pile.set_vhomogeneous(false);
        pile.set_hhomogeneous(false);
        // Bulle jamais plus haute que l'écran : le contenu défile si besoin (petits écrans)
        let defil_bulle = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .max_content_height(hauteur_bulle())
            .child(&pile)
            .build();
        racine.append(&defil_bulle);

        // --- Menu principal : en-tête, cartes Wi-Fi et Son, puis tous les réglages en grille ---
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 16);
        let entete = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        entete.append(&icones::image("r-reglages", 44));
        let titre = etiquette(&tr("Réglages"), "titre-panneau");
        titre.set_hexpand(true);
        titre.set_valign(gtk::Align::Center);
        entete.append(&titre);
        let fermer = gtk::Button::with_label("✕");
        fermer.add_css_class("fermer-panneau");
        fermer.set_tooltip_text(Some(&tr("Fermer (Échap)")));
        // Bureau Plasma : les Réglages sont une fenêtre ordinaire, qui a déjà son bouton « fermer »
        fermer.set_visible(!crate::plasma::actif());
        entete.append(&fermer);
        menu.append(&entete);
        let tete = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        tete.set_homogeneous(true);
        menu.append(&tete);
        let l_impr = ligne("appareils", &tr("Imprimantes et scanner"));
        let l_bt = ligne("bluetooth", &tr("Bluetooth"));
        // (v13.2) « Écran » : luminosité et mode nuit, puis le deuxième écran s'il y en a un
        let l_ecran = ligne("ecran", if crate::plasma::actif() { n_("Écran et luminosité") } else { n_("Écran") });
        let l_fond = ligne("image", &tr("Fond d'écran"));
        let l_taille = ligne("taille", &tr("Taille d'affichage"));
        let l_heure = ligne("horloge", &tr("Date et heure"));
        let l_langue = ligne("langue", &tr("Langue et clavier"));
        let l_rail = ligne("rail", &tr("Rail des applications"));
        let l_batterie = ligne("batterie", &tr("Batterie"));
        let l_secu = ligne("cadenas", &tr("Code et verrouillage"));
        let l_nav = ligne("internet", &tr("Navigateur"));
        let l_console = ligne("console", &tr("Console (avancé)"));
        let l_camera = ligne("camera", &tr("Caméra"));
        let l_sauvegarde = ligne("sauvegarde", &tr("Sauvegarde"));
        let l_apropos = ligne("info", &tr("À propos de SoleOS"));
        // v1.0.3 : réparer / réinitialiser, en tuile rouge à part (avant : dans « À propos »)
        let l_reparer = ligne("danger", &tr("Réparer ou réinitialiser"));
        l_reparer.add_css_class("carte-danger");
        let grille = gtk::FlowBox::new();
        grille.set_selection_mode(gtk::SelectionMode::None);
        grille.set_homogeneous(true);
        grille.set_min_children_per_line(1);
        grille.set_max_children_per_line(3);
        grille.set_column_spacing(12);
        grille.set_row_spacing(12);
        for l in [&l_bt, &l_ecran, &l_taille, &l_fond, &l_rail, &l_impr, &l_camera, &l_sauvegarde, &l_langue, &l_heure, &l_batterie, &l_secu, &l_nav, &l_console, &l_apropos, &l_reparer] {
            grille.append(l);
        }
        // Sur la clé (mode essai) : rien à réparer
        if std::path::Path::new("/run/live/medium").exists() {
            montrer(&l_reparer, false);
        }
        menu.append(&grille);
        pile.add_named(&menu, Some("menu"));

        // --- Pages ---
        let impr = Panneau::nouveau("imprimantes");
        let bt = Panneau::nouveau("bluetooth");
        let ecran = Panneau::nouveau("ecran");
        let taille = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let heure = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let securite = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let navigateur = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let fond = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let batterie = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let apropos = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let langue = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let page_rail = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let camera = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let sauvegarde = gtk::Box::new(gtk::Orientation::Vertical, 10);

        let r = Rc::new(Reglages {
            langue: langue.clone(),
            racine,
            tete: tete.clone(),
            fermer_cb: RefCell::new(None),
            pile: pile.clone(),
            impr: impr.clone(),
            bt: bt.clone(),
            ecran: ecran.clone(),
            ligne_bt: l_bt.clone(),
            ligne_ecran: l_ecran.clone(),
            taille: taille.clone(),
            heure: heure.clone(),
            securite: securite.clone(),
            navigateur: navigateur.clone(),
            ligne_navigateur: l_nav.clone(),
            fond: fond.clone(),
            batterie: batterie.clone(),
            ligne_batterie: l_batterie.clone(),
            page_rail: page_rail.clone(),
            ligne_rail: l_rail.clone(),
            changer_rail: RefCell::new(None),
            apropos: apropos.clone(),
            camera: camera.clone(),
            sauvegarde: sauvegarde.clone(),
            etat: RefCell::new(Etat::default()),
            changer_fond: RefCell::new(None),
            bulle: RefCell::new(None),
            console: RefCell::new(None),
            ligne_taille: l_taille.clone(),
        });
        r.ajouter_page("imprimantes", &impr.racine);
        r.ajouter_page("bluetooth", &bt.racine);
        ecran.racine.prepend(&bloc_ecran());
        r.ajouter_page("ecran", &ecran.racine);
        // (v13.2) la Console n'est plus dans les Réglages : éclair et Ctrl + Alt + T
        montrer(&l_console, false);
        r.ajouter_page("taille", &taille);
        r.ajouter_page("heure", &heure);
        r.ajouter_page("securite", &securite);
        r.ajouter_page("navigateur", &navigateur);
        r.ajouter_page("fond", &fond);
        r.ajouter_page("batterie", &batterie);
        r.ajouter_page("apropos", &apropos);
        r.ajouter_page("langue", &langue);
        r.ajouter_page("rail", &page_rail);
        r.ajouter_page("camera", &camera);
        r.ajouter_page("sauvegarde", &sauvegarde);
        // Sans rail (bureau de secours sans couches) : pas de carte « Rail »
        montrer(&l_rail, false);
        if crate::plasma::actif() {
            r.cartes_plasma();
        }

        for (l, nom) in [
            (&l_impr, "imprimantes"),
            (&l_bt, "bluetooth"),
            (&l_camera, "camera"),
            (&l_sauvegarde, "sauvegarde"),
            (&l_ecran, "ecran"),
            (&l_fond, "fond"),
            (&l_taille, "taille"),
            (&l_heure, "heure"),
            (&l_langue, "langue"),
            (&l_rail, "rail"),
            (&l_batterie, "batterie"),
            (&l_secu, "securite"),
            (&l_nav, "navigateur"),
            (&l_apropos, "apropos"),
        ] {
            let moi = Rc::downgrade(&r);
            l.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.ouvrir(nom);
                }
            });
        }
        let moi = Rc::downgrade(&r);
        fermer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.fermer();
            }
        });
        let moi = Rc::downgrade(&r);
        l_reparer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                // Les Réglages se ferment d'abord : sinon l'écran de réparation s'ouvrait derrière eux
                m.fermer();
                glib::timeout_add_local_once(std::time::Duration::from_millis(250), || {
                    crate::lancer(&["soleos-installateur", "--reinitialiser"]);
                });
            }
        });
        let moi = Rc::downgrade(&r);
        l_console.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.fermer();
                if let Some(f) = m.console.borrow().as_ref() {
                    f();
                }
            }
        });
        r
    }

    /// Bureau Plasma : cartes Wi-Fi et Son en tête (le détail s'ouvre dans les modules de Plasma)
    fn cartes_plasma(self: &Rc<Self>) {
        use crate::systeme::{self, Reseau};
        // Wi-Fi : état de la connexion, interrupteur, « Choisir un réseau »
        let info_reseau = gtk::Label::new(None);
        let (carte_wifi, image_wifi) = carte_tete("r-wifi", &tr("Wi-Fi"), &info_reseau);
        let interrupteur = gtk::Switch::new();
        interrupteur.set_valign(gtk::Align::Center);
        if let Some(l) = carte_wifi.first_child().and_downcast::<gtk::Box>() {
            l.append(&interrupteur);
        }
        let choisir = gtk::Button::with_label(&tr("Choisir un réseau"));
        choisir.add_css_class("secondaire");
        choisir.connect_clicked(|_| crate::plasma::ouvrir_module("kcm_networkmanagement"));
        carte_wifi.append(&choisir);
        let maj_wifi = Rc::new(std::cell::Cell::new(false));
        let actualiser_wifi = {
            let (info, image, inter, b) = (info_reseau.clone(), image_wifi.clone(), interrupteur.clone(), maj_wifi.clone());
            move || {
                let (icone, texte) = match systeme::reseau() {
                    Reseau::Wifi(q) => ("wifi", trv("Wi-Fi connecté (signal {q} %)", &[("q", &q.to_string())])),
                    Reseau::Filaire => ("filaire", tr("Connecté par câble")),
                    Reseau::Aucun => ("wifi-off", tr("Pas de connexion")),
                };
                icones::changer(&image, &format!("r-{icone}"));
                info.set_label(&texte);
                let actif = systeme::wifi_actif();
                inter.set_visible(actif.is_some());
                b.set(true);
                inter.set_active(actif.unwrap_or(false));
                b.set(false);
            }
        };
        actualiser_wifi();
        {
            let b = maj_wifi.clone();
            interrupteur.connect_active_notify(move |i| {
                if b.get() {
                    return;
                }
                let oui = i.is_active();
                std::thread::spawn(move || systeme::allumer_wifi(oui));
            });
        }
        // Son : curseur, « Couper le son », « Plus de réglages »
        let info_son = gtk::Label::new(None);
        let (carte_son, image_son) = carte_tete("r-son", &tr("Son"), &info_son);
        let ligne_son = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let curseur = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 5.0);
        curseur.set_draw_value(false);
        curseur.set_hexpand(true);
        let muet = gtk::Button::with_label(&tr("Couper le son"));
        muet.add_css_class("secondaire");
        ligne_son.append(&curseur);
        ligne_son.append(&muet);
        carte_son.append(&ligne_son);
        let plus = gtk::Button::with_label(&tr("Plus de réglages"));
        plus.add_css_class("secondaire");
        plus.connect_clicked(|_| crate::plasma::ouvrir_module("kcm_pulseaudio"));
        carte_son.append(&plus);
        let maj_son = Rc::new(std::cell::Cell::new(false));
        let actualiser_son: Rc<dyn Fn()> = {
            let (info, image, c, m, b) = (info_son.clone(), image_son.clone(), curseur.clone(), muet.clone(), maj_son.clone());
            Rc::new(move || match systeme::volume() {
                Some((niveau, coupe)) => {
                    icones::changer(&image, if coupe || niveau == 0 { "r-son-coupe" } else { "r-son" });
                    info.set_label(&if coupe { tr("Son coupé") } else { format!("{} · {}", tr("Volume"), crate::tr::pourcent(niveau)) });
                    b.set(true);
                    c.set_value(niveau.min(100) as f64);
                    b.set(false);
                    m.set_label(&if coupe { tr("Rétablir le son") } else { tr("Couper le son") });
                    c.set_sensitive(true);
                    m.set_sensitive(true);
                }
                None => {
                    info.set_label(&tr("Son coupé"));
                    c.set_sensitive(false);
                    m.set_sensitive(false);
                }
            })
        };
        actualiser_son();
        {
            let (a, b) = (actualiser_son.clone(), maj_son.clone());
            curseur.connect_value_changed(move |c| {
                if b.get() {
                    return;
                }
                systeme::regler_volume(c.value().round() as u8);
                a();
            });
        }
        {
            let a = actualiser_son.clone();
            muet.connect_clicked(move |_| {
                let coupe = systeme::volume().map(|(_, m)| m).unwrap_or(false);
                systeme::couper_son(!coupe);
                a();
            });
        }
        // Mise à jour régulière (réseau qui change, volume réglé au clavier…)
        let c_wifi = carte_wifi.clone();
        glib::timeout_add_seconds_local(3, move || {
            if c_wifi.root().is_some_and(|r| r.is_visible()) {
                actualiser_wifi();
                actualiser_son();
            }
            glib::ControlFlow::Continue
        });
        self.tete.append(&carte_wifi);
        self.tete.append(&carte_son);
    }

    /// Page fournie par l'accueil (Wi-Fi)
    pub fn ajouter_page_externe(self: &Rc<Self>, nom: &str, contenu: &impl IsA<gtk::Widget>) {
        self.ajouter_page(nom, contenu);
    }

    /// Page avec un bouton « ‹ Réglages » en haut
    fn ajouter_page(self: &Rc<Self>, nom: &str, contenu: &impl IsA<gtk::Widget>) {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let retour = gtk::Button::with_label(&tr("‹  Réglages"));
        retour.add_css_class("retour");
        retour.set_halign(gtk::Align::Start);
        let pile = self.pile.clone();
        retour.connect_clicked(move |_| pile.set_visible_child_name("menu"));
        page.append(&retour);
        page.append(contenu);
        self.pile.add_named(&page, Some(nom));
    }

    fn fermer(&self) {
        if let Some(b) = self.bulle.borrow().as_ref() {
            b.popdown();
        }
        if let Some(f) = self.fermer_cb.borrow().as_ref() {
            f();
        }
    }

    /// À l'ouverture de la bulle : menu principal (lignes visibles selon le matériel)
    pub fn au_debut(self: &Rc<Self>) {
        if crate::plasma::actif() {
            // Plasma règle aussi l'écran principal (luminosité, taille, plusieurs écrans)
            montrer(&self.ligne_bt, appareils::carte_bluetooth());
            montrer(&self.ligne_ecran, true);
            montrer(&self.ligne_taille, false);
        } else {
            montrer(&self.ligne_bt, appareils::carte_bluetooth());
            montrer(&self.ligne_ecran, true);
        }
        montrer(&self.ligne_batterie, crate::systeme::secteur().is_some() || std::env::var_os("SOLEOS_TEST_BATTERIE").is_some());
        self.pile.set_visible_child_name("menu");
        self.marquer_navigateur();
        if let Some(d) = self.racine.first_child().and_downcast::<gtk::ScrolledWindow>() {
            d.set_max_content_height(hauteur_bulle());
        }
        // Panneau du bureau v12 : large (au plus 920 points), pour la grille de cartes
        if self.fermer_cb.borrow().is_some() {
            let largeur = gdk::Display::default()
                .and_then(|d| d.monitors().item(0))
                .and_downcast::<gdk::Monitor>()
                .map(|m| (m.geometry().width() - 140).clamp(320, 920))
                .unwrap_or(900);
            self.racine.set_size_request(largeur, -1);
            if let Some(d) = self.racine.first_child().and_downcast::<gtk::ScrolledWindow>() {
                d.set_max_content_height(hauteur_bulle() - 40);
            }
        }
    }

    /// Ouvre directement une page : « imprimantes », « bluetooth », « ecran », « taille », « securite »
    pub fn ouvrir(self: &Rc<Self>, nom: &str) {
        // Bureau Plasma : Wi-Fi, son, Bluetooth, écran, imprimantes, batterie → modules de Plasma
        if crate::plasma::actif() {
            if let Some(module) = crate::plasma::module_pour(nom) {
                crate::plasma::ouvrir_module(module);
                return;
            }
        }
        match nom {
            "imprimantes" => self.impr.actualiser(),
            "bluetooth" => self.bt.actualiser(),
            "ecran" => self.ecran.actualiser(),
            "taille" => self.remplir_taille(),
            "securite" => self.remplir_securite(),
            "heure" => self.remplir_heure(),
            "navigateur" => self.remplir_navigateur(),
            "fond" => self.remplir_fond(),
            "batterie" => self.remplir_batterie(),
            "apropos" => self.remplir_apropos(),
            "camera" => self.remplir_camera(),
            "sauvegarde" => self.remplir_sauvegarde(),
            "langue" => crate::langue::page_reglages(&self.langue),
            "rail" => self.remplir_rail(),
            _ => {}
        }
        if self.pile.child_by_name(nom).is_some() {
            self.pile.set_visible_child_name(nom);
        }
    }

    fn appliquer_fond(self: &Rc<Self>, id: &str) {
        crate::fonds::enregistrer(id);
        if let Some(f) = self.changer_fond.borrow().as_ref() {
            f(id);
        }
        self.remplir_fond();
    }

    // --- Fond d'écran ----------------------------------------------------------------
    fn remplir_fond(self: &Rc<Self>) {
        use crate::fonds;
        let page = &self.fond;
        vider(page);
        page.append(&etiquette(&tr("Fond d'écran"), "nom-appareil"));
        let contenu = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let actuel = fonds::choisi();

        // Fonds SoleOS
        let grille = gtk::FlowBox::new();
        grille.set_selection_mode(gtk::SelectionMode::None);
        grille.set_min_children_per_line(3);
        grille.set_max_children_per_line(3);
        grille.set_row_spacing(6);
        grille.set_column_spacing(6);
        grille.set_homogeneous(true);
        for f in fonds::FONDS {
            if f.id != "nuit" && fonds::image(f.id).is_none() {
                continue;
            }
            let apercu: gtk::Widget = match fonds::vignette(f.id) {
                Some(p) if f.id != "nuit" => {
                    let image = gtk::Picture::for_filename(&p);
                    image.set_content_fit(gtk::ContentFit::Cover);
                    image.upcast()
                }
                _ => {
                    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    b.add_css_class("vignette-nuit");
                    b.upcast()
                }
            };
            let bouton = choix_fond(&apercu, f.nom, actuel == f.id);
            let (moi, id) = (Rc::downgrade(self), f.id);
            bouton.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.appliquer_fond(id);
                }
            });
            grille.insert(&bouton, -1);
        }
        contenu.append(&grille);

        // Photo personnelle déjà choisie
        if let Some(p) = fonds::perso() {
            let mini = fonds::miniature(&p).unwrap_or(p);
            let image = gtk::Picture::for_filename(&mini);
            image.set_content_fit(gtk::ContentFit::Cover);
            let bouton = choix_fond(&image, "Ma photo", actuel == "perso");
            bouton.set_halign(gtk::Align::Start);
            let moi = Rc::downgrade(self);
            bouton.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.appliquer_fond("perso");
                }
            });
            contenu.append(&bouton);
        }

        // Mes images (dossier Images) : miniatures préparées en arrière-plan
        contenu.append(&etiquette(&tr("Mes images"), "titre-menu"));
        let attente = etiquette(&tr("Recherche des photos…"), "texte-info");
        contenu.append(&attente);
        let photos = gtk::FlowBox::new();
        photos.set_selection_mode(gtk::SelectionMode::None);
        photos.set_min_children_per_line(3);
        photos.set_max_children_per_line(3);
        photos.set_row_spacing(6);
        photos.set_column_spacing(6);
        photos.set_homogeneous(true);
        contenu.append(&photos);
        let moi = Rc::downgrade(self);
        let (attente2, photos2) = (attente.clone(), photos.clone());
        en_fond(
            || {
                fonds::mes_images(24)
                    .into_iter()
                    .filter_map(|p| fonds::miniature(&p).map(|m| (p, m)))
                    .collect::<Vec<_>>()
            },
            move |liste: Vec<(std::path::PathBuf, std::path::PathBuf)>| {
                if liste.is_empty() {
                    attente2.set_label(&tr("Aucune photo dans votre dossier Images."));
                    return;
                }
                attente2.set_visible(false);
                for (source, mini) in liste {
                    let image = gtk::Picture::for_filename(&mini);
                    image.set_content_fit(gtk::ContentFit::Cover);
                    let nom = source.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let bouton = choix_fond(&image, &nom, false);
                    bouton.set_tooltip_text(Some(&source.to_string_lossy()));
                    let moi = moi.clone();
                    bouton.connect_clicked(move |_| {
                        if let Some(m) = moi.upgrade() {
                            if fonds::importer(&source).is_ok() {
                                m.appliquer_fond("perso");
                            }
                        }
                    });
                    photos2.insert(&bouton, -1);
                }
            },
        );

        // Autre photo (clé USB, NAS…)
        let parcourir = petit(&tr("Choisir une autre image…"), false);
        parcourir.set_halign(gtk::Align::Start);
        let moi = Rc::downgrade(self);
        parcourir.connect_clicked(move |b| {
            if let Some(m) = moi.upgrade() {
                m.choisir_image(b);
            }
        });
        // Essais automatiques : ouvre le choix d'image tout seul
        thread_local! { static ESSAI_FAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        if std::env::var_os("SOLEOS_TEST_CHOISIR_FOND").is_some() && !ESSAI_FAIT.with(|f| f.replace(true)) {
            let b = parcourir.clone();
            glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                if b.is_mapped() {
                    b.emit_clicked();
                }
            });
        }
        contenu.append(&parcourir);
        contenu.append(&etiquette(
            "Astuce : dans Fichiers, clic droit sur une photo → « Définir comme arrière-plan ».",
            "texte-info",
        ));

        let defil = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(260)
            .max_content_height(440)
            .propagate_natural_height(true)
            .child(&contenu)
            .build();
        page.append(&defil);
    }

    fn choisir_image(self: &Rc<Self>, depuis: &gtk::Button) {
        // Le panneau des Réglages est posé au-dessus de toutes les fenêtres : la fenêtre de choix
        // (une fenêtre ordinaire) s'ouvrirait derrière lui, invisible. On le cache le temps du choix.
        let panneau = depuis.root().and_then(|r| r.downcast::<gtk::Window>().ok()).filter(|f| gtk4_layer_shell::LayerShell::is_layer_window(f));
        #[allow(deprecated)]
        let choix = gtk::FileChooserNative::new(
            Some(&tr("Choisir une image de fond")),
            None::<&gtk::Window>,
            gtk::FileChooserAction::Open,
            Some(&tr("Choisir")),
            Some(&tr("Annuler")),
        );
        choix.set_modal(false);
        let filtre = gtk::FileFilter::new();
        filtre.set_name(Some(&tr("Images")));
        filtre.add_mime_type("image/*");
        for motif in ["*.jpg", "*.jpeg", "*.png", "*.webp", "*.JPG", "*.JPEG", "*.PNG"] {
            filtre.add_pattern(motif);
        }
        #[allow(deprecated)]
        choix.add_filter(&filtre);
        if let Some(d) = crate::fonds::dossier_images() {
            #[allow(deprecated)]
            let _ = choix.set_current_folder(Some(&gtk::gio::File::for_path(d)));
        }
        let moi = Rc::downgrade(self);
        let p = panneau.clone();
        #[allow(deprecated)]
        choix.connect_response(move |c, reponse| {
            if reponse == gtk::ResponseType::Accept {
                if let (Some(m), Some(chemin)) = (moi.upgrade(), c.file().and_then(|f| f.path())) {
                    if crate::fonds::importer(&chemin).is_ok() {
                        m.appliquer_fond("perso");
                    }
                }
            }
            // Retour au panneau des Réglages, sur la même page
            if let Some(p) = &p {
                p.present();
            }
        });
        if let Some(p) = &panneau {
            p.set_visible(false);
        }
        #[allow(deprecated)]
        choix.show();
        // Garde la boîte de dialogue en vie jusqu'à la réponse
        unsafe { self.racine.set_data("choix-fond", choix) };
        if std::env::var_os("SOLEOS_DEBUG_FOND").is_some() {
            eprintln!("choix du fond ouvert");
        }
    }

    // --- Batterie ----------------------------------------------------------------------
    fn remplir_batterie(self: &Rc<Self>) {
        let page = &self.batterie;
        vider(page);
        page.append(&etiquette(&tr("Batterie"), "nom-appareil"));
        let etat = etiquette("", "texte-menu");
        let conso = etiquette("", "texte-info");
        page.append(&etat);
        page.append(&conso);
        let maj = {
            let (etat, conso) = (etat.clone(), conso.clone());
            move || {
                let niveau = crate::systeme::batterie().map(|(n, _)| n);
                let secteur = crate::systeme::secteur().unwrap_or(true);
                etat.set_label(&match niveau {
                    Some(n) => format!("{}  ·  {}", crate::tr::pourcent(n), if secteur { tr("chargeur branché") } else { tr("sur batterie") }),
                    None => tr("Pas de batterie détectée"),
                });
                conso.set_label(&match crate::systeme::consommation() {
                    Some((w, h)) if w > 0.05 => {
                        let mut t = trv("Consommation actuelle : {w} W", &[("w", &crate::tr::decimal(w as f64, 1))]);
                        if let Some(h) = h {
                            let minutes = (h * 60.0).round() as u32;
                            t.push('\n');
                            t.push_str(&trv(
                                "Autonomie restante estimée : {h} h {m}",
                                &[("h", &(minutes / 60).to_string()), ("m", &format!("{:02}", minutes % 60))],
                            ));
                        }
                        t
                    }
                    Some(_) => tr("Consommation : non mesurée pendant la charge complète."),
                    None => tr("Consommation : non disponible sur cet ordinateur."),
                });
            }
        };
        maj();
        // Mise à jour toutes les 2 secondes tant que la page est affichée
        let (pile, page_b) = (self.pile.clone(), page.clone());
        glib::timeout_add_seconds_local(2, move || {
            if !page_b.is_mapped() || pile.visible_child_name().as_deref() != Some("batterie") {
                return glib::ControlFlow::Break;
            }
            maj();
            glib::ControlFlow::Continue
        });

        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        ligne.set_margin_top(8);
        let texte = etiquette(&tr("Baisser la luminosité sur batterie"), "texte-menu");
        texte.set_hexpand(true);
        let interrupteur = gtk::Switch::new();
        interrupteur.set_valign(gtk::Align::Center);
        interrupteur.set_active(crate::systeme::option_luminosite());
        interrupteur.connect_state_set(|_, oui| {
            crate::systeme::choisir_option_luminosite(oui);
            glib::Propagation::Proceed
        });
        ligne.append(&texte);
        ligne.append(&interrupteur);
        page.append(&ligne);
        page.append(&etiquette(
            "Quand le chargeur est débranché, l'écran baisse de 30 % ; il revient à sa luminosité en le rebranchant.",
            "texte-info",
        ));
        page.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        page.append(&etiquette(
            "Sur batterie, SoleOS passe automatiquement le processeur en mode économie, et le navigateur réduit \
             le travail des onglets en arrière-plan. Les vidéos sont décodées par la carte graphique.",
            "texte-info",
        ));
    }

    // --- À propos ------------------------------------------------------------------------
    fn remplir_apropos(self: &Rc<Self>) {
        let page = &self.apropos;
        vider(page);
        let logo = crate::theme::marque_taille(30);
        logo.set_halign(gtk::Align::Start);
        logo.set_margin_bottom(6);
        page.append(&logo);
        let version = std::fs::read_to_string("/etc/soleos-version").map(|v| v.trim().to_string()).unwrap_or_default();
        page.append(&etiquette(&format!("SoleOS {version}"), "nom-appareil"));
        page.append(&etiquette(&tr("Créé par Giuseppe D'Arco"), "texte-menu"));
        page.append(&etiquette(&tr("© 2026 GDH"), "texte-info"));
        page.append(&etiquette(
            &tr("Gratuit pour les particuliers, les associations, les écoles, les administrations et l'usage interne des entreprises. Toute utilisation commerciale demande une licence de GDH. Basé sur Debian 13."),
            "texte-info",
        ));
        // Vie privée : ce que SoleOS fait (et ne fait pas) de vos données
        page.append(&etiquette(&tr("Votre vie privée"), "titre-menu"));
        page.append(&etiquette(
            &tr("SoleOS n'envoie aucune donnée sur vous : aucun suivi, aucune publicité, aucun compte obligatoire. Vos fichiers sont chiffrés sur le disque et protégés par votre code."),
            "texte-info",
        ));
        let licence = gtk::Button::with_label(&tr("Lire la licence"));
        licence.add_css_class("lien-son");
        licence.set_halign(gtk::Align::Start);
        licence.connect_clicked(|_| crate::lancer(&["gio", "open", "/usr/share/doc/soleos/LICENSE"]));
        page.append(&licence);
        page.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        page.append(&etiquette(&tr("Cet ordinateur"), "titre-menu"));
        if let Some(p) = crate::systeme::prenom() {
            page.append(&etiquette(&trv("Utilisateur : {prenom}", &[("prenom", &p)]), "texte-info"));
        }
        let processeur = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("model name").map(|r| r.trim_start_matches([' ', '\t', ':']).trim().to_string())))
            .unwrap_or_else(|| tr("inconnu"));
        let memoire = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("MemTotal:").and_then(|r| r.trim().trim_end_matches(" kB").trim().parse::<f64>().ok())))
            .map(|k| trv("{n} Go", &[("n", &format!("{:.0}", (k / 1024.0 / 1024.0).round().max(1.0)))]))
            .unwrap_or_else(|| tr("inconnue"));
        // (une ligne par information : deux lignes dans la même étiquette se chevauchaient)
        page.append(&etiquette(&trv("Processeur : {processeur}", &[("processeur", &processeur)]), "texte-info"));
        page.append(&etiquette(&trv("Mémoire : {memoire}", &[("memoire", &memoire)]), "texte-info"));
        if let Some((total, libre)) = disque() {
            let mo = |o: u64| crate::tr::entier_espace(o / 1_000_000);
            let go = |o: u64| crate::tr::entier_espace((o + 500_000_000) / 1_000_000_000);
            page.append(&etiquette(
                &trv("Disque : {go} Go ({mo} Mo), libre : {libre_go} Go ({libre_mo} Mo)",
                    &[("go", &go(total)), ("mo", &mo(total)), ("libre_go", &go(libre)), ("libre_mo", &mo(libre))]),
                "texte-info",
            ));
        }
        // Écran : résolution et taille d'affichage (utile pour l'assistance)
        let ecrans: Vec<String> = appareils::lire_ecrans()
            .iter()
            .filter(|e| e.actif)
            .filter_map(|e| {
                e.courant.map(|m| {
                    let pouces = if e.mm.0 > 0 {
                        let d = ((e.mm.0 as f32).powi(2) + (e.mm.1 as f32).powi(2)).sqrt() / 25.4;
                        trv(", {d} pouces", &[("d", &crate::tr::decimal(d as f64, 1))])
                    } else {
                        String::new()
                    };
                    trv(
                        "{l} × {h}{pouces}, affichage à {z} %",
                        &[
                            ("l", &m.0.to_string()),
                            ("h", &m.1.to_string()),
                            ("pouces", &pouces),
                            ("z", &((e.echelle * 100.0).round() as i32).to_string()),
                        ],
                    )
                })
            })
            .collect();
        if !ecrans.is_empty() {
            page.append(&etiquette(&format!("{} : {}", tr("Écran"), ecrans.join(" ; ")), "texte-info"));
        }
        // v1.0.5 : noyau Linux (stable de Debian, ou récent pour le matériel très récent)
        let ligne_noyau = etiquette(&trv("Noyau Linux : {v}", &[("v", &noyau_actuel())]), "texte-info");
        page.append(&ligne_noyau);
        let bloc_noyau = gtk::Box::new(gtk::Orientation::Vertical, 6);
        page.append(&bloc_noyau);
        remplir_noyau(&ligne_noyau, &bloc_noyau, None);
        // Signaler un problème : message préparé pour l'assistance, avec le relevé
        let signaler = petit("Signaler un problème", false);
        signaler.set_halign(gtk::Align::Start);
        {
            let moi = Rc::downgrade(self);
            signaler.connect_clicked(move |_| {
                if let Some(r) = moi.upgrade() {
                    r.ouvrir("signaler");
                }
            });
        }
        page.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        page.append(&signaler);
        // (v1.0.3 : la réparation est une tuile rouge du menu des Réglages)
    }

    // --- Caméra (v13) ---------------------------------------------------------------
    /// Caméras vues par le pilote standard du noyau (UVC) et essai dans l'application Caméra
    /// Réglages → Sauvegarde (v1) : destination, automatique chaque semaine, maintenant, ouvrir
    fn remplir_sauvegarde(self: &Rc<Self>) {
        use crate::sauvegarde as sv;
        let page = &self.sauvegarde;
        vider(page);
        page.append(&etiquette(&tr("Sauvegarde"), "nom-appareil"));
        page.append(&etiquette(
            &tr("Vos Documents, Images, Musique, Vidéos, Téléchargements et Bureau sont copiés sur une clé USB, un disque ou un dossier du NAS. Rien n'est jamais effacé dans la sauvegarde : un fichier supprimé par erreur s'y retrouve."),
            "texte-info",
        ));
        let dest = etiquette("", "texte-menu");
        dest.set_selectable(true);
        let maj_dest = {
            let dest = dest.clone();
            move || {
                dest.set_label(&match sv::destination() {
                    Some(d) => trv("Destination : {d}", &[("d", &d.to_string_lossy())]),
                    None => tr("Aucune destination choisie."),
                })
            }
        };
        maj_dest();
        page.append(&dest);
        let choisir = petit("Choisir la destination…", false);
        choisir.set_halign(gtk::Align::Start);
        page.append(&choisir);

        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let texte = etiquette(&tr("Chaque semaine, automatiquement (quand la destination est branchée)"), "texte-menu");
        texte.set_hexpand(true);
        let auto = gtk::Switch::new();
        auto.set_valign(gtk::Align::Center);
        auto.set_active(sv::automatique());
        auto.connect_state_set(|_, oui| {
            sv::choisir_automatique(oui);
            glib::Propagation::Proceed
        });
        ligne.append(&texte);
        ligne.append(&auto);
        page.append(&ligne);

        let derniere = etiquette("", "texte-info");
        let maj_derniere = {
            let derniere = derniere.clone();
            move || {
                derniere.set_label(&match sv::derniere_texte() {
                    Some(t) => trv("Dernière sauvegarde : {t}", &[("t", &t)]),
                    None => tr("Aucune sauvegarde pour l'instant."),
                })
            }
        };
        maj_derniere();
        page.append(&derniere);

        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let maintenant = petit("Sauvegarder maintenant", true);
        let ouvrir = petit("Ouvrir la sauvegarde (restaurer)", false);
        boutons.append(&maintenant);
        boutons.append(&ouvrir);
        page.append(&boutons);
        let info = etiquette("", "texte-info");
        page.append(&info);

        {
            let maj = maj_dest.clone();
            choisir.connect_clicked(move |b| {
                let dialogue = gtk::FileChooserNative::new(
                    Some(&tr("Où ranger la sauvegarde ?")),
                    b.root().and_downcast::<gtk::Window>().as_ref(),
                    gtk::FileChooserAction::SelectFolder,
                    Some(&tr("Choisir")),
                    Some(&tr("Annuler")),
                );
                let maj = maj.clone();
                dialogue.connect_response(move |d, r| {
                    if r == gtk::ResponseType::Accept {
                        if let Some(chemin) = d.file().and_then(|f| f.path()) {
                            sv::choisir_destination(&chemin);
                            maj();
                        }
                    }
                    d.destroy();
                });
                dialogue.show();
                std::mem::forget(dialogue);
            });
        }
        ouvrir.connect_clicked(|_| sv::ouvrir());
        {
            let info = info.clone();
            maintenant.connect_clicked(move |b| {
                if sv::destination().is_none() {
                    info.set_label(&tr("Choisissez d'abord une destination."));
                    return;
                }
                b.set_sensitive(false);
                info.set_label(&tr("Sauvegarde en cours… vous pouvez continuer à travailler."));
                let (b, info, maj) = (b.clone(), info.clone(), maj_derniere.clone());
                crate::wifi::en_fond(sv::sauvegarder, move |r| {
                    b.set_sensitive(true);
                    match r {
                        Ok(()) => {
                            info.set_label(&tr("Sauvegarde terminée."));
                            maj();
                        }
                        Err(e) => info.set_label(&trv("La sauvegarde n'a pas pu se faire : {e}", &[("e", &e)])),
                    }
                });
            });
        }
    }

    fn remplir_camera(self: &Rc<Self>) {
        let page = &self.camera;
        vider(page);
        page.append(&etiquette(&tr("Caméra"), "nom-appareil"));
        let mut noms: Vec<String> = Vec::new();
        if let Ok(liste) = std::fs::read_dir("/sys/class/video4linux") {
            let mut entrees: Vec<_> = liste.flatten().map(|e| e.path()).collect();
            entrees.sort();
            for e in entrees {
                // Une caméra crée plusieurs entrées : on garde la première de chaque nom
                if std::fs::read_to_string(e.join("index")).map(|i| i.trim() != "0").unwrap_or(false) {
                    continue;
                }
                if let Ok(n) = std::fs::read_to_string(e.join("name")) {
                    let n = n.trim().trim_end_matches(':').to_string();
                    if !n.is_empty() && !noms.contains(&n) {
                        noms.push(n);
                    }
                }
            }
        }
        if noms.is_empty() {
            page.append(&etiquette(&tr("Aucune caméra détectée. Une webcam USB fonctionne dès qu'on la branche."), "texte-info"));
        } else {
            for n in &noms {
                page.append(&etiquette(&trv("{nom} : prête (pilote standard)", &[("nom", n)]), "texte-info"));
            }
            let essai = petit("Tester la caméra", true);
            essai.set_halign(gtk::Align::Start);
            let info = etiquette("", "texte-info");
            {
                let info = info.clone();
                essai.connect_clicked(move |b| {
                    b.set_sensitive(false);
                    info.set_label(&tr("Ouverture de l'application Caméra…"));
                    let (b, info) = (b.clone(), info.clone());
                    crate::wifi::en_fond(
                        || {
                            let id = "org.gnome.Snapshot";
                            let ok = std::process::Command::new("flatpak").args(["--user", "info", id]).output().map(|s| s.status.success()).unwrap_or(false)
                                || std::process::Command::new("/usr/local/bin/soleos-applications").args(["--installer", id]).status().map(|s| s.success()).unwrap_or(false);
                            if ok {
                                crate::lancer(&["flatpak", "run", id]);
                            }
                            ok
                        },
                        move |ok| {
                            b.set_sensitive(true);
                            info.set_label(&if ok { String::new() } else { tr("L'application Caméra n'a pas pu être installée (connexion à Internet ?).") });
                        },
                    );
                });
            }
            page.append(&essai);
            page.append(&info);
        }
        page.append(&etiquette(&tr("Les applications (navigateur, visio) demandent la permission avant d'utiliser la caméra."), "texte-info"));
    }

    // --- Navigateur ------------------------------------------------------------------
    fn remplir_navigateur(self: &Rc<Self>) {
        let n = &self.navigateur;
        vider(n);
        n.append(&etiquette(&tr("Navigateur"), "nom-appareil"));
        let (extensions, notifications) = crate::navigateur::etat();
        let mut constat = Vec::new();
        if extensions > 0 {
            let n = extensions.to_string();
            constat.push(if extensions > 1 {
                trv("{n} extensions installées", &[("n", &n)])
            } else {
                trv("{n} extension installée", &[("n", &n)])
            });
        }
        if notifications > 0 {
            let n = notifications.to_string();
            constat.push(if notifications > 1 {
                trv("{n} sites autorisés à envoyer des notifications", &[("n", &n)])
            } else {
                trv("{n} site autorisé à envoyer des notifications", &[("n", &n)])
            });
        }
        if !constat.is_empty() {
            let alerte = etiquette(
                &format!(
                    "⚠  {}",
                    trv(
                        "{constat}. Si vous ne les reconnaissez pas, ils peuvent afficher des publicités.",
                        &[("constat", &constat.join(&tr(" et ")))]
                    )
                ),
                "texte-info",
            );
            alerte.add_css_class("attention-texte");
            n.append(&alerte);
        }
        n.append(&etiquette(
            "Des publicités ou des pages s'ouvrent toutes seules ? Le navigateur peut être remis à neuf : extensions, \
             notifications des sites, page d'accueil et moteur de recherche sont effacés. Vos favoris, mots de passe \
             et historique sont gardés.",
            "texte-info",
        ));
        let bouton = petit(&tr("Remettre le navigateur à neuf"), true);
        bouton.set_halign(gtk::Align::Start);
        let resultat = etiquette("", "texte-info");
        n.append(&bouton);
        n.append(&resultat);
        let moi = Rc::downgrade(self);
        let (b, res) = (bouton.clone(), resultat.clone());
        bouton.connect_clicked(move |_| {
            b.set_sensitive(false);
            res.set_label(&tr("Fermeture du navigateur et remise à neuf…"));
            let (b, res, moi) = (b.clone(), res.clone(), moi.clone());
            en_fond(crate::navigateur::remettre_a_neuf, move |r| {
                b.set_sensitive(true);
                match r {
                    Ok(t) => res.set_label(&format!("✓ {t}")),
                    Err(e) => res.set_label(&e),
                }
                if let Some(m) = moi.upgrade() {
                    m.marquer_navigateur();
                }
            });
        });
    }

    /// Point d'attention sur la ligne « Navigateur » quand quelque chose peut produire des publicités
    fn marquer_navigateur(&self) {
        let (e, n) = crate::navigateur::etat();
        if e + n > 0 { self.ligne_navigateur.add_css_class("a-verifier") } else { self.ligne_navigateur.remove_css_class("a-verifier") }
    }

    // --- Taille d'affichage ------------------------------------------------------
    /// Le rail est disponible : sa carte apparaît dans les Réglages
    pub fn activer_rail(&self) {
        montrer(&self.ligne_rail, true);
    }

    // --- Rail des applications -------------------------------------------------------------
    fn remplir_rail(self: &Rc<Self>) {
        let p = &self.page_rail;
        vider(p);
        p.append(&etiquette(&tr("Rail des applications"), "nom-appareil"));
        p.append(&etiquette(&tr("Les applications ouvertes, la batterie et le bouton marche/arrêt."), "texte-info"));
        let actuel = crate::rail::lire();
        p.append(&etiquette(&tr("Position"), "titre-menu"));
        let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        rangee.add_css_class("onglets");
        let mut premier: Option<gtk::ToggleButton> = None;
        for (cote, nom) in [
            (crate::rail::Cote::Gauche, n_("À gauche")),
            (crate::rail::Cote::Droite, n_("À droite")),
            (crate::rail::Cote::Bas, n_("En bas")),
        ] {
            let b = gtk::ToggleButton::with_label(&tr(nom));
            if let Some(pr) = &premier {
                b.set_group(Some(pr));
            } else {
                premier = Some(b.clone());
            }
            b.set_active(cote == actuel.cote);
            let moi = Rc::downgrade(self);
            b.connect_toggled(move |b| {
                if b.is_active() {
                    if let Some(m) = moi.upgrade() {
                        let mut r = crate::rail::lire();
                        r.cote = cote;
                        m.appliquer_rail(r);
                    }
                }
            });
            rangee.append(&b);
        }
        p.append(&rangee);
        p.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let texte = etiquette(&tr("Masquer quand une application occupe l'écran"), "texte-menu");
        texte.set_hexpand(true);
        let interrupteur = gtk::Switch::new();
        interrupteur.set_valign(gtk::Align::Center);
        interrupteur.set_active(actuel.auto);
        let moi = Rc::downgrade(self);
        interrupteur.connect_state_set(move |_, oui| {
            if let Some(m) = moi.upgrade() {
                let mut r = crate::rail::lire();
                r.auto = oui;
                m.appliquer_rail(r);
            }
            glib::Propagation::Proceed
        });
        ligne.append(&texte);
        ligne.append(&interrupteur);
        p.append(&ligne);
        p.append(&etiquette(
            &tr("Le rail se replie en une fine languette contre le bord de l'écran : approchez la souris pour le faire revenir. En plein écran (vidéo, présentation), il disparaît complètement."),
            "texte-info",
        ));
    }

    fn appliquer_rail(&self, r: crate::rail::Reglage) {
        crate::rail::ecrire(r);
        if let Some(f) = self.changer_rail.borrow().as_ref() {
            f(r);
        }
    }

    fn remplir_taille(self: &Rc<Self>) {
        let m = &self.taille;
        vider(m);
        m.append(&etiquette(&tr("Taille d'affichage"), "nom-appareil"));
        m.append(&etiquette(&tr("Agrandit tout l'écran : textes, boutons et fenêtres."), "texte-info"));
        // Mode grand confort (v1) : textes et curseur plus grands, en plus de la taille choisie
        let ligne_confort = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let textes_confort = gtk::Box::new(gtk::Orientation::Vertical, 2);
        textes_confort.set_hexpand(true);
        textes_confort.append(&etiquette(&tr("Mode grand confort"), "texte-menu"));
        textes_confort.append(&etiquette(&tr("Textes plus grands et curseur plus visible, dans toutes les applications."), "texte-info"));
        ligne_confort.append(&textes_confort);
        let confort = gtk::Switch::new();
        confort.set_valign(gtk::Align::Center);
        confort.set_active(crate::systeme::grand_confort());
        confort.connect_state_set(|_, oui| {
            std::thread::spawn(move || crate::systeme::choisir_grand_confort(oui));
            glib::Propagation::Proceed
        });
        ligne_confort.append(&confort);
        m.append(&ligne_confort);
        m.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let choix = appareils::taille_choisie();
        for (valeur, nom) in [
            (None, n_("Automatique (selon l'écran)")),
            (Some(1.0f32), n_("Normale (100 %)")),
            (Some(1.25), n_("Grande (125 %)")),
            (Some(1.5), n_("Très grande (150 %)")),
            (Some(2.0), n_("Maximale (200 %)")),
        ] {
            let coche = match (choix, valeur) {
                (None, None) => true,
                (Some(a), Some(b)) => (a - b).abs() < 0.01,
                _ => false,
            };
            let b = gtk::Button::with_label(&format!("{}  {}", if coche { "✓" } else { "    " }, tr(nom)));
            b.add_css_class("menu");
            if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
            }
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                appareils::choisir_taille(valeur);
                std::thread::spawn(appareils::appliquer_taille);
                if let Some(m) = moi.upgrade() {
                    m.remplir_taille();
                }
            });
            m.append(&b);
        }
        // Taille personnalisée : de 75 % à 200 %, par pas de 5 %
        let presets = [1.0f32, 1.25, 1.5, 2.0];
        let perso = choix.filter(|v| !presets.iter().any(|p| (p - v).abs() < 0.01));
        let ligne_perso = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        ligne_perso.add_css_class("ligne-perso");
        let titre = etiquette(&format!("{}  {}", if perso.is_some() { "✓" } else { "    " }, tr("Personnalisée")), "texte-menu");
        titre.set_hexpand(true);
        titre.set_xalign(0.0);
        let actuelle = perso.or(choix).unwrap_or(1.0);
        let valeur = etiquette(&crate::tr::pourcent((actuelle * 100.0).round() as i32), "texte-menu");
        let moins = gtk::Button::with_label(&tr("−"));
        let plus = gtk::Button::with_label(&tr("+"));
        for b in [&moins, &plus] {
            b.add_css_class("pas");
        }
        ligne_perso.append(&titre);
        ligne_perso.append(&moins);
        ligne_perso.append(&valeur);
        ligne_perso.append(&plus);
        for (b, pas) in [(&moins, -0.05f32), (&plus, 0.05f32)] {
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                let v = ((actuelle + pas) * 20.0).round() / 20.0;
                let v = v.clamp(0.75, 2.0);
                appareils::choisir_taille(Some(v));
                std::thread::spawn(appareils::appliquer_taille);
                if let Some(m) = moi.upgrade() {
                    m.remplir_taille();
                }
            });
        }
        m.append(&ligne_perso);
    }

    // --- Date et heure ----------------------------------------------------------------
    fn remplir_heure(self: &Rc<Self>) {
        let m = &self.heure;
        vider(m);
        let lire = |cle: &str| {
            std::process::Command::new("timedatectl")
                .args(["show", "-p", cle, "--value"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default()
        };
        let fuseau = lire("Timezone");
        let auto = lire("NTP") == "yes";
        let maintenant = glib::DateTime::now_local().ok();
        m.append(&etiquette(&tr("Date et heure"), "nom-appareil"));
        if let Some(d) = &maintenant {
            m.append(&etiquette(&d.format(&tr("%A %e %B %Y, %H:%M")).map(|t| t.to_string()).unwrap_or_default(), "texte-info"));
        }
        let message = etiquette("", "texte-info");
        message.set_visible(false);
        let changer = {
            let moi = Rc::downgrade(self);
            let message = message.clone();
            move |args: Vec<String>| {
                let (moi, message) = (moi.clone(), message.clone());
                en_fond(
                    move || {
                        std::process::Command::new("sudo")
                            .arg("-n")
                            .arg("/usr/local/sbin/soleos-systeme")
                            .args(&args)
                            .output()
                            .map_err(|e| e.to_string())
                            .and_then(|o| if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) })
                    },
                    move |r: Result<(), String>| match r {
                        Ok(()) => {
                            if let Some(m) = moi.upgrade() {
                                m.remplir_heure();
                            }
                        }
                        Err(e) => {
                            message.set_label(&if e.is_empty() { tr("Le réglage n'a pas pu être appliqué.") } else { e });
                            message.set_visible(true);
                        }
                    },
                );
            }
        };
        // Réglage automatique par Internet
        let b = gtk::Button::with_label(&format!("{}  {}", if auto { "✓" } else { "    " }, tr("Réglage automatique (par Internet)")));
        b.add_css_class("menu");
        if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
            l.set_xalign(0.0);
        }
        let c = changer.clone();
        b.connect_clicked(move |_| c(vec!["heure-auto".into(), if auto { "non".into() } else { "oui".into() }]));
        m.append(&b);
        // Réglage manuel (sans Internet)
        if !auto {
            let ligne_manuelle = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ligne_manuelle.set_margin_start(18);
            let saisie = gtk::Entry::new();
            saisie.add_css_class("champ");
            saisie.set_hexpand(true);
            if let Some(d) = &maintenant {
                saisie.set_text(&d.format("%Y-%m-%d %H:%M").map(|t| t.to_string()).unwrap_or_default());
            }
            saisie.set_tooltip_text(Some(&tr("Année-mois-jour heure:minutes")));
            let ok = petit(&tr("Régler"), true);
            let (c, e) = (changer.clone(), saisie.clone());
            ok.connect_clicked(move |_| c(vec!["heure-regler".into(), e.text().trim().to_string()]));
            ligne_manuelle.append(&saisie);
            ligne_manuelle.append(&ok);
            m.append(&ligne_manuelle);
        }
        m.append(&message);
        // Fuseau horaire
        m.append(&etiquette(&tr("Fuseau horaire"), "titre-menu"));
        let liste = gtk::Box::new(gtk::Orientation::Vertical, 0);
        for (zone, nom) in FUSEAUX {
            let coche = fuseau == *zone;
            let b = gtk::Button::with_label(&format!("{}  {}", if coche { "✓" } else { "    " }, tr(nom)));
            b.add_css_class("menu");
            if let Some(l) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
                l.set_xalign(0.0);
            }
            let c = changer.clone();
            let z = zone.to_string();
            b.connect_clicked(move |_| c(vec!["fuseau".into(), z.clone()]));
            liste.append(&b);
        }
        let defil = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(220)
            .max_content_height(220)
            .child(&liste)
            .build();
        m.append(&defil);
    }

    // --- Code et verrouillage -------------------------------------------------------
    fn remplir_securite(self: &Rc<Self>) {
        // Lu tout de suite (rapide) : la bulle garde sa taille une fois affichée
        *self.etat.borrow_mut() = securite::etat();
        self.afficher_securite();
    }

    fn afficher_securite(self: &Rc<Self>) {
        let s = &self.securite;
        vider(s);
        let e = self.etat.borrow().clone();
        s.append(&etiquette(&tr("Code et verrouillage"), "nom-appareil"));

        // Verrouillage automatique
        s.append(&etiquette(&tr("Verrouiller l'écran après"), "titre-menu"));
        let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        rangee.add_css_class("onglets");
        let actuel = securite::delai_verrouillage();
        let mut premier: Option<gtk::ToggleButton> = None;
        for (m, nom) in [(5u32, n_("5 min")), (10, n_("10 min")), (30, n_("30 min")), (60, n_("1 h")), (0, n_("Jamais"))] {
            let b = gtk::ToggleButton::with_label(&tr(nom));
            if let Some(p) = &premier {
                b.set_group(Some(p));
            } else {
                premier = Some(b.clone());
            }
            b.set_active(m == actuel);
            b.connect_toggled(move |b| {
                if b.is_active() {
                    securite::choisir_delai(m);
                }
            });
            rangee.append(&b);
        }
        s.append(&rangee);
        s.append(&etiquette(
            "L'écran se verrouille aussi quand l'ordinateur se met en veille (capot fermé). Raccourci : touche Windows + L.",
            "texte-info",
        ));
        if e.verrouillable {
            let maintenant = petit(&tr("🔒  Verrouiller maintenant"), false);
            maintenant.set_halign(gtk::Align::Start);
            let moi = Rc::downgrade(self);
            maintenant.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.fermer();
                }
                securite::verrouiller();
            });
            s.append(&maintenant);
        }

        s.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        if !e.chiffre {
            s.append(&etiquette(
                &if std::path::Path::new("/run/live/medium").exists() {
                    tr("Essai depuis la clé USB : le code se choisit après l'installation de SoleOS.")
                } else {
                    tr("Les fichiers de cet ordinateur ne sont pas chiffrés (SoleOS installé avant la version 9). « Réparer → Tout effacer » active la protection.")
                },
                "texte-info",
            ));
            return;
        }
        if e.methode == "provisoire" {
            s.append(&etiquette(&tr("Le code se choisit dans l'assistant de bienvenue."), "texte-info"));
            let b = petit(&tr("Ouvrir l'assistant"), true);
            b.set_halign(gtk::Align::Start);
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.fermer();
                }
                crate::lancer(&["soleos-accueil", "--bienvenue"]);
            });
            s.append(&b);
            return;
        }
        // Textes en version « code » ou « mot de passe » (phrases entières, pour la traduction)
        let par_code = e.par_code();
        let selon = move |code: &str, mdp: &str| if par_code { tr(code) } else { tr(mdp) };
        s.append(&etiquette(
            &selon(
                "Vos fichiers sont chiffrés. Ils s'ouvrent au démarrage avec votre code (protégé par la puce de sécurité de l'ordinateur).",
                "Vos fichiers sont chiffrés. Ils s'ouvrent au démarrage avec votre mot de passe.",
            ),
            "texte-info",
        ));

        // Changer mon code
        let zone = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone.add_css_class("zone-mdp");
        zone.set_visible(false);
        let changer = petit(&selon("Changer mon code…", "Changer mon mot de passe…"), false);
        changer.set_halign(gtk::Align::Start);
        let z = zone.clone();
        let autre_zone: Rc<RefCell<Option<gtk::Box>>> = Rc::new(RefCell::new(None));
        let az = autre_zone.clone();
        changer.connect_clicked(move |_| {
            if let Some(a) = az.borrow().as_ref() {
                a.set_visible(false);
            }
            z.set_visible(!z.is_visible());
        });
        if std::env::var_os("SOLEOS_TEST_ZONE").is_some() {
            let c = changer.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || c.emit_clicked());
        }
        s.append(&changer);
        s.append(&zone);
        let ancien = saisie_secrete(&selon("Code actuel (ou clé de secours)", "Mot de passe actuel (ou clé de secours)"));
        let nouveau = saisie_secrete(&selon("Nouveau code (6 chiffres)", "Nouveau mot de passe (8 caractères ou plus)"));
        let encore = saisie_secrete("Encore une fois");
        let message = etiquette("", "texte-info");
        let valider = petit(&tr("Enregistrer"), true);
        valider.set_halign(gtk::Align::End);
        for w in [ancien.upcast_ref::<gtk::Widget>(), nouveau.upcast_ref(), encore.upcast_ref(), message.upcast_ref(), valider.upcast_ref()] {
            zone.append(w);
        }
        let enregistrer = {
            let (a, n, n2, msg, v) = (ancien.clone(), nouveau.clone(), encore.clone(), message.clone(), valider.clone());
            move || {
                let (x, y, z) = (a.text().to_string(), n.text().to_string(), n2.text().to_string());
                if x.trim().is_empty() {
                    msg.set_label(&selon("Tapez d'abord votre code actuel.", "Tapez d'abord votre mot de passe actuel."));
                    return;
                }
                let y_norm = if par_code { securite::normaliser_code(&y) } else { y.clone() };
                let z_norm = if par_code { securite::normaliser_code(&z) } else { z.clone() };
                if par_code && !securite::code_valide(&y) {
                    match securite::conseil_maj(&y) {
                        Some(c) => msg.set_label(&format!("{} {c}", tr("Le nouveau code doit contenir exactement 6 chiffres."))),
                        None => msg.set_label(&tr("Le nouveau code doit contenir exactement 6 chiffres.")),
                    }
                    return;
                }
                if par_code && crate::bienvenue::trop_simple(&y_norm) {
                    msg.set_label(&tr("Ce code est trop facile à deviner. Choisissez-en un autre."));
                    return;
                }
                if !par_code && y.chars().count() < securite::LONGUEUR_MDP {
                    msg.set_label(&tr("Le nouveau mot de passe doit contenir au moins 8 caractères."));
                    return;
                }
                if y_norm != z_norm {
                    msg.set_label(&tr("Les deux nouveaux ne sont pas identiques."));
                    return;
                }
                msg.set_label(&tr("Enregistrement…"));
                v.set_sensitive(false);
                let entree = format!("{x}\n{y_norm}\n");
                let conseil = if par_code { securite::conseil_maj(&x) } else { None };
                let (a, n, n2, msg, v) = (a.clone(), n.clone(), n2.clone(), msg.clone(), v.clone());
                en_fond(
                    move || securite::appeler(&["changer"], &entree),
                    move |r| {
                        v.set_sensitive(true);
                        match r {
                            Ok(_) => {
                                for e in [&a, &n, &n2] {
                                    e.set_text("");
                                }
                                msg.set_label(&format!(
                                    "✓ {}",
                                    selon(
                                        "Nouveau code enregistré. Il servira dès le prochain démarrage.",
                                        "Nouveau mot de passe enregistré. Il servira dès le prochain démarrage.",
                                    )
                                ));
                            }
                            Err(e) => match conseil {
                                Some(c) => msg.set_label(&format!("{e} {c}")),
                                None => msg.set_label(&e),
                            },
                        }
                    },
                );
            }
        };
        let e2 = enregistrer.clone();
        valider.connect_clicked(move |_| e2());
        encore.connect_activate(move |_| enregistrer());

        // Nouvelle clé de secours
        let zone_cle = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone_cle.add_css_class("zone-mdp");
        zone_cle.set_visible(false);
        *autre_zone.borrow_mut() = Some(zone_cle.clone());
        let nouvelle = petit(&tr("Nouvelle clé de secours…"), false);
        nouvelle.set_halign(gtk::Align::Start);
        let (z, autre) = (zone_cle.clone(), zone.clone());
        nouvelle.connect_clicked(move |_| {
            autre.set_visible(false);
            z.set_visible(!z.is_visible());
        });
        s.append(&nouvelle);
        s.append(&zone_cle);
        zone_cle.append(&etiquette(
            "Si vous avez perdu votre papier, ou si quelqu'un a pu le voir : la clé actuelle ne fonctionnera plus.",
            "texte-info",
        ));
        let code_cle = saisie_secrete(&selon("Votre code", "Votre mot de passe"));
        let creer = petit(&tr("Créer une nouvelle clé"), true);
        creer.set_halign(gtk::Align::End);
        let resultat = etiquette("", "texte-info");
        let cle = gtk::Label::new(None);
        cle.add_css_class("cle-secours");
        cle.set_selectable(true);
        cle.set_visible(false);
        zone_cle.append(&code_cle);
        zone_cle.append(&creer);
        zone_cle.append(&resultat);
        zone_cle.append(&cle);
        let creer_cle = {
            let (c, r, k, b) = (code_cle.clone(), resultat.clone(), cle.clone(), creer.clone());
            move || {
                let x = c.text().to_string();
                if x.trim().is_empty() {
                    return;
                }
                r.set_label(&tr("Création…"));
                b.set_sensitive(false);
                let (c, r, k, b) = (c.clone(), r.clone(), k.clone(), b.clone());
                en_fond(
                    move || securite::appeler(&["nouvelle-cle"], &format!("{x}\n")),
                    move |res| {
                        b.set_sensitive(true);
                        c.set_text("");
                        match res.map(|l| securite::cle_dans(&l)) {
                            Ok(Some(nouvelle)) => {
                                r.set_label(&tr("Voici votre nouvelle clé de secours. Écrivez-la sur papier et rangez-la en lieu sûr :"));
                                k.set_label(&securite::cle_sur_deux_lignes(&nouvelle));
                                k.set_visible(true);
                            }
                            Ok(None) => r.set_label(&tr("La clé n'a pas pu être créée.")),
                            Err(e) => r.set_label(&e),
                        }
                    },
                );
            }
        };
        let c2 = creer_cle.clone();
        creer.connect_clicked(move |_| c2());
        code_cle.connect_activate(move |_| creer_cle());
    }
}

/// Vignette cliquable d'un fond (aperçu arrondi + nom, coche si c'est le fond actuel)
fn choix_fond(apercu: &impl IsA<gtk::Widget>, nom: &str, actuel: bool) -> gtk::Button {
    apercu.set_size_request(136, 77);
    apercu.set_overflow(gtk::Overflow::Hidden);
    apercu.add_css_class("vignette");
    let nom = tr(nom);
    let texte = gtk::Label::new(Some(&if actuel { format!("✓ {nom}") } else { nom.clone() }));
    texte.add_css_class("nom-vignette");
    texte.set_ellipsize(gtk::pango::EllipsizeMode::End);
    texte.set_max_width_chars(16);
    let boite = gtk::Box::new(gtk::Orientation::Vertical, 4);
    boite.append(apercu);
    boite.append(&texte);
    let b = gtk::Button::builder().child(&boite).build();
    b.add_css_class("choix-fond");
    if actuel {
        b.add_css_class("actif");
    }
    b
}

fn noyau_actuel() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| s.trim().to_string()).unwrap_or_default()
}

fn sudo_systeme(args: &[&str]) -> (bool, String) {
    match std::process::Command::new("sudo").arg("-n").arg("/usr/local/sbin/soleos-systeme").args(args).output() {
        Ok(s) => (s.status.success(), String::from_utf8_lossy(&s.stdout).to_string()),
        Err(_) => (false, String::new()),
    }
}

/// Réglages → À propos : noyau stable ou récent (essai au prochain démarrage, retour automatique)
fn remplir_noyau(ligne: &gtk::Label, bloc: &gtk::Box, note: Option<String>) {
    let (ligne, bloc) = (ligne.clone(), bloc.clone());
    en_fond(
        || sudo_systeme(&["noyau-etat"]),
        move |(ok, etat)| {
            while let Some(e) = bloc.first_child() {
                bloc.remove(&e);
            }
            if let Some(n) = &note {
                let l = etiquette(n, "texte-info");
                l.set_wrap(true);
                bloc.append(&l);
            }
            if !ok {
                return;
            }
            let val = |k: &str| etat.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
            let (actuel, mode, stable, recent) = (val("actuel"), val("mode"), val("stable"), val("recent"));
            let genre = if !recent.is_empty() && actuel == recent { tr("récent") } else { tr("stable") };
            ligne.set_label(&trv("Noyau Linux : {v} ({genre})", &[("v", &actuel), ("genre", &genre)]));
            if recent.is_empty() {
                return;
            }
            let (texte, cible) = match mode.as_str() {
                "essai" => (tr("Le noyau récent sera essayé au prochain démarrage."), None),
                "recent" => (String::new(), Some(("stable", trv("Revenir au noyau stable ({v})", &[("v", &stable)])))),
                _ => (
                    tr("Ordinateur très récent (Wi-Fi, son ou affichage qui marchent mal) ? Essayez le noyau récent : s'il ne démarre pas bien, SoleOS revient tout seul au noyau stable."),
                    Some(("recent", trv("Essayer le noyau récent ({v})", &[("v", &recent)]))),
                ),
            };
            if !texte.is_empty() {
                let l = etiquette(&texte, "texte-info");
                l.set_wrap(true);
                bloc.append(&l);
            }
            if let Some((choix, libelle)) = cible {
                let b = petit(&libelle, false);
                b.set_halign(gtk::Align::Start);
                let (ligne2, bloc2) = (ligne.clone(), bloc.clone());
                b.connect_clicked(move |b| {
                    b.set_sensitive(false);
                    let (ligne3, bloc3) = (ligne2.clone(), bloc2.clone());
                    en_fond(
                        move || sudo_systeme(&["noyau", choix]),
                        move |(ok, _)| {
                            let note = if ok { tr("Redémarrez l'ordinateur pour appliquer ce choix.") } else { tr("Ce choix n'a pas pu être enregistré.") };
                            remplir_noyau(&ligne3, &bloc3, Some(note));
                        },
                    );
                });
                bloc.append(&b);
            }
        },
    );
}
