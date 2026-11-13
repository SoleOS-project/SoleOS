//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – passage d'une application à l'autre (Alt + Tab), avec les icônes des applications.
//!
//! Le gestionnaire d'affichage lance « soleos-accueil --alttab » à chaque Alt + Tab
//! (« --alttab-arriere » avec Maj). Le sélecteur s'affiche au centre de l'écran, les
//! applications de la plus récemment utilisée à la plus ancienne ; chaque Tab passe à la
//! suivante, et lâcher Alt met l'application choisie au premier plan. Échap annule ;
//! les flèches et la souris marchent aussi.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::fenetres::Gestionnaire;

pub struct Selecteur {
    fenetre: gtk::Window,
    rangee: gtk::Box,
    titre: gtk::Label,
    /// Identifiants des fenêtres proposées et leurs vignettes
    elements: RefCell<Vec<(u32, gtk::Button, String)>>,
    indice: Cell<usize>,
    gestionnaire: Gestionnaire,
    surveillance: RefCell<Option<glib::SourceId>>,
    debut: Cell<Option<Instant>>,
    /// Le sélecteur a reçu le clavier au moins une fois depuis son ouverture
    clavier_recu: Cell<bool>,
}

impl Selecteur {
    pub fn nouveau(app: &gtk::Application, gestionnaire: Gestionnaire) -> Rc<Selecteur> {
        let fenetre = gtk::Window::builder().application(app).decorated(false).resizable(false).build();
        fenetre.add_css_class("alttab");
        fenetre.init_layer_shell();
        fenetre.set_layer(Layer::Overlay);
        fenetre.set_namespace(Some("soleos-alttab"));
        fenetre.set_keyboard_mode(KeyboardMode::Exclusive);
        fenetre.set_exclusive_zone(-1);
        let cadre = gtk::Box::new(gtk::Orientation::Vertical, 10);
        cadre.add_css_class("alttab-cadre");
        let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        rangee.set_halign(gtk::Align::Center);
        let titre = gtk::Label::new(None);
        titre.add_css_class("alttab-titre");
        titre.set_ellipsize(gtk::pango::EllipsizeMode::End);
        titre.set_max_width_chars(60);
        cadre.append(&rangee);
        cadre.append(&titre);
        fenetre.set_child(Some(&cadre));

        let s = Rc::new(Selecteur {
            fenetre: fenetre.clone(),
            rangee,
            titre,
            elements: RefCell::new(Vec::new()),
            indice: Cell::new(0),
            gestionnaire,
            surveillance: RefCell::new(None),
            debut: Cell::new(None),
            clavier_recu: Cell::new(false),
        });

        let touches = gtk::EventControllerKey::new();
        let moi = Rc::downgrade(&s);
        touches.connect_key_pressed(move |_, touche, _, etat| {
            let Some(s) = moi.upgrade() else { return glib::Propagation::Proceed };
            s.clavier_recu.set(true);
            if std::env::var_os("SOLEOS_DEBUG_ALTTAB").is_some() { eprintln!("alttab appui {:?} {:?}", touche, etat); }
            use gdk::Key;
            match touche {
                Key::Tab if etat.contains(gdk::ModifierType::SHIFT_MASK) => s.deplacer(-1),
                Key::Tab | Key::Right | Key::Down => s.deplacer(1),
                Key::ISO_Left_Tab | Key::Left | Key::Up => s.deplacer(-1),
                Key::Escape => s.fermer(),
                Key::Return | Key::KP_Enter | Key::space => s.valider(),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        let moi = Rc::downgrade(&s);
        touches.connect_key_released(move |_, touche, _, _| {
            if std::env::var_os("SOLEOS_DEBUG_ALTTAB").is_some() { eprintln!("alttab relâché {:?}", touche); }
            let Some(s) = moi.upgrade() else { return };
            if matches!(touche, gdk::Key::Alt_L | gdk::Key::Alt_R | gdk::Key::Meta_L | gdk::Key::Meta_R) {
                s.valider();
            }
        });
        let moi = Rc::downgrade(&s);
        touches.connect_modifiers(move |_, m| {
            if std::env::var_os("SOLEOS_DEBUG_ALTTAB").is_some() { eprintln!("alttab modificateurs {:?}", m); }
            if let Some(s) = moi.upgrade() {
                s.clavier_recu.set(true);
            }
            glib::Propagation::Proceed
        });
        fenetre.add_controller(touches);
        let focus = gtk::EventControllerFocus::new();
        let moi = Rc::downgrade(&s);
        focus.connect_enter(move |_| {
            if std::env::var_os("SOLEOS_DEBUG_ALTTAB").is_some() { eprintln!("alttab focus enter"); }
            if let Some(s) = moi.upgrade() {
                s.clavier_recu.set(true);
            }
        });
        fenetre.add_controller(focus);
        let moi = Rc::downgrade(&s);
        fenetre.connect_is_active_notify(move |f| {
            if let Some(s) = moi.upgrade() {
                if f.is_active() {
                    s.clavier_recu.set(true);
                }
            }
        });
        s
    }

    pub fn visible(&self) -> bool {
        self.fenetre.is_visible()
    }

    /// Alt + Tab (sens 1) ou Alt + Maj + Tab (sens -1). `fenetres` : de la plus récente à la plus ancienne.
    pub fn alt_tab(self: &Rc<Self>, fenetres: Vec<(u32, String, gtk::Image)>, sens: i32) {
        if self.visible() {
            self.deplacer(sens);
            return;
        }
        if fenetres.is_empty() {
            return;
        }
        if fenetres.len() == 1 {
            // Une seule application : on la ramène simplement devant (si elle était réduite)
            self.gestionnaire.activer(fenetres[0].0);
            return;
        }
        while let Some(e) = self.rangee.first_child() {
            self.rangee.remove(&e);
        }
        let mut elements = Vec::new();
        for (i, (id, titre, image)) in fenetres.into_iter().enumerate() {
            image.set_pixel_size(64);
            let b = gtk::Button::new();
            b.add_css_class("alttab-element");
            b.set_child(Some(&image));
            b.set_tooltip_text(Some(&titre));
            b.set_focus_on_click(false);
            b.set_can_focus(false);
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(s) = moi.upgrade() {
                    s.indice.set(i);
                    s.valider();
                }
            });
            self.rangee.append(&b);
            elements.push((id, b, titre));
        }
        let n = elements.len();
        *self.elements.borrow_mut() = elements;
        self.indice.set(if sens < 0 { n - 1 } else { 1 });
        self.marquer();
        self.clavier_recu.set(false);
        self.debut.set(Some(Instant::now()));
        self.fenetre.present();
        self.surveiller();
    }

    /// Lâcher d'Alt (aussi quand Alt a été lâché avant l'apparition du sélecteur)
    fn surveiller(self: &Rc<Self>) {
        if let Some(id) = self.surveillance.borrow_mut().take() {
            id.remove();
        }
        let moi = Rc::downgrade(self);
        let id = glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(s) = moi.upgrade() else { return glib::ControlFlow::Break };
            if !s.visible() {
                s.surveillance.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            let ecoule = s.debut.get().map(|d| d.elapsed()).unwrap_or_default();
            let alt_tenu = gdk::Display::default()
                .and_then(|d| d.default_seat())
                .and_then(|siege| siege.keyboard())
                .map(|k| k.modifier_state().contains(gdk::ModifierType::ALT_MASK))
                .unwrap_or(false);
            if std::env::var_os("SOLEOS_DEBUG_ALTTAB").is_some() {
                eprintln!("alttab {:?} clavier={} alt={}", ecoule, s.clavier_recu.get(), alt_tenu);
            }
            // L'état des touches n'est fiable qu'une fois le clavier reçu ; sans clavier au bout
            // d'une seconde, on valide le choix proposé.
            // Alt vu enfoncé : l'état du clavier est à jour
            if alt_tenu {
                s.clavier_recu.set(true);
            }
            if (s.clavier_recu.get() && !alt_tenu) || (!s.clavier_recu.get() && ecoule > Duration::from_millis(1000)) {
                s.surveillance.borrow_mut().take();
                s.valider();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        *self.surveillance.borrow_mut() = Some(id);
    }

    fn deplacer(&self, sens: i32) {
        let n = self.elements.borrow().len() as i32;
        if n == 0 {
            return;
        }
        let i = (self.indice.get() as i32 + sens).rem_euclid(n);
        self.indice.set(i as usize);
        self.marquer();
    }

    fn marquer(&self) {
        let elements = self.elements.borrow();
        for (i, (_, b, titre)) in elements.iter().enumerate() {
            if i == self.indice.get() {
                b.add_css_class("choisi");
                self.titre.set_label(titre);
            } else {
                b.remove_css_class("choisi");
            }
        }
    }

    fn valider(&self) {
        if !self.visible() {
            return;
        }
        let id = self.elements.borrow().get(self.indice.get()).map(|e| e.0);
        self.fermer();
        if let Some(id) = id {
            self.gestionnaire.activer(id);
        }
    }

    fn fermer(&self) {
        self.fenetre.set_visible(false);
        self.elements.borrow_mut().clear();
    }
}

pub const STYLE: &str = r#"
window.alttab { background: transparent; }
.alttab-cadre {
  background-color: rgba(14, 20, 38, 0.94);
  border: 1px solid rgba(255, 255, 255, 0.10);
  border-radius: 22px;
  padding: 18px 20px 14px 20px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
}
.alttab-element {
  background: transparent;
  border: 2px solid transparent;
  border-radius: 16px;
  padding: 12px;
  min-width: 64px;
  min-height: 64px;
  box-shadow: none;
}
.alttab-element:hover { background-color: rgba(255, 255, 255, 0.08); }
.alttab-element.choisi {
  background-color: rgba(255, 194, 61, 0.18);
  border-color: #FFC23D;
}
.alttab-titre { color: #f2f4f8; font-size: 1.05em; font-weight: 600; }
"#;
