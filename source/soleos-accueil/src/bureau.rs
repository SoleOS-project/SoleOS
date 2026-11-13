//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS v12.5 – le bureau :
//! - à gauche, la barre magique : Bureautique, Mail, Internet, Fichiers, Applications, Réglages,
//!   avec leur nom (icônes seules sur les petits écrans) ; un clic ouvre toujours quelque chose de neuf ;
//! - en bas, la barre des fenêtres : l'éclair, un bouton par fenêtre ouverte (icône et titre,
//!   l'active soulignée en jaune ; un clic la passe devant ou la réduit), puis Wi-Fi, son, batterie
//!   et l'heure ;
//! - le panneau de l'éclair (ou touche ⊞) : recherche, mes applications, verrouiller, veille,
//!   redémarrer, éteindre.
//! Le centre de l'écran ne montre que le fond d'écran.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use super::{correspond, encoder, entree_menu, icone_fenetre, lancer, Accueil, Appli, APPLIS, MAIL};
use crate::applis;
use crate::fenetres::Fenetre;
use crate::icones;
use crate::securite;
use crate::tr::{self, tr, trv};

/// Hauteur de la barre du bas (assez haute pour se viser du premier coup, au doigt comme à la souris)
pub const HAUTEUR_BAS: i32 = 56;
/// Bord droit de la barre magique (pour que les menus de la barre du bas ne passent pas dessous)
const MARGE_MAGIQUE: i32 = 108;

/// Petit écran (1024 × 600…) : barre magique en icônes seules
pub fn petit_ecran() -> bool {
    gdk::Display::default()
        .and_then(|d| d.monitors().item(0))
        .and_downcast::<gdk::Monitor>()
        .map(|m| m.geometry().height() < 700)
        .unwrap_or(false)
}

/// Icône de la barre magique pour chaque appli principale
fn icone_magique(appli: &Appli) -> &'static str {
    match appli.nom {
        "Bureautique" => "bureautique",
        "Mail" => "mail",
        "Internet" => "internet",
        "Fichiers" => "fichiers",
        _ => "applications",
    }
}

fn lanceur(icone: &str, nom: &str, petit: bool) -> gtk::Button {
    let boite = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let image = icones::image(icone, if petit { 40 } else { 44 });
    boite.append(&image);
    if !petit {
        let n = gtk::Label::new(Some(nom));
        n.add_css_class("nom-lanceur");
        boite.append(&n);
    }
    let b = gtk::Button::builder().child(&boite).build();
    b.add_css_class("lanceur");
    b.set_tooltip_text(Some(nom));
    b
}

