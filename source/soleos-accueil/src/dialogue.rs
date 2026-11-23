//! (v1.2) Petite fenêtre de choix au-dessus de tout (même style que « Renommer ») :
//! un titre, un texte, des boutons. Échap ferme sans rien faire.

use crate::tr::tr;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::rc::Rc;

/// Un bouton : texte, style (« principal », « secondaire », « danger ») et action
pub struct Choix {
    pub texte: String,
    pub style: &'static str,
    pub action: Box<dyn Fn()>,
}

pub fn choisir(app: &gtk::Application, titre: &str, texte: &str, choix: Vec<Choix>) {
    use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
    let f = gtk::Window::builder().application(app).decorated(false).title(titre).build();
    f.add_css_class("fenetre-bas");
    f.add_css_class("soleos");
    f.init_layer_shell();
    f.set_layer(Layer::Overlay);
    f.set_namespace(Some("soleos-choix"));
    f.set_keyboard_mode(KeyboardMode::Exclusive);
    let carte = gtk::Box::new(gtk::Orientation::Vertical, 12);
    carte.add_css_class("carte-renommer");
    let t = gtk::Label::new(Some(titre));
    t.add_css_class("titre-menu");
    t.set_xalign(0.0);
    carte.append(&t);
    let l = gtk::Label::new(Some(texte));
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_max_width_chars(46);
    carte.append(&l);
    let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    rangee.set_halign(gtk::Align::End);
    let annuler = gtk::Button::with_label(&tr("Annuler"));
    annuler.add_css_class("secondaire");
    rangee.append(&annuler);
    for c in choix {
        let b = gtk::Button::with_label(&c.texte);
        b.add_css_class(c.style);
        let action = Rc::new(c.action);
        let f2 = f.clone();
        b.connect_clicked(move |_| {
            f2.destroy();
            action();
        });
        rangee.append(&b);
    }
    carte.append(&rangee);
    f.set_child(Some(&carte));
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
}
