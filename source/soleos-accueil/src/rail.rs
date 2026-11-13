//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – le rail des applications ouvertes (à gauche, à droite ou en bas de l'écran).
//!
//! Masquage automatique (par défaut) : quand l'application au premier plan occupe tout l'écran
//! (agrandie), le rail se replie en une fine languette contre le bord ; il revient dès que la
//! souris s'en approche. En plein écran (vidéo, présentation), il disparaît complètement.
//! Réglage gardé dans ~/.config/soleos/rail.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cote {
    Gauche,
    Droite,
    Bas,
}

impl Cote {
    pub fn code(self) -> &'static str {
        match self {
            Cote::Gauche => "gauche",
            Cote::Droite => "droite",
            Cote::Bas => "bas",
        }
    }
    fn depuis(code: &str) -> Cote {
        match code {
            "droite" => Cote::Droite,
            "bas" => Cote::Bas,
            _ => Cote::Gauche,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reglage {
    pub cote: Cote,
    /// Se replie quand une application occupe l'écran
    pub auto: bool,
}

impl Default for Reglage {
    fn default() -> Self {
        Reglage { cote: Cote::Gauche, auto: true }
    }
}

fn fichier() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/soleos/rail"))
}

pub fn lire() -> Reglage {
    let mut r = Reglage::default();
    let Some(f) = fichier() else { return r };
    let Ok(t) = std::fs::read_to_string(f) else { return r };
    for l in t.lines() {
        match l.trim().split_once('=') {
            Some(("cote", v)) => r.cote = Cote::depuis(v.trim()),
            Some(("auto", v)) => r.auto = v.trim() != "0",
            _ => {}
        }
    }
    r
}

pub fn ecrire(r: Reglage) {
    let Some(f) = fichier() else { return };
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, format!("cote={}\nauto={}\n", r.cote.code(), if r.auto { 1 } else { 0 }));
}

thread_local! {
    /// Côté où s'ouvrent les bulles du rail (menus des applis ouvertes, marche/arrêt)
    static POSITION_BULLES: Cell<gtk::PositionType> = const { Cell::new(gtk::PositionType::Right) };
}

/// Côté où ouvrir une bulle attachée au rail
pub fn position_bulles() -> gtk::PositionType {
    POSITION_BULLES.with(|p| p.get())
}

pub struct Rail {
    pub fenetre: gtk::Window,
    contenu: gtk::Box,
    bandeau: gtk::Box,
    separateurs: Vec<gtk::Separator>,
    /// Fine languette affichée quand le rail est replié
    bord: gtk::Box,
    survol: gtk::EventControllerMotion,
    reglage: Cell<Reglage>,
    /// Une application agrandie est au premier plan
    masquer: Cell<bool>,
    /// Une application est en plein écran au premier plan
    plein_ecran: Cell<bool>,
    /// Déplié par le passage de la souris
    deplie_survol: Cell<bool>,
    attente: RefCell<Option<glib::SourceId>>,
}

impl Rail {
    pub fn nouveau(fenetre: &gtk::Window, contenu: &gtk::Box, bandeau: &gtk::Box, separateurs: Vec<gtk::Separator>) -> Rc<Rail> {
        let racine = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let bord = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bord.add_css_class("rail-bord");
        bord.set_visible(false);
        racine.append(contenu);
        racine.append(&bord);
        fenetre.set_child(Some(&racine));
        let survol = gtk::EventControllerMotion::new();
        fenetre.add_controller(survol.clone());
        let r = Rc::new(Rail {
            fenetre: fenetre.clone(),
            contenu: contenu.clone(),
            bandeau: bandeau.clone(),
            separateurs,
            bord,
            survol: survol.clone(),
            reglage: Cell::new(lire()),
            masquer: Cell::new(false),
            plein_ecran: Cell::new(false),
            deplie_survol: Cell::new(false),
            attente: RefCell::new(None),
        });
        let moi = Rc::downgrade(&r);
        survol.connect_enter(move |_, _, _| {
            if let Some(r) = moi.upgrade() {
                if let Some(id) = r.attente.borrow_mut().take() {
                    id.remove();
                }
                if !r.deplie_survol.get() {
                    r.deplie_survol.set(true);
                    r.mettre_a_jour();
                }
            }
        });
        let moi = Rc::downgrade(&r);
        survol.connect_leave(move |_| {
            let Some(r) = moi.upgrade() else { return };
            r.replier_plus_tard();
        });
        r.appliquer(r.reglage.get());
        r
    }