/// La barre magique (fenêtre « couche » à gauche, toujours visible, sans clavier)
pub fn barre_magique(app: &gtk::Application) -> (gtk::Window, Vec<(&'static Appli, gtk::Button)>, gtk::Button) {
    let petit = petit_ecran();
    let f = gtk::Window::builder().application(app).decorated(false).title("Barre magique").build();
    f.add_css_class("fenetre-magique");
    f.add_css_class("soleos");
    f.init_layer_shell();
    f.set_layer(Layer::Top);
    f.set_namespace(Some("soleos-magique"));
    f.set_anchor(Edge::Left, true);
    f.set_margin(Edge::Left, 8);
    f.auto_exclusive_zone_enable();
    f.set_keyboard_mode(KeyboardMode::None);
    let colonne = gtk::Box::new(gtk::Orientation::Vertical, if petit { 8 } else { 4 });
    colonne.add_css_class("magique");
    if petit {
        colonne.add_css_class("petit");
    }
    // v1.0.2 : petit bouton « – » pour réduire la barre à la main (« + » pour la remettre)
    let replier = gtk::Button::with_label("–");
    replier.add_css_class("replier-magique");
    replier.set_halign(gtk::Align::Center);
    replier.set_tooltip_text(Some(&tr("Réduire la barre")));
    colonne.append(&replier);
    let mut boutons = Vec::new();
    for appli in APPLIS {
        let b = lanceur(icone_magique(appli), &tr(appli.nom), petit);
        colonne.append(&b);
        boutons.push((appli, b));
    }
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.add_css_class("separateur-magique");
    colonne.append(&sep);
    let reglages = lanceur("reglages-tuile", &tr("Réglages"), petit);
    colonne.append(&reglages);
    colonne.append(&corbeille(petit));
    // SoleOS v1.0.1 : quand une fenêtre agrandie est devant, la barre recule (les fenêtres
    // prennent tout l'écran) ; une fine poignée invisible au bord gauche la fait revenir
    // par-dessus la fenêtre dès que la souris s'en approche.
    let racine = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let poignee = gtk::Box::new(gtk::Orientation::Vertical, 0);
    poignee.add_css_class("poignee-magique");
    poignee.set_size_request(4, 220);
    poignee.set_visible(false);
    racine.append(&poignee);
    racine.append(&colonne);
    f.set_child(Some(&racine));
    let barre = Rc::new(BarreMagique {
        fenetre: f.clone(),
        colonne: colonne.clone(),
        poignee: poignee.clone(),
        replier: replier.clone(),
        agrandie: std::cell::Cell::new(false),
        a_la_main: std::cell::Cell::new(reduite_a_la_main()),
        survol: std::cell::Cell::new(false),
        attente: std::cell::RefCell::new(None),
    });
    for w in [poignee.upcast_ref::<gtk::Widget>(), colonne.upcast_ref()] {
        let m = gtk::EventControllerMotion::new();
        let moi = Rc::downgrade(&barre);
        m.connect_enter(move |_, _, _| {
            if let Some(b) = moi.upgrade() {
                if let Some(id) = b.attente.borrow_mut().take() {
                    id.remove();
                }
                if !b.survol.get() {
                    b.survol.set(true);
                    b.mettre_a_jour();
                }
            }
        });
        let moi = Rc::downgrade(&barre);
        m.connect_leave(move |_| {
            if let Some(b) = moi.upgrade() {
                b.replier_plus_tard();
            }
        });
        w.add_controller(m);
    }
    {
        let moi = Rc::downgrade(&barre);
        replier.connect_clicked(move |_| {
            if let Some(b) = moi.upgrade() {
                let main = !b.a_la_main.get();
                b.a_la_main.set(main);
                garder_reduite_a_la_main(main);
                if let Some(id) = b.attente.borrow_mut().take() {
                    id.remove();
                }
                // Réduite : elle part tout de suite, même si la souris est encore dessus
                b.survol.set(false);
                b.mettre_a_jour();
            }
        });
    }
    // État de départ (réduite à la main) seulement une fois la barre affichée : certaines versions
    // de gtk4-layer-shell plantent si l'on change la place réservée d'une couche pas encore affichée
    if barre.a_la_main.get() {
        let moi = Rc::downgrade(&barre);
        f.connect_map(move |_| {
            if let Some(b) = moi.upgrade() {
                let b2 = b.clone();
                glib::idle_add_local_once(move || b2.mettre_a_jour());
            }
        });
    }
    MAGIQUE.with(|m| *m.borrow_mut() = Some(barre));
    (f, boutons, reglages)
}

thread_local! {
    static MAGIQUE: std::cell::RefCell<Option<Rc<BarreMagique>>> = const { std::cell::RefCell::new(None) };
}

/// La barre magique et son recul quand une fenêtre agrandie occupe l'écran
pub struct BarreMagique {
    fenetre: gtk::Window,
    colonne: gtk::Box,
    poignee: gtk::Box,
    replier: gtk::Button,
    agrandie: std::cell::Cell<bool>,
    /// Réduite à la main (bouton « – »), gardé d'une session à l'autre
    a_la_main: std::cell::Cell<bool>,
    survol: std::cell::Cell<bool>,
    attente: std::cell::RefCell<Option<glib::SourceId>>,
}

impl BarreMagique {
    fn mettre_a_jour(&self) {
        let cachee = self.agrandie.get() || self.a_la_main.get();
        let recule = cachee && !self.survol.get();
        if self.a_la_main.get() {
            self.replier.set_label("+");
            self.replier.set_tooltip_text(Some(&tr("Garder la barre affichée")));
        } else {
            self.replier.set_label("–");
            self.replier.set_tooltip_text(Some(&tr("Réduire la barre")));
        }
        self.colonne.set_visible(!recule);
        self.poignee.set_visible(recule);
        self.fenetre.set_margin(Edge::Left, if recule { 0 } else { 8 });
        // Repliée : la poignée occupe tout le bord gauche (au-dessus de la barre du bas)
        self.fenetre.set_anchor(Edge::Top, recule);
        self.fenetre.set_anchor(Edge::Bottom, recule);
        self.fenetre.set_margin(Edge::Bottom, if recule { HAUTEUR_BAS } else { 0 });
        // Fenêtre agrandie devant : la barre ne réserve plus de place (même dépliée au survol,
        // elle passe par-dessus) ; sinon les fenêtres s'arrêtent à côté d'elle
        if cachee {
            self.fenetre.set_exclusive_zone(0);
        } else {
            self.fenetre.auto_exclusive_zone_enable();
        }
    }

    /// Repliée un peu après le départ de la souris (sauf si un menu de la barre est ouvert)
    fn replier_plus_tard(self: &Rc<Self>) {
        if let Some(id) = self.attente.borrow_mut().take() {
            id.remove();
        }
        let moi = Rc::downgrade(self);
        let id = glib::timeout_add_local(std::time::Duration::from_millis(700), move || {
            let Some(b) = moi.upgrade() else { return glib::ControlFlow::Break };
            let dedans = b.colonne.observe_controllers().into_iter().flatten().any(|c| {
                c.downcast_ref::<gtk::EventControllerMotion>().is_some_and(|m| m.contains_pointer())
            });
            if dedans || crate::rail::bulle_ouverte(b.colonne.upcast_ref()) {
                return glib::ControlFlow::Continue;
            }
            b.attente.borrow_mut().take();
            b.survol.set(false);
            b.mettre_a_jour();
            glib::ControlFlow::Break
        });
        *self.attente.borrow_mut() = Some(id);
    }
}

fn fichier_reduite() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("soleos").join("barre-magique-reduite"))
}

fn reduite_a_la_main() -> bool {
    fichier_reduite().map(|f| f.exists()).unwrap_or(false)
}

fn garder_reduite_a_la_main(oui: bool) {
    if let Some(f) = fichier_reduite() {
        if oui {
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(f, "oui\n");
        } else {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// Une fenêtre agrandie (ou en plein écran) est-elle ouverte ? La barre magique recule ou revient.
pub fn magique_fenetres(agrandie: bool) {
    MAGIQUE.with(|m| {
        if let Some(b) = m.borrow().as_ref() {
            if b.agrandie.get() != agrandie {
                b.agrandie.set(agrandie);
                // Une fenêtre vient de s'agrandir (souvent ouverte d'un clic sur la barre, la souris
                // encore dessus) : la barre recule tout de suite
                b.survol.set(false);
                if let Some(id) = b.attente.borrow_mut().take() {
                    id.remove();
                }
                b.mettre_a_jour();
            }
        }
    });
}

/// Corbeille (v13.2), en bas de la barre magique : clic → ouverte dans Fichiers (« Restaurer ») ;
/// clic droit → Vider (avec confirmation) ; un fichier glissé dessus y part. Icône vide ou pleine.
fn corbeille(petit: bool) -> gtk::Button {
    let b = lanceur("corbeille", &tr("Corbeille"), petit);
    let image = b.child().and_then(|c| c.first_child()).and_downcast::<gtk::Image>();
    // Lecture et surveillance de la corbeille sans jamais bloquer l'écran (gvfs, en arrière-plan)
    let poubelle = gio::File::for_uri("trash:///");
    let maj: Rc<dyn Fn()> = {
        let (b, image, poubelle) = (b.clone(), image.clone(), poubelle.clone());
        Rc::new(move || {
            let (b, image) = (b.clone(), image.clone());
            poubelle.query_info_async("trash::item-count", gio::FileQueryInfoFlags::NONE, glib::Priority::LOW, gio::Cancellable::NONE, move |r| {
                let n = r.map(|i| i.attribute_uint32("trash::item-count")).unwrap_or(0);
                if let Some(i) = &image {
                    icones::changer(i, if n > 0 { "corbeille-pleine" } else { "corbeille" });
                }
                b.set_tooltip_text(Some(&if n == 0 {
                    tr("Corbeille (vide)")
                } else {
                    trv("Corbeille : {n} élément(s)", &[("n", &n.to_string())])
                }));
            });
        })
    };
    {
        let maj = maj.clone();
        // Quelques secondes après l'ouverture de session (gvfs prêt), puis à chaque changement
        glib::timeout_add_seconds_local_once(3, move || {
            maj();
            let poubelle = gio::File::for_uri("trash:///");
            if let Ok(m) = poubelle.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
                let maj = maj.clone();
                m.connect_changed(move |_, _, _, _| maj());
                // Surveillance gardée pour toute la vie de la barre
                std::mem::forget(m);
            }
        });
    }
    b.connect_clicked(|_| lancer(&["nautilus", "--new-window", "trash:///"]));
    // Clic droit : Ouvrir, Vider la corbeille (confirmation)
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let bulle = gtk::Popover::new();
    bulle.set_child(Some(&menu));
    bulle.set_parent(&b);
    bulle.set_position(gtk::PositionType::Right);
    menu.append(&entree_menu(&tr("Ouvrir"), &bulle, || lancer(&["nautilus", "--new-window", "trash:///"])));
    menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let confirmer = gtk::Popover::new();
    confirmer.set_position(gtk::PositionType::Right);
    let boite = gtk::Box::new(gtk::Orientation::Vertical, 10);
    boite.set_margin_top(4);
    boite.set_margin_bottom(4);
    boite.set_margin_start(4);
    boite.set_margin_end(4);
    let question = gtk::Label::new(Some(&tr("Vider la corbeille ? Les éléments seront effacés définitivement.")));
    question.set_wrap(true);
    question.set_max_width_chars(30);
    boite.append(&question);
    let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    rangee.set_halign(gtk::Align::End);
    let annuler = gtk::Button::with_label(&tr("Annuler"));
    annuler.add_css_class("secondaire");
    annuler.add_css_class("petit");
    let vider = gtk::Button::with_label(&tr("Vider"));
    vider.add_css_class("danger");
    vider.add_css_class("petit");
    rangee.append(&annuler);
    rangee.append(&vider);
    boite.append(&rangee);
    confirmer.set_child(Some(&boite));
    confirmer.set_parent(&b);
    {
        let c = confirmer.clone();
        annuler.connect_clicked(move |_| c.popdown());
        let c = confirmer.clone();
        vider.connect_clicked(move |_| {
            c.popdown();
            lancer(&["gio", "trash", "--empty"]);
        });
    }
    let c = confirmer.clone();
    let entree_vider = entree_menu(&tr("Vider la corbeille"), &bulle, move || c.popup());
    entree_vider.add_css_class("danger");
    menu.append(&entree_vider);
    super::attacher_menu_contextuel(&b, &bulle);
    // Glisser un fichier sur la corbeille
    let depot = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::MOVE | gdk::DragAction::COPY);
    depot.connect_drop(|_, valeur, _, _| {
        let Ok(liste) = valeur.get::<gdk::FileList>() else { return false };
        for f in liste.files() {
            let _ = f.trash(gio::Cancellable::NONE);
        }
        true
    });
    b.add_controller(depot);
    b
}

/// La barre du bas : éléments à remplir par l'accueil
pub struct BarreBas {
    pub fenetre: gtk::Window,
    pub eclair: gtk::Button,
    pub taches: gtk::Box,
    pub droite: gtk::Box,
    pub heure: gtk::Label,
    pub date: gtk::Label,
}

pub fn barre_bas(app: &gtk::Application) -> BarreBas {
    let f = gtk::Window::builder().application(app).decorated(false).title("Barre des fenêtres").build();
    f.add_css_class("fenetre-bas");
    f.add_css_class("soleos");
    f.init_layer_shell();
    f.set_layer(Layer::Top);
    f.set_namespace(Some("soleos-barre"));
    for bord in [Edge::Left, Edge::Right, Edge::Bottom] {
        f.set_anchor(bord, true);
    }
    f.auto_exclusive_zone_enable();
    // Clavier à la demande : mot de passe du Wi-Fi tapé dans la bulle
    f.set_keyboard_mode(KeyboardMode::OnDemand);
    let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    ligne.add_css_class("barre-bas");
    ligne.set_size_request(-1, HAUTEUR_BAS);
    let eclair = gtk::Button::builder().child(&icones::image("eclair-bouton", 38)).build();
    eclair.add_css_class("eclair");
    eclair.set_tooltip_text(Some(&tr("Rechercher, applications, éteindre (touche Windows)")));
    eclair.set_valign(gtk::Align::Center);
    ligne.append(&eclair);
    let taches = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    taches.add_css_class("taches");
    taches.set_margin_start(6);
    // Beaucoup de fenêtres : les boutons rétrécissent ; au pire on fait défiler, sans jamais
    // pousser le Wi-Fi, le son et l'heure hors de l'écran
    let defil = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .child(&taches)
        .build();
    defil.add_css_class("defil-taches");
    ligne.append(&defil);
    let droite = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    droite.set_valign(gtk::Align::Center);
    ligne.append(&droite);
    let horloge = gtk::Box::new(gtk::Orientation::Vertical, 0);
    horloge.add_css_class("horloge-bas");
    horloge.set_valign(gtk::Align::Center);
    let heure = gtk::Label::new(None);
    heure.add_css_class("heure-bas");
    heure.set_xalign(1.0);
    let date = gtk::Label::new(None);
    date.add_css_class("date-bas");
    date.set_xalign(1.0);
    horloge.append(&heure);
    horloge.append(&date);
    // v13.1 : un clic sur l'heure ouvre le calendrier du mois (comme Windows)
    let bouton_horloge = gtk::Button::builder().child(&horloge).build();
    bouton_horloge.add_css_class("etat-bas");
    bouton_horloge.add_css_class("bouton-horloge");
    bouton_horloge.set_valign(gtk::Align::Center);
    let calendrier = gtk::Calendar::new();
    calendrier.add_css_class("calendrier-bas");
    let bulle_cal = gtk::Popover::new();
    bulle_cal.add_css_class("bulle-bas");
    bulle_cal.set_child(Some(&calendrier));
    bulle_cal.set_parent(&bouton_horloge);
    bulle_cal.set_position(gtk::PositionType::Top);
    {
        let c = calendrier.clone();
        bulle_cal.connect_show(move |b| {
            apparition(b);
            if let Ok(m) = glib::DateTime::now_local() {
                c.select_day(&m);
            }
        });
        let bl = bulle_cal.clone();
        bouton_horloge.connect_clicked(move |_| bl.popup());
    }
    ligne.append(&bouton_horloge);
    let maj = {
        let (h, d) = (heure.clone(), date.clone());
        move || {
            let maintenant = glib::DateTime::now_local().ok();
            if let Some(m) = maintenant {
                h.set_label(&m.format("%H:%M").map(|s| s.to_string()).unwrap_or_default());
                // « lun. 2 nov. » : jour et mois abrégés, dans la langue de l'ordinateur
                d.set_label(&m.format(&tr("%a %-e %b")).map(|s| s.to_string()).unwrap_or_default());
            }
        }
    };
    maj();
    glib::timeout_add_seconds_local(2, move || {
        maj();
        glib::ControlFlow::Continue
    });
    f.set_child(Some(&ligne));
    BarreBas { fenetre: f, eclair, taches, droite, heure, date }
}

/// Apparition en fondu (v13.1, 140 ms) : léger, rien ne tourne une fois l'animation finie
pub fn apparition(w: &impl IsA<gtk::Widget>) {
    let w = w.as_ref().clone();
    w.set_opacity(0.0);
    let debut = std::cell::Cell::new(None::<i64>);
    w.add_tick_callback(move |w, horloge| {
        let t = horloge.frame_time();
        let d = *debut.get().get_or_insert(t);
        debut.set(Some(d));
        let x = ((t - d) as f64 / 140_000.0).min(1.0);
        // Départ rapide, arrivée douce
        w.set_opacity(1.0 - (1.0 - x).powi(3));
        if x >= 1.0 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
}

/// Bouton d'état de la barre du bas (Wi-Fi, son, batterie)
pub fn bouton_bas(icone: &str, info: &str) -> (gtk::Button, gtk::Image) {
    let nom = match icone {
        "son" => "r-son-bas".to_string(),
        "wifi-off" => "r-wifi-off-bas".to_string(),
        autre => format!("r-{autre}"),
    };
    let image = icones::image(&nom, 26);
    let b = gtk::Button::builder().child(&image).tooltip_text(info).build();
    b.add_css_class("etat-bas");
    (b, image)
}

/// Petite bulle d'information en bas de l'écran (2,5 s), sans prendre le clavier
pub fn bulle_info(app: &gtk::Application, texte: &str) {
    let f = gtk::Window::builder().application(app).decorated(false).title("Info").build();
    f.add_css_class("fenetre-bas");
    f.init_layer_shell();
    f.set_layer(Layer::Overlay);
    f.set_namespace(Some("soleos-info"));
    f.set_anchor(Edge::Bottom, true);
    f.set_margin(Edge::Bottom, HAUTEUR_BAS + 18);
    f.set_exclusive_zone(-1);
    f.set_keyboard_mode(KeyboardMode::None);
    let l = gtk::Label::new(Some(texte));
    l.add_css_class("bulle-info");
    f.set_child(Some(&l));
    f.present();
    glib::timeout_add_seconds_local_once(3, move || f.destroy());
}

/// Le panneau de l'éclair
pub struct PanneauEclair {
    pub fenetre: gtk::Window,
    pub recherche: gtk::Entry,
    pub grille: gtk::FlowBox,
    pub vide: gtk::Label,
    pub web: gtk::Button,
    pub texte_web: gtk::Label,
    pub verrouiller: gtk::Button,
}

fn bouton_panneau(icone: &str, texte: &str, classe: &str) -> gtk::Button {
    let boite = gtk::Box::new(gtk::Orientation::Vertical, 4);
    boite.append(&icones::image(icone, 20));
    let l = gtk::Label::new(Some(texte));
    boite.append(&l);
    let b = gtk::Button::builder().child(&boite).build();
    b.add_css_class("bouton-panneau");
    if !classe.is_empty() {
        b.add_css_class(classe);
    }
    b.set_hexpand(true);
    b
}

pub fn panneau_eclair(app: &gtk::Application) -> Rc<PanneauEclair> {
    let f = gtk::Window::builder().application(app).decorated(false).title(tr("Rechercher")).build();
    f.add_css_class("voile-eclair");
    f.add_css_class("soleos");
    f.init_layer_shell();
    f.set_layer(Layer::Overlay);
    f.set_namespace(Some("soleos-eclair"));
    for bord in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        f.set_anchor(bord, true);
    }
    f.set_exclusive_zone(-1);
    f.set_keyboard_mode(KeyboardMode::Exclusive);
    let carte = gtk::Box::new(gtk::Orientation::Vertical, 0);
    carte.add_css_class("panneau-eclair");
    carte.set_halign(gtk::Align::Start);
    carte.set_valign(gtk::Align::End);
    carte.set_margin_start(10);
    carte.set_margin_bottom(HAUTEUR_BAS + 10);
    carte.set_size_request(560, -1);
    let recherche = gtk::Entry::builder().placeholder_text(tr("Rechercher une application ou sur le web…")).build();
    recherche.add_css_class("recherche-eclair");
    carte.append(&recherche);
    let titre = gtk::Label::new(Some(&tr("Mes applications")));
    titre.add_css_class("titre-panneau-eclair");
    titre.set_xalign(0.0);
    carte.append(&titre);
    let grille = gtk::FlowBox::new();
    grille.set_selection_mode(gtk::SelectionMode::None);
    grille.set_homogeneous(true);
    grille.set_min_children_per_line(5);
    grille.set_max_children_per_line(5);
    grille.set_row_spacing(6);
    grille.set_column_spacing(6);
    let defil = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(300)
        .child(&grille)
        .build();
    carte.append(&defil);
    let vide = gtk::Label::new(Some(&tr("Aucune application ajoutée pour l'instant. Touchez « Ajouter » pour en choisir.")));
    vide.add_css_class("discret");
    vide.set_wrap(true);
    vide.set_xalign(0.0);
    carte.append(&vide);
    // « Rechercher … sur le web » : toujours proposé quand on a tapé quelque chose
    let ligne_web = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    ligne_web.append(&icones::image("loupe", 16));
    let texte_web = gtk::Label::new(None);
    texte_web.set_xalign(0.0);
    texte_web.set_ellipsize(gtk::pango::EllipsizeMode::End);
    ligne_web.append(&texte_web);
    let web = gtk::Button::builder().child(&ligne_web).build();
    web.add_css_class("web-eclair");
    web.set_visible(false);
    carte.append(&web);
    let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    boutons.add_css_class("boutons-panneau");
    boutons.set_homogeneous(true);
    let verrouiller = bouton_panneau("cadenas", &tr("Verrouiller"), "");
    let veille = bouton_panneau("veille", &tr("Veille"), "");
    let redemarrer = bouton_panneau("redemarrer", &tr("Redémarrer"), "");
    let eteindre = bouton_panneau("alimentation", &tr("Éteindre"), "rouge");
    for b in [&verrouiller, &veille, &redemarrer, &eteindre] {
        boutons.append(b);
    }
    carte.append(&boutons);
    // Capture d'écran (sans la touche Impr. écran) et version de SoleOS, en bas du panneau
    let bas_panneau = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bas_panneau.set_margin_top(12);
    let capture = gtk::Button::with_label(&tr("Capture d'écran"));
    capture.add_css_class("lien-son");
    {
        let f = f.clone();
        capture.connect_clicked(move |_| {
            f.set_visible(false);
            // Le panneau a le temps de disparaître avant la photo
            glib::timeout_add_local_once(std::time::Duration::from_millis(700), || lancer(&["/usr/local/bin/soleos-capture"]));
        });
    }
    bas_panneau.append(&capture);
    let signaler = gtk::Button::with_label(&tr("Signaler un problème"));
    signaler.add_css_class("lien-son");
    {
        let f = f.clone();
        signaler.connect_clicked(move |_| {
            f.set_visible(false);
            super::demander("signaler");
        });
    }
    bas_panneau.append(&signaler);
    if let Ok(v) = std::fs::read_to_string("/etc/soleos-version") {
        let version = gtk::Label::new(Some(&format!("SoleOS {}", v.trim())));
        version.add_css_class("version-eclair");
        version.set_hexpand(true);
        version.set_xalign(1.0);
        bas_panneau.append(&version);
    }
    carte.append(&bas_panneau);
    f.set_child(Some(&carte));
    let p = Rc::new(PanneauEclair { fenetre: f.clone(), recherche: recherche.clone(), grille, vide, web: web.clone(), texte_web, verrouiller: verrouiller.clone() });
    // Fermer : Échap, clic à côté
    let touches = gtk::EventControllerKey::new();
    let fk = f.clone();
    touches.connect_key_pressed(move |_, touche, _, _| {
        if touche == gdk::Key::Escape {
            fk.set_visible(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    touches.set_propagation_phase(gtk::PropagationPhase::Capture);
    f.add_controller(touches);
    let clic = gtk::GestureClick::new();
    let (fc, cc) = (f.clone(), carte.clone());
    clic.connect_pressed(move |_, _, x, y| {
        let dedans = cc.compute_bounds(&fc).map(|r| r.contains_point(&gtk::graphene::Point::new(x as f32, y as f32))).unwrap_or(true);
        if !dedans {
            fc.set_visible(false);
        }
    });
    f.add_controller(clic);
    let fermer = {
        let f = f.clone();
        move || f.set_visible(false)
    };
    {
        let fe = fermer.clone();
        verrouiller.connect_clicked(move |_| {
            fe();
            securite::verrouiller();
        });
    }
    {
        let fe = fermer.clone();
        veille.connect_clicked(move |_| {
            fe();
            lancer(&["systemctl", "suspend"]);
        });
    }
    {
        let fe = fermer.clone();
        redemarrer.connect_clicked(move |_| {
            fe();
            lancer(&["systemctl", "reboot"]);
        });
    }
    eteindre.connect_clicked(move |_| {
        fermer();
        lancer(&["systemctl", "poweroff"]);
    });
    p
}

impl Accueil {
    /// Clic sur la barre magique : toujours quelque chose de neuf (nouveau document, nouvelle
    /// fenêtre). Seules les applis à une seule fenêtre (Thunderbird, Applications) reviennent devant.
    pub(crate) fn lancer_magique(self: &Rc<Self>, appli: &'static Appli, bouton: &gtk::Button) {
        self.fermer_eclair();
        match appli.nom {
            "Bureautique" if self.choix_bureautique.parent().is_some() => {
                if self.choix_bureautique.parent().as_ref() != Some(bouton.upcast_ref()) {
                    self.choix_bureautique.unparent();
                    self.choix_bureautique.set_parent(bouton);
                }
                self.choix_bureautique.set_position(gtk::PositionType::Right);
                self.choix_bureautique.popup();
            }
            "Mail" | "Applications" => {
                if let Some(g) = &self.gestionnaire {
                    if let Some(f) = g.liste().iter().find(|f| correspond(appli, f)) {
                        g.activer(f.id);
                        return;
                    }
                }
                if appli.nom == "Mail" {
                    lancer(MAIL);
                } else {
                    lancer(appli.commande);
                }
            }
            "Internet" => lancer(&["soleos-web", "--new-window", tr::courante().web]),
            _ => lancer(appli.commande),
        }
    }

    /// Panneau de l'éclair : ouvre ou ferme (bouton éclair, touche ⊞)
    pub(crate) fn basculer_eclair(self: &Rc<Self>) {
        let Some(p) = &self.panneau_eclair else { return };
        if p.fenetre.is_visible() {
            p.fenetre.set_visible(false);
            return;
        }
        self.choix_bureautique.popdown();
        p.recherche.set_text("");
        self.remplir_eclair("");
        p.verrouiller.set_sensitive(std::path::Path::new("/run/soleos/verrouillable").exists());
        p.fenetre.present();
        apparition(&p.fenetre);
        p.recherche.grab_focus();
    }

    pub(crate) fn fermer_eclair(&self) {
        if let Some(p) = &self.panneau_eclair {
            p.fenetre.set_visible(false);
        }
    }

    /// « Mes applications » du panneau, filtrées par la recherche
    pub(crate) fn remplir_eclair(self: &Rc<Self>, filtre: &str) {
        let Some(p) = &self.panneau_eclair else { return };
        while let Some(e) = p.grille.first_child() {
            p.grille.remove(&e);
        }
        let f = filtre.trim().to_lowercase();
        let installees: Vec<applis::Installee> =
            applis::installees().into_iter().filter(|a| f.is_empty() || a.nom.to_lowercase().contains(&f)).collect();
        for a in &installees {
            let image = gtk::Image::new();
            image.set_pixel_size(48);
            match a.icone.as_ref().and_then(|c| gdk::Texture::from_filename(c).ok()) {
                Some(t) => image.set_paintable(Some(&t)),
                None => icones::changer(&image, "appli"),
            }
            let nom = gtk::Label::new(Some(&a.nom));
            nom.set_max_width_chars(11);
            nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let boite = gtk::Box::new(gtk::Orientation::Vertical, 6);
            boite.append(&image);
            boite.append(&nom);
            let b = gtk::Button::builder().child(&boite).build();
            b.add_css_class("appli-eclair");
            b.set_tooltip_text(Some(&a.nom));
            let (moi, a2) = (Rc::downgrade(self), a.clone());
            b.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.fermer_eclair();
                    m.ouvrir_installee(&a2);
                }
            });
            // Clic droit (ou appui long) : « Ajouter au bureau » (v13.1)
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let bulle = gtk::Popover::new();
            bulle.set_child(Some(&menu));
            bulle.set_parent(&b);
            {
                let (a3, fen) = (a.clone(), p.fenetre.clone());
                menu.append(&entree_menu(&tr("Ajouter au bureau"), &bulle, move || {
                    let ok = crate::raccourcis::ajouter_appli(&a3.id, &a3.nom, a3.icone.as_deref(), &a3.commande()).is_ok();
                    if let Some(app) = fen.application() {
                        bulle_info(&app, &if ok { trv("« {nom} » est sur le bureau", &[("nom", &a3.nom)]) } else { tr("Impossible d'ajouter le raccourci") });
                    }
                }));
            }
            super::attacher_menu_contextuel(&b, &bulle);
            p.grille.insert(&b, -1);
        }
        // La Console (avancé) : toujours dans « Mes applications », et trouvée en tapant
        // « console » ou « terminal » (aussi : Ctrl + Alt + T)
        let nom_console = tr("Console");
        let console = f.is_empty()
            || (f.chars().count() >= 3 && (nom_console.to_lowercase().starts_with(&f) || "console".starts_with(&f) || "terminal".starts_with(&f)));
        if console {
            let boite = gtk::Box::new(gtk::Orientation::Vertical, 6);
            boite.append(&icones::image("tuile-console", 48));
            boite.append(&gtk::Label::new(Some(&nom_console)));
            let b = gtk::Button::builder().child(&boite).build();
            b.add_css_class("appli-eclair");
            b.set_tooltip_text(Some(&tr("Console (avancé) — Ctrl + Alt + T")));
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.fermer_eclair();
                    lancer(super::CONSOLE.commande);
                }
            });
            p.grille.insert(&b, -1);
        }
        let vide = installees.is_empty() && !console;
        p.vide.set_visible(installees.is_empty() && f.is_empty());
        if let Some(defil) = p.grille.ancestor(gtk::ScrolledWindow::static_type()) {
            defil.set_visible(!vide);
        }
        if let Some(titre) = p.recherche.next_sibling() {
            titre.set_visible(!vide || f.is_empty());
        }
        p.web.set_visible(!f.is_empty());
        if !f.is_empty() {
            p.texte_web.set_label(&(trv("Rechercher « {q} » sur le web", &[("q", filtre.trim())])));
        }
    }

    /// Entrée dans la recherche : la première application trouvée, sinon une recherche sur le web
    pub(crate) fn valider_eclair(self: &Rc<Self>, texte: &str) {
        let q = texte.trim();
        if q.is_empty() {
            return;
        }
        let ql = q.to_lowercase();
        if let Some(a) = applis::installees().into_iter().find(|a| a.nom.to_lowercase().contains(&ql)) {
            self.fermer_eclair();
            self.ouvrir_installee(&a);
            return;
        }
        if ql.chars().count() >= 3 && (tr("Console").to_lowercase().starts_with(&ql) || "console".starts_with(&ql) || "terminal".starts_with(&ql)) {
            self.fermer_eclair();
            lancer(super::CONSOLE.commande);
            return;
        }
        // Les applis principales aussi (« mail », « fichiers »…)
        if let Some(appli) = APPLIS.iter().find(|a| tr(a.nom).to_lowercase().starts_with(&ql)) {
            if let Some((_, b)) = self.boutons_magique.iter().find(|(a, _)| a.nom == appli.nom) {
                self.lancer_magique(appli, b);
            }
            return;
        }
        self.chercher_web(q);
    }

    pub(crate) fn chercher_web(&self, q: &str) {
        self.fermer_eclair();
        let adresse = format!("{}{}", tr::courante().recherche, encoder(q.trim()));
        lancer(&["soleos-web", "--new-window", &adresse]);
    }

    /// Barre du bas : un bouton par fenêtre ouverte, dans l'ordre d'ouverture
    pub(crate) fn maj_taches(self: &Rc<Self>, liste: &[Fenetre]) {
        let Some(t) = &self.taches else { return };
        while let Some(e) = t.first_child() {
            // Le menu (bulle) du bouton part avec lui
            let mut enfant = e.first_child();
            while let Some(c) = enfant {
                enfant = c.next_sibling();
                if c.is::<gtk::Popover>() {
                    c.unparent();
                }
            }
            t.remove(&e);
        }
        let installees = applis::installees();
        let mut fenetres: Vec<&Fenetre> = liste.iter().collect();
        fenetres.sort_by_key(|f| f.id);
        // v13.1 : icônes seules, en couleur (comme la barre magique) ; le nom apparaît au survol
        for f in fenetres {
            let titre = super::titre_lisible(f);
            let ligne = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let image = icone_fenetre(f, &installees);
            image.set_pixel_size(32);
            image.set_valign(gtk::Align::Center);
            image.set_vexpand(true);
            ligne.append(&image);
            let b = gtk::Button::builder().child(&ligne).build();
            b.add_css_class("tache");
            if f.active && !f.reduite {
                b.add_css_class("active");
            }
            if f.reduite {
                b.add_css_class("reduite");
            }
            b.set_tooltip_text(Some(&titre));
            b.set_valign(gtk::Align::Center);
            let (moi, id) = (Rc::downgrade(self), f.id);
            b.connect_clicked(move |_| {
                let Some(m) = moi.upgrade() else { return };
                let Some(g) = m.gestionnaire.clone() else { return };
                // Comme Windows : la fenêtre déjà devant se réduit, sinon elle passe devant
                let devant = g.liste().iter().any(|x| x.id == id && x.active && !x.reduite);
                if devant {
                    g.minimiser(id);
                } else {
                    g.activer(id);
                }
            });
            // Clic droit (ou appui long) : moitié gauche / droite, tout l'écran, fermer
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let bulle = gtk::Popover::new();
            bulle.set_child(Some(&menu));
            bulle.set_parent(&b);
            bulle.set_position(gtk::PositionType::Top);
            for (texte, touche) in [
                (format!("◧  {}", tr("À gauche")), "Left"),
                (format!("◨  {}", tr("À droite")), "Right"),
                (format!("▣  {}", tr("Tout l'écran")), "Up"),
            ] {
                let moi = Rc::downgrade(self);
                menu.append(&entree_menu(&texte, &bulle, move || {
                    if let Some(m) = moi.upgrade() {
                        m.placer(id, touche);
                    }
                }));
            }
            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let moi = Rc::downgrade(self);
            let fermer = entree_menu(&format!("✕  {}", tr("Fermer")), &bulle, move || {
                if let Some(g) = moi.upgrade().and_then(|m| m.gestionnaire.clone()) {
                    g.fermer(id);
                }
            });
            fermer.add_css_class("danger");
            menu.append(&fermer);
            super::attacher_menu_contextuel(&b, &bulle);
            // Les premiers boutons sont sous la barre magique : leur menu s'ouvre un peu à droite
            // pour ne pas passer dessous
            {
                let bl = bulle.clone();
                bulle.connect_show(move |p| {
                    let x = p
                        .parent()
                        .and_then(|b| b.compute_point(&b.root().unwrap(), &gtk::graphene::Point::new(b.width() as f32 / 2.0, 0.0)))
                        .map(|pt| pt.x() as i32)
                        .unwrap_or(400);
                    let decalage = (MARGE_MAGIQUE + 95 - x).max(0);
                    bl.set_offset(decalage, 0);
                });
            }
            t.append(&b);
        }
    }
}