    /// Replie le rail un peu après le départ de la souris (sauf si un menu du rail est ouvert)
    fn replier_plus_tard(self: &Rc<Self>) {
        if let Some(id) = self.attente.borrow_mut().take() {
            id.remove();
        }
        let moi = Rc::downgrade(self);
        let id = glib::timeout_add_local(Duration::from_millis(700), move || {
            let Some(r) = moi.upgrade() else { return glib::ControlFlow::Break };
            if r.survol.contains_pointer() || bulle_ouverte(r.contenu.upcast_ref()) {
                // Encore utilisé : on vérifie à nouveau un peu plus tard
                return glib::ControlFlow::Continue;
            }
            r.attente.borrow_mut().take();
            r.deplie_survol.set(false);
            r.mettre_a_jour();
            glib::ControlFlow::Break
        });
        *self.attente.borrow_mut() = Some(id);
    }

    /// Place le rail selon le réglage (côté, masquage)
    pub fn appliquer(&self, r: Reglage) {
        self.reglage.set(r);
        let f = &self.fenetre;
        for e in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            f.set_anchor(e, false);
            f.set_margin(e, 0);
        }
        let (bord_ecran, orientation, bulles) = match r.cote {
            Cote::Gauche => (Edge::Left, gtk::Orientation::Vertical, gtk::PositionType::Right),
            Cote::Droite => (Edge::Right, gtk::Orientation::Vertical, gtk::PositionType::Left),
            Cote::Bas => (Edge::Bottom, gtk::Orientation::Horizontal, gtk::PositionType::Top),
        };
        f.set_anchor(bord_ecran, true);
        POSITION_BULLES.with(|p| p.set(bulles));
        self.contenu.set_orientation(orientation);
        self.bandeau.set_orientation(orientation);
        for s in &self.separateurs {
            s.set_orientation(if orientation == gtk::Orientation::Vertical {
                gtk::Orientation::Horizontal
            } else {
                gtk::Orientation::Vertical
            });
        }
        for c in ["gauche", "droite", "bas"] {
            self.contenu.remove_css_class(c);
        }
        self.contenu.add_css_class(r.cote.code());
        if let Some(racine) = self.bord.parent().and_downcast::<gtk::Box>() {
            racine.set_orientation(if orientation == gtk::Orientation::Vertical {
                gtk::Orientation::Horizontal
            } else {
                gtk::Orientation::Vertical
            });
        }
        if orientation == gtk::Orientation::Vertical {
            self.bord.set_size_request(6, 140);
        } else {
            self.bord.set_size_request(140, 6);
        }
        // Masquage automatique : les fenêtres agrandies prennent tout l'écran (le rail se
        // replie par-dessus). Sinon, le rail garde sa place et les fenêtres s'arrêtent à côté.
        if r.auto {
            f.set_exclusive_zone(0);
        } else {
            f.auto_exclusive_zone_enable();
        }
        // Bulles déjà créées : ouvertes du bon côté
        changer_position_bulles(self.contenu.upcast_ref(), bulles);
        self.deplie_survol.set(false);
        self.mettre_a_jour();
    }

    /// État de l'application au premier plan (appelé à chaque changement de fenêtre)
    pub fn etat_fenetres(&self, agrandie_devant: bool, plein_ecran_devant: bool) {
        let change = agrandie_devant != self.masquer.get() || plein_ecran_devant != self.plein_ecran.get();
        self.masquer.set(agrandie_devant);
        self.plein_ecran.set(plein_ecran_devant);
        if change {
            self.mettre_a_jour();
        }
    }

    fn mettre_a_jour(&self) {
        let r = self.reglage.get();
        if self.plein_ecran.get() && !self.deplie_survol.get() {
            // Plein écran : rien du tout par-dessus la vidéo ou la présentation
            self.fenetre.set_visible(false);
            return;
        }
        self.fenetre.set_visible(true);
        let replie = r.auto && self.masquer.get() && !self.deplie_survol.get();
        self.contenu.set_visible(!replie);
        self.bord.set_visible(replie);
        let bord_ecran = match r.cote {
            Cote::Gauche => Edge::Left,
            Cote::Droite => Edge::Right,
            Cote::Bas => Edge::Bottom,
        };
        self.fenetre.set_margin(bord_ecran, if replie { 0 } else { 12 });
    }
}

/// Une bulle (menu) du rail est-elle ouverte ?
pub(crate) fn bulle_ouverte(w: &gtk::Widget) -> bool {
    if let Some(p) = w.downcast_ref::<gtk::Popover>() {
        if p.is_visible() {
            return true;
        }
    }
    let mut enfant = w.first_child();
    while let Some(e) = enfant {
        if bulle_ouverte(&e) {
            return true;
        }
        enfant = e.next_sibling();
    }
    false
}

fn changer_position_bulles(w: &gtk::Widget, position: gtk::PositionType) {
    if let Some(p) = w.downcast_ref::<gtk::Popover>() {
        p.set_position(position);
    }
    let mut enfant = w.first_child();
    while let Some(e) = enfant {
        changer_position_bulles(&e, position);
        enfant = e.next_sibling();
    }
}