pub const STYLE: &str = r#"
window.fenetre-magique, window.fenetre-bas { background: transparent; }
button.replier-magique { min-height: 0; min-width: 0; padding: 0 14px; margin: 0 0 2px 0; border-radius: 8px;
  background: transparent; color: alpha(white, 0.55); font-weight: 700; font-size: 15px; line-height: 1; border: none; box-shadow: none; }
button.replier-magique:hover { background: alpha(white, 0.10); color: white; }
.poignee-magique { background: rgba(140, 150, 170, 0.35); border-radius: 0 3px 3px 0; min-width: 4px; }
.magique { background-color: alpha(#0E1426, 0.88); border: 1px solid alpha(white, 0.08); border-radius: 22px;
  padding: 10px 6px; min-width: 76px; box-shadow: 0 10px 30px alpha(black, 0.35); }
.magique.petit { min-width: 52px; }
button.lanceur { background: none; border: none; box-shadow: none; border-radius: 14px; padding: 6px 4px; }
button.lanceur:hover { background-color: alpha(#FFC23D, 0.14); }
button.lanceur:active { background-color: alpha(#FFC23D, 0.26); }
.nom-lanceur { font-size: 11.5px; font-weight: 500; color: #E9ECF5; }
.separateur-magique { background-color: alpha(white, 0.12); min-height: 1px; margin: 4px 14px; }
.barre-bas { background-color: alpha(#0B1020, 0.95); border-top: 1px solid alpha(white, 0.07); padding: 0 10px; }
button.eclair { background: none; border: none; box-shadow: none; border-radius: 13px; padding: 3px; min-width: 44px; min-height: 44px; }
button.eclair:hover { background-color: alpha(#FFC23D, 0.14); }
button.eclair.ouvert { background-color: alpha(#FFC23D, 0.22); }
button.tache { background-color: transparent; border: none; box-shadow: none; border-radius: 12px;
  padding: 0 7px; min-height: 46px; min-width: 46px; border-bottom: 3px solid transparent; transition: background-color 150ms ease-out; }
scrolledwindow.defil-taches, scrolledwindow.defil-taches > viewport { background: none; }
button.tache:hover { background-color: alpha(white, 0.10); }
button.tache label { color: #DFE3EE; font-size: 13.5px; }
button.tache.active { background-color: alpha(#FFC23D, 0.13); border-bottom-color: #FFC23D; }
button.tache:not(.active):not(.reduite) { border-bottom-color: alpha(white, 0.28); }
button.tache.reduite image { opacity: 0.55; }
button.etat-bas { background: none; border: none; box-shadow: none; border-radius: 11px; min-width: 40px; min-height: 40px; padding: 0; }
button.etat-bas:hover { background-color: alpha(white, 0.08); }
.horloge-bas { margin: 0 4px; }
.groupe-etat { margin-right: 2px; }
button.tuile-batterie { background-color: alpha(white, 0.06); border-radius: 14px; padding: 0 10px 0 6px; min-height: 40px; margin: 0 2px 0 4px; }
button.tuile-batterie label { color: #E9ECF5; font-weight: 600; font-size: 13.5px; }
button.bouton-horloge { min-width: 0; padding: 0 6px; }
calendar.calendrier-bas { background: none; border: none; color: #E9ECF5; padding: 4px; font-size: 13.5px; }
calendar.calendrier-bas > grid > label.today { background-color: #FFC23D; color: #1A1D27; border-radius: 9px; font-weight: 700; box-shadow: none; }
calendar.calendrier-bas > grid > label:selected { background-color: alpha(#FFC23D, 0.25); color: #FFFFFF; border-radius: 9px; }
calendar.calendrier-bas > grid > label.other-month { color: alpha(#E9ECF5, 0.3); }
calendar.calendrier-bas > grid > label.day-name { color: #8B93A8; font-size: 11.5px; }
calendar.calendrier-bas > header { border: none; background: none; }
calendar.calendrier-bas > header > button { background: none; border: none; box-shadow: none; color: #E9ECF5; }
.heure-bas { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 17px; font-weight: 600; color: #F2F4F8; }
.date-bas { font-size: 11.5px; color: #A7B0C8; }
.barre-bas .maj-prete { margin-right: 8px; }
.barre-bas .texte-etat { color: #DFE3EE; }
window.voile-eclair { background: transparent; }
.panneau-eclair { background-color: alpha(#0E1426, 0.97); border: 1px solid alpha(white, 0.10); border-radius: 22px;
  padding: 22px 24px; box-shadow: 0 20px 60px alpha(black, 0.55); }
entry.recherche-eclair { min-height: 46px; border-radius: 14px; border: 1.5px solid #FFC23D; background-color: #141A2E;
  color: #F2F4F8; font-size: 15px; padding: 0 14px; box-shadow: none; }
.titre-panneau-eclair { font-size: 12px; letter-spacing: 1px; color: #8B93A8; margin: 18px 0 10px 0; }
button.appli-eclair { background: none; border: none; box-shadow: none; border-radius: 12px; padding: 8px 2px; }
button.appli-eclair:hover { background-color: alpha(white, 0.08); }
button.appli-eclair label { color: #DFE3EE; font-size: 12px; }
.boutons-panneau { margin-top: 18px; }
button.bouton-panneau { background-color: alpha(white, 0.06); border: none; box-shadow: none; border-radius: 14px; min-height: 54px; }
button.bouton-panneau:hover { background-color: alpha(white, 0.12); }
button.bouton-panneau label { color: #DFE3EE; font-size: 12px; }
button.bouton-panneau, button.web-eclair, button.tache, button.lanceur, button.eclair, button.etat-bas, button.appli-eclair { background-image: none; }
.cadre-signaler { border: 1px solid alpha(white, 0.15); border-radius: 12px; }
.champ-signaler, .champ-signaler text { background-color: #141A2E; color: #F2F4F8; font-size: 14px; }
.bulle-info { background-color: alpha(#0E1426, 0.96); color: #F2F4F8; border: 1px solid #FFC23D; border-radius: 14px; padding: 12px 20px; font-size: 14px; }
.version-bureau { font-size: 12px; color: alpha(#E9ECF5, 0.55); }
.version-eclair { font-size: 11.5px; color: #8B93A8; }
button.lien-son, .feuille button.lien-son, popover button.lien-son, .panneau-eclair button.lien-son { background: none; border: none; box-shadow: none; padding: 2px 0; color: #FFC23D; font-size: 13px; min-height: 0; }
button.lien-son:hover { text-decoration: underline; }
.texte-essai-son { font-size: 15px; }
button.web-eclair { background-color: alpha(white, 0.05); border: none; box-shadow: none; border-radius: 12px; min-height: 42px;
  margin-top: 10px; padding: 0 14px; }
button.web-eclair:hover { background-color: alpha(#FFC23D, 0.14); }
button.web-eclair label { color: #DFE3EE; font-size: 13.5px; }
button.bouton-panneau.rouge { background-color: alpha(#E24A4A, 0.22); }
button.bouton-panneau.rouge:hover { background-color: alpha(#E24A4A, 0.34); }
button.bouton-panneau:disabled { opacity: 0.4; }
"#;
