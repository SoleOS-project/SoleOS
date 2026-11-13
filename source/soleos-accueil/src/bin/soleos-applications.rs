//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – « Applications » : mes applications, la boutique (sélection SoleOS + toute la
//! logithèque Flathub), le choix de la messagerie et l'ajout d'un fichier AppImage.
//!
//!   soleos-applications                     mes applications
//!   soleos-applications --ajouter           directement la boutique
//!   soleos-applications --mail              choisir sa messagerie (tuile « Mail »)
//!   soleos-applications --ajouter-appimage FICHIER

#[path = "../applis.rs"]
mod applis;
#[path = "../icones.rs"]
mod icones;
#[path = "../theme.rs"]
mod theme;
#[path = "../tr.rs"]
mod tr;
use tr::{tr, trv};

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use applis::{Installee, Proposee};

const STYLE: &str = r#"
/* Applications : thème clair (comme les autres applications), accent jaune Éclair */
window.applications { background-color: #F4F6FA; background-image: none; color: #1B2033; }
.applications label { color: #1B2033; }
.titre { font-size: 30px; font-family: "Space Grotesk", "Inter", sans-serif; font-weight: 700; }
.sous-titre { font-size: 16px; color: #4A5368; }
.discret { font-size: 13px; color: #5B6478; }
.applications .discret { color: #5B6478; }
.categorie { font-size: 15px; font-weight: 600; color: #3B4256; margin-top: 18px; }
.onglets button { padding: 8px 18px; border-radius: 999px; background: none; border: none; box-shadow: none; color: #3B4256; font-size: 15px; }
.onglets button:checked { background: #FFC23D; color: #0B2A5B; }
.onglets button:checked label { color: #0B2A5B; }
.onglets button:hover { background: #E6EAF2; }
button.tuile-appli { background: none; border: none; box-shadow: none; border-radius: 20px; padding: 12px; }
button.tuile-appli:hover { background: #E6EAF2; }
.nom-tuile { font-size: 14px; }
.applications .carte { padding: 14px 16px; background: #FFFFFF; border: 1px solid #DCE2EC; border-radius: 18px; }
.applications .carte:hover { background: #FAFBFD; }
.nom-carte { font-size: 15px; font-weight: 600; }
button.principal, button.secondaire { border-radius: 10px; padding: 6px 16px; }
.applications button.secondaire { background: #E8ECF3; color: #1B2033; border: 1px solid #D5DBE6; }
.applications button.secondaire label { color: #1B2033; }
button.fermer { background: #E8ECF3; color: #1B2033; border-radius: 999px; padding: 8px 18px; border: none; box-shadow: none; }
button.fermer:hover { background: #DDE3EC; }
button.danger { background: #D93025; color: white; border-radius: 10px; padding: 6px 14px; border: none; box-shadow: none; }
entry.recherche, .applications entry { background: #FFFFFF; color: #1B2033; border: 1px solid #CFD6E2; border-radius: 14px; padding: 8px 18px; box-shadow: none; min-height: 28px; font-size: 15px; caret-color: #2563EB; }
entry.recherche:focus-within, .applications entry:focus-within { border-color: #2563EB; }
.avertissement { background: #FFF4DB; border-radius: 12px; padding: 10px 14px; color: #6B4A00; }
.avertissement label { color: #6B4A00; }
.messagerie { background: #FFFFFF; border: 1px solid #DCE2EC; border-radius: 18px; padding: 16px; min-width: 180px; }
.messagerie:hover { background: #FAFBFD; }
.pastille { border-radius: 14px; min-width: 52px; min-height: 52px; font-size: 24px; font-weight: 700; color: white; }
.pastille label { color: white; }
popover > contents { background-color: #FFFFFF; border: 1px solid #DCE2EC; border-radius: 14px; padding: 6px; }
popover label { color: #1B2033; }
popover button.menu { background: none; border: none; box-shadow: none; color: #1B2033; padding: 8px 14px; border-radius: 10px; }
popover button.menu:hover { background: #EEF1F6; }
popover button.menu.attention { color: #B4410C; }
scrollbar { background: none; }
scrollbar slider { background: #C3CAD6; }
spinner { color: #2563EB; }
"#;

/// Exécute un travail dans un fil à part, puis la suite dans l'interface.
fn en_fond<T: Send + 'static>(travail: impl FnOnce() -> T + Send + 'static, ensuite: impl FnOnce(T) + 'static) {
    let (envoi, reception) = async_channel::bounded::<T>(1);
    std::thread::spawn(move || {
        let _ = envoi.send_blocking(travail());
    });
    glib::spawn_future_local(async move {
        if let Ok(r) = reception.recv().await {
            ensuite(r);
        }
    });
}

/// (le texte est traduit ici : on peut passer directement le texte français)
fn etiquette(texte: &str, classe: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&tr(texte)));
    l.set_wrap(true);
    l.set_justify(gtk::Justification::Center);
    if !classe.is_empty() {
        l.add_css_class(classe);
    }
    l
}

fn lancer(commande: &[String]) {
    let Some((prog, args)) = commande.split_first() else { return };
    let mut c = Command::new(prog);
    c.args(args);
    // AppImage : sans dépendre de FUSE
    c.env("APPIMAGE_EXTRACT_AND_RUN", "1");
    c.current_dir(applis::maison());
    if let Ok(mut e) = c.spawn() {
        std::thread::spawn(move || {
            let _ = e.wait();
        });
    }
}

/// Session d'essai depuis la clé USB : tout ce qu'on installerait serait perdu à l'extinction
/// (et occuperait la mémoire) → la boutique n'est qu'une vitrine.
fn mode_essai() -> bool {
    std::path::Path::new("/run/live/medium").exists() || std::env::var_os("SOLEOS_TEST_ESSAI").is_some()
}

/// Demande à l'accueil d'ouvrir une application (ou de la ramener devant si elle est déjà ouverte)
fn ouvrir_par_accueil(id: &str) {
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") {
        let _ = std::fs::write(std::path::PathBuf::from(d).join("soleos-ouvrir"), format!("appli:{id}"));
        lancer(&["soleos-accueil".to_string()]);
    } else if let Some(a) = applis::installees().into_iter().find(|a| a.id == id) {
        lancer(&a.commande());
    }
}

/// Image d'une application : son icône, sinon une icône neutre
fn image_appli(chemin: Option<&std::path::Path>, taille: i32) -> gtk::Image {
    let image = gtk::Image::new();
    image.set_pixel_size(taille);
    match icones::appli(chemin, &[], taille) {
        Some(i) => i,
        None => {
            icones::changer(&image, "appli");
            image
        }
    }
}

fn vider(b: &impl IsA<gtk::Widget>) {
    let w = b.as_ref();
    while let Some(e) = w.first_child() {
        e.unparent();
    }
}

struct Fenetre {
    fenetre: gtk::ApplicationWindow,
    pile: gtk::Stack,
    grille_mes: gtk::FlowBox,
    info_mes: gtk::Label,
    boutique: gtk::Box,
    recherche: gtk::Entry,
    resultats: gtk::Box,
    etat_boutique: gtk::Label,
    /// Installations demandées, dans l'ordre : la première est en cours, les autres attendent
    en_cours: RefCell<Vec<String>>,
    /// Boutons « Installer » affichés (les cartes sont recréées : recherche, catalogue mis à jour)
    cartes: RefCell<Vec<(String, glib::WeakRef<gtk::Button>, glib::WeakRef<gtk::Spinner>)>>,
}

impl Fenetre {
    // ------------------------------------------------------------------
    //  Mes applications
    // ------------------------------------------------------------------
    fn afficher_mes(self: &Rc<Self>) {
        while let Some(e) = self.grille_mes.first_child() {
            self.grille_mes.remove(&e);
        }
        let liste = applis::installees();
        self.info_mes.set_label(&if liste.is_empty() {
            tr("Aucune application ajoutée pour l'instant. Touchez « Ajouter » pour en choisir.")
        } else {
            tr("Clic droit (ou appui long) sur une application : désinstaller.")
        });
        for a in liste {
            self.grille_mes.insert(&self.tuile(a), -1);
        }
        // Tuile « Ajouter »
        let image = icones::image("applications", 84);
        let nom = gtk::Label::new(Some(&tr("Ajouter")));
        nom.add_css_class("nom-tuile");
        let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
        b.append(&image);
        b.append(&nom);
        let bouton = gtk::Button::builder().child(&b).build();
        bouton.add_css_class("tuile-appli");
        let moi = Rc::downgrade(self);
        bouton.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.pile.set_visible_child_name("ajouter");
            }
        });
        self.grille_mes.insert(&bouton, -1);
    }

    fn tuile(self: &Rc<Self>, a: Installee) -> gtk::Button {
        let image = image_appli(a.icone.as_deref(), 84);
        let nom = gtk::Label::new(Some(&a.nom));
        nom.add_css_class("nom-tuile");
        nom.set_max_width_chars(14);
        nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
        b.append(&image);
        b.append(&nom);
        if applis::est_epinglee(&a.id) {
            let p = gtk::Label::new(Some(&tr("sur l'accueil")));
            p.add_css_class("discret");
            b.append(&p);
        }
        let bouton = gtk::Button::builder().child(&b).build();
        bouton.add_css_class("tuile-appli");
        bouton.set_tooltip_text(Some(&a.nom));
        let id_appli = a.id.clone();
        let f = self.fenetre.clone();
        bouton.connect_clicked(move |_| {
            // L'accueil ouvre l'application, ou la ramène devant si elle est déjà ouverte
            ouvrir_par_accueil(&id_appli);
            // Retour à l'accueil : l'application s'ouvre par-dessus
            let f = f.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || f.close());
        });

        // Menu : clic droit ou appui long
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let bulle = gtk::Popover::new();
        bulle.set_child(Some(&menu));
        bulle.set_parent(&bouton);
        let entree = |texte: &str| {
            let b = gtk::Button::with_label(texte);
            b.add_css_class("menu");
            b.child().and_downcast::<gtk::Label>().map(|l| l.set_xalign(0.0));
            b
        };
        let epinglee = applis::est_epinglee(&a.id);
        let epingle = entree(&if epinglee { tr("Retirer de l'accueil") } else { tr("Épingler à l'accueil") });
        let desinstaller = entree(&tr("Désinstaller…"));
        desinstaller.add_css_class("attention");
        // Bureau Plasma : pas d'accueil où épingler (les applications se lancent d'ici ou par ⊞)
        if !plasma() {
            menu.append(&epingle);
        }
        // Applications livrées avec SoleOS : pas de désinstallation
        if !matches!(a.genre, applis::Genre::Systeme(_)) {
            menu.append(&desinstaller);
        }
        let (id, moi, bu) = (a.id.clone(), Rc::downgrade(self), bulle.clone());
        epingle.connect_clicked(move |_| {
            if epinglee { applis::retirer_epingle(&id) } else { applis::epingler(&id) }
            bu.popdown();
            if let Some(m) = moi.upgrade() {
                m.afficher_mes();
            }
        });
        let (moi, bu, appli, menu2) = (Rc::downgrade(self), bulle.clone(), a.clone(), menu.clone());
        desinstaller.connect_clicked(move |_| {
            // Confirmation dans la même bulle
            vider(&menu2);
            menu2.append(&etiquette(&trv("Désinstaller « {nom} » ?", &[("nom", &appli.nom)]), ""));
            let oui = gtk::Button::with_label(&tr("Désinstaller"));
            oui.add_css_class("danger");
            let non = gtk::Button::with_label(&tr("Annuler"));
            non.add_css_class("secondaire");
            let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            ligne.set_halign(gtk::Align::Center);
            ligne.set_margin_top(8);
            ligne.append(&non);
            ligne.append(&oui);
            menu2.append(&ligne);
            let bu2 = bu.clone();
            non.connect_clicked(move |_| bu2.popdown());
            let (moi, bu3, appli) = (moi.clone(), bu.clone(), appli.clone());
            oui.connect_clicked(move |b| {
                b.set_sensitive(false);
                b.set_label(&tr("…"));
                let (moi, bu4, appli) = (moi.clone(), bu3.clone(), appli.clone());
                en_fond(move || applis::desinstaller(&appli), move |r| {
                    bu4.popdown();
                    if let Some(m) = moi.upgrade() {
                        m.afficher_mes();
                        if let Err(e) = r {
                            m.info_mes.set_label(&e);
                        }
                    }
                });
            });
        });
        let ouvrir_menu = {
            let b = bulle.clone();
            move || b.popup()
        };
        let clic = gtk::GestureClick::new();
        clic.set_button(3);
        clic.set_propagation_phase(gtk::PropagationPhase::Capture);
        let o = ouvrir_menu.clone();
        clic.connect_pressed(move |g, _, _, _| {
            g.set_state(gtk::EventSequenceState::Claimed);
            o()
        });
        bouton.add_controller(clic);
        let appui = gtk::GestureLongPress::new();
        appui.set_propagation_phase(gtk::PropagationPhase::Capture);
        appui.connect_pressed(move |g, _, _| {
            g.set_state(gtk::EventSequenceState::Claimed);
            ouvrir_menu()
        });
        bouton.add_controller(appui);
        bouton
    }

    // ------------------------------------------------------------------
    //  Boutique : sélection SoleOS
    // ------------------------------------------------------------------
    fn afficher_selection(self: &Rc<Self>) {
        vider(&self.boutique);
        for cat in applis::CATEGORIES {
            let titre = gtk::Label::new(Some(&tr(cat)));
            titre.add_css_class("categorie");
            titre.set_xalign(0.0);
            self.boutique.append(&titre);
            let grille = gtk::FlowBox::new();
            grille.set_selection_mode(gtk::SelectionMode::None);
            grille.set_column_spacing(12);
            grille.set_row_spacing(12);
            grille.set_min_children_per_line(1);
            grille.set_max_children_per_line(4);
            grille.set_homogeneous(true);
            if *cat == "Suite Craft" {
                let intro = etiquette(
                    "Logiciels libres et rapides, écrits en Rust par l'équipe ArtCraft. Encore jeunes : \
                     gardez Bureautique pour les documents importants.",
                    "discret",
                );
                intro.set_xalign(0.0);
                intro.set_justify(gtk::Justification::Left);
                self.boutique.append(&intro);
                for (code, nom, description) in applis::CRAFT {
                    grille.insert(&self.carte(&format!("craft:{code}"), nom, description), -1);
                }
            }
            for p in applis::SELECTION.iter().filter(|p| p.categorie == *cat) {
                grille.insert(&self.carte(p.id, p.nom, p.description), -1);
            }
            self.boutique.append(&grille);
        }
    }

    /// Carte d'une application à ajouter
    fn carte(self: &Rc<Self>, id: &str, nom: &str, description: &str) -> gtk::Box {
        let icone = image_appli(applis::icone_catalogue(id).as_deref(), 48);
        let n = gtk::Label::new(Some(&tr(nom)));
        n.add_css_class("nom-carte");
        n.set_xalign(0.0);
        n.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let d = gtk::Label::new(Some(&tr(description)));
        d.add_css_class("discret");
        d.set_xalign(0.0);
        d.set_wrap(true);
        d.set_lines(2);
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let textes = gtk::Box::new(gtk::Orientation::Vertical, 2);
        textes.set_hexpand(true);
        textes.set_valign(gtk::Align::Center);
        textes.append(&n);
        textes.append(&d);
        let bouton = gtk::Button::new();
        bouton.set_valign(gtk::Align::Center);
        let roue = gtk::Spinner::new();
        roue.set_visible(false);
        let carte = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        carte.add_css_class("carte");
        carte.append(&icone);
        carte.append(&textes);
        carte.append(&roue);
        carte.append(&bouton);
        self.regler_bouton(&bouton, &roue, id);
        self.cartes.borrow_mut().push((id.to_string(), bouton.downgrade(), roue.downgrade()));
        // Banc de captures : clics automatiques sur « Installer » (SOLEOS_TEST_INSTALLER=id1,id2…)
        if let Ok(liste) = std::env::var("SOLEOS_TEST_INSTALLER") {
            if let Some(rang) = liste.split(',').position(|x| x == id) {
                let b = bouton.clone();
                glib::timeout_add_local_once(Duration::from_millis(800 + 300 * rang as u64), move || b.emit_clicked());
            }
        }
        carte
    }

    fn regler_bouton(self: &Rc<Self>, bouton: &gtk::Button, roue: &gtk::Spinner, id: &str) {
        bouton.remove_css_class("principal");
        bouton.remove_css_class("secondaire");
        if let Some(rang) = self.en_cours.borrow().iter().position(|x| x == id) {
            roue.set_visible(rang == 0);
            if rang == 0 { roue.start() } else { roue.stop() }
            bouton.set_label(&if rang == 0 { tr("Installation…") } else { tr("En attente…") });
            bouton.set_sensitive(false);
            bouton.add_css_class("secondaire");
            return;
        }
        roue.stop();
        roue.set_visible(false);
        if mode_essai() {
            bouton.set_label(&tr("Après installation"));
            bouton.set_sensitive(false);
            bouton.add_css_class("secondaire");
            return;
        }
        bouton.set_sensitive(true);
        let installee = applis::est_installee(id);
        bouton.set_label(&if installee { tr("Ouvrir") } else { tr("Installer") });
        bouton.add_css_class(if installee { "secondaire" } else { "principal" });
        // Un seul gestionnaire de clic à la fois
        unsafe {
            if let Some(ancien) = bouton.steal_data::<glib::SignalHandlerId>("clic") {
                bouton.disconnect(ancien);
            }
        }
        let (id, moi, r) = (id.to_string(), Rc::downgrade(self), roue.clone());
        let h = bouton.connect_clicked(move |b| {
            let Some(m) = moi.upgrade() else { return };
            if applis::est_installee(&id) {
                ouvrir_par_accueil(applis::code_craft(&id).unwrap_or(&id));
                let f = m.fenetre.clone();
                glib::timeout_add_local_once(Duration::from_millis(400), move || f.close());
                return;
            }
            m.installer(&id, b, &r);
        });
        unsafe { bouton.set_data("clic", h) };
    }

    /// Met à jour tous les boutons affichés (Installer / Installation… / En attente… / Ouvrir)
    fn mettre_a_jour_boutons(self: &Rc<Self>) {
        self.cartes.borrow_mut().retain(|(_, b, r)| b.upgrade().is_some() && r.upgrade().is_some());
        let cartes = self.cartes.borrow().clone();
        for (id, b, r) in cartes {
            if let (Some(b), Some(r)) = (b.upgrade(), r.upgrade()) {
                self.regler_bouton(&b, &r, &id);
            }
        }
    }

    fn installer(self: &Rc<Self>, id: &str, _bouton: &gtk::Button, _roue: &gtk::Spinner) {
        if self.en_cours.borrow().iter().any(|x| x == id) {
            return;
        }
        self.en_cours.borrow_mut().push(id.to_string());
        self.mettre_a_jour_boutons();
        self.etat_boutique.set_visible(true);
        if self.en_cours.borrow().len() == 1 {
            self.installer_suivante();
        } else {
            self.etat_boutique.set_label(&tr("Une installation est en cours : la suivante démarrera juste après."));
        }
    }

    /// Lance la première installation de la file (une seule à la fois : connexion et disque partagés)
    fn installer_suivante(self: &Rc<Self>) {
        let Some(id) = self.en_cours.borrow().first().cloned() else { return };
        self.mettre_a_jour_boutons();
        let attente = self.en_cours.borrow().len() - 1;
        self.etat_boutique.set_label(&if attente == 0 {
            tr("Téléchargement en cours… La première application peut prendre quelques minutes (éléments communs à télécharger).")
        } else {
            trv("Téléchargement en cours… ({n} en attente)", &[("n", &attente.to_string())])
        });
        let (id2, idc, moi) = (id.clone(), id, Rc::downgrade(self));
        en_fond(move || applis::installer(&id2), move |res| {
            let Some(m) = moi.upgrade() else { return };
            m.en_cours.borrow_mut().retain(|x| x != &idc);
            m.mettre_a_jour_boutons();
            match res {
                Ok(()) => m.etat_boutique.set_label(&tr("Installée : elle est dans « Mes applications ».")),
                Err(e) => m.etat_boutique.set_label(&e),
            }
            m.afficher_mes();
            if !m.en_cours.borrow().is_empty() {
                m.installer_suivante();
            } else if !m.fenetre.is_visible() {
                m.fenetre.close();
            }
        });
    }

    // ------------------------------------------------------------------
    //  Boutique : recherche dans toute la logithèque
    // ------------------------------------------------------------------
    fn chercher(self: &Rc<Self>) {
        let terme = self.recherche.text().trim().to_string();
        vider(&self.resultats);
        if terme.is_empty() {
            self.resultats.set_visible(false);
            self.boutique.set_visible(true);
            return;
        }
        self.boutique.set_visible(false);
        self.resultats.set_visible(true);
        let avert = etiquette(
            "Résultats de toute la logithèque Flathub : ces applications ne sont pas vérifiées par SoleOS. \
             Elles ne peuvent pas abîmer le système, mais installez seulement celles que vous connaissez.",
            "avertissement",
        );
        avert.set_xalign(0.0);
        avert.set_justify(gtk::Justification::Left);
        self.resultats.append(&avert);
        let attente = etiquette(&tr("Recherche…"), "sous-titre");
        self.resultats.append(&attente);
        let moi = Rc::downgrade(self);
        let demande = terme.clone();
        en_fond(move || {
            let liste = applis::chercher(&terme);
            if !liste.is_empty() || mode_essai() {
                return liste;
            }
            // Premier usage : catalogue pas encore téléchargé → on le récupère puis on cherche à nouveau
            if applis::preparer_depot().is_ok() {
                applis::rafraichir_catalogue();
            }
            applis::chercher(&terme)
        }, move |liste| {
            let Some(m) = moi.upgrade() else { return };
            // Résultat d'une ancienne recherche (on a continué à taper) : ignoré
            if m.recherche.text().trim() != demande.as_str() || attente.parent().is_none() {
                return;
            }
            m.resultats.remove(&attente);
            if liste.is_empty() {
                m.resultats.append(&etiquette(&tr("Aucune application trouvée (ou pas de connexion à Internet)."), "sous-titre"));
                return;
            }
            let grille = gtk::FlowBox::new();
            grille.set_selection_mode(gtk::SelectionMode::None);
            grille.set_column_spacing(12);
            grille.set_row_spacing(12);
            grille.set_min_children_per_line(1);
            grille.set_max_children_per_line(3);
            grille.set_homogeneous(true);
            for r in liste {
                // Une appli de la sélection garde son nom français
                let (nom, desc) = match applis::proposee(&r.id) {
                    Some(Proposee { nom, description, .. }) => (tr(nom), format!("{} · {}", tr(description), tr("sélection SoleOS"))),
                    None => (r.nom.clone(), r.description.clone()),
                };
                grille.insert(&m.carte(&r.id, &nom, &desc), -1);
            }
            m.resultats.append(&grille);
        });
    }
}

// =====================================================================
//  Choix de la messagerie (tuile « Mail »)
// =====================================================================

fn page_mail(fenetre: &gtk::ApplicationWindow) -> gtk::Widget {
    let c = gtk::Box::new(gtk::Orientation::Vertical, 18);
    c.set_halign(gtk::Align::Center);
    c.set_valign(gtk::Align::Center);
    let deja = applis::messageries();
    if deja.is_empty() {
        c.append(&etiquette(&tr("Quelle messagerie utilisez-vous ?"), "titre"));
        c.append(&etiquette(
            "La tuile « Mail » l'ouvrira directement. Vous en avez plusieurs (par exemple Gmail et Orange) ?\n\
             Ajoutez la deuxième ensuite : clic droit (ou appui long) sur la tuile « Mail ».",
            "sous-titre",
        ));
    } else {
        c.append(&etiquette(&tr("Ajouter une messagerie"), "titre"));
        let noms: Vec<String> = deja.iter().map(|m| applis::nom_messagerie(m)).collect();
        c.append(&etiquette(
            &trv("Déjà dans la tuile « Mail » : {noms}. En cliquant sur « Mail », vous choisirez laquelle ouvrir.", &[("noms", &noms.join(", "))]),
            "sous-titre",
        ));
    }
    let grille = gtk::FlowBox::new();
    grille.set_selection_mode(gtk::SelectionMode::None);
    grille.set_max_children_per_line(4);
    grille.set_min_children_per_line(1);
    grille.set_column_spacing(14);
    grille.set_row_spacing(14);
    grille.set_homogeneous(true);
    grille.set_margin_top(10);
    let choisir = |cible: String, f: gtk::ApplicationWindow| {
        move |_: &gtk::Button| {
            applis::choisir_messagerie(&cible);
            // L'accueil l'ouvre et affiche « Ouverture… » sur la tuile Mail jusqu'à ce qu'elle apparaisse
            match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(d) if std::fs::write(std::path::PathBuf::from(&d).join("soleos-ouvrir"), format!("mail:{cible}")).is_ok() => {
                    lancer(&["soleos-accueil".to_string()]);
                }
                _ => {
                    if let Some(cmd) = applis::commande_messagerie(&cible) {
                        lancer(&cmd);
                    }
                }
            }
            let f = f.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || f.close());
        }
    };
    let pastille = |lettre: &str, couleur: &str| {
        let l = gtk::Label::new(Some(lettre));
        l.add_css_class("pastille");
        let fournisseur = gtk::CssProvider::new();
        #[allow(deprecated)]
        fournisseur.load_from_data(&format!("label {{ background: {couleur}; }}"));
        #[allow(deprecated)]
        l.style_context().add_provider(&fournisseur, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        l
    };
    for (_, nom, adresse, couleur) in applis::messageries_proposees() {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        b.append(&pastille(&nom.chars().next().unwrap().to_string(), couleur));
        let n = gtk::Label::new(Some(nom));
        n.add_css_class("nom-carte");
        b.append(&n);
        let bouton = gtk::Button::builder().child(&b).build();
        bouton.add_css_class("messagerie");
        bouton.add_css_class("tuile-appli");
        bouton.connect_clicked(choisir(adresse.to_string(), fenetre.clone()));
        grille.insert(&bouton, -1);
    }
    c.append(&grille);

    // Autre adresse, ou le logiciel Thunderbird
    let autre = gtk::Entry::builder().placeholder_text(tr("Autre messagerie : adresse du site (ex. webmail.monentreprise.fr)")).width_request(520).build();
    autre.add_css_class("recherche");
    let f = fenetre.clone();
    autre.connect_activate(move |e| {
        let mut a = e.text().trim().to_string();
        if a.is_empty() {
            return;
        }
        if !a.starts_with("http") {
            a = format!("https://{a}");
        }
        choisir(a, f.clone())(&gtk::Button::new());
    });
    let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    ligne.set_halign(gtk::Align::Center);
    ligne.append(&autre);
    c.append(&ligne);

    let thunderbird = gtk::Button::with_label(&tr("J'utilise le logiciel Thunderbird"));
    thunderbird.add_css_class("secondaire");
    thunderbird.set_halign(gtk::Align::Center);
    let etat = etiquette("", "discret");
    let (f, e) = (fenetre.clone(), etat.clone());
    thunderbird.connect_clicked(move |b| {
        if applis::est_installee("org.mozilla.Thunderbird") {
            choisir("thunderbird".into(), f.clone())(b);
            return;
        }
        b.set_sensitive(false);
        e.set_label(&tr("Installation de Thunderbird… (quelques minutes)"));
        let (f, e, b) = (f.clone(), e.clone(), b.clone());
        en_fond(|| applis::installer("org.mozilla.Thunderbird"), move |r| match r {
            Ok(()) => choisir("thunderbird".into(), f.clone())(&b),
            Err(m) => {
                e.set_label(&m);
                b.set_sensitive(true);
            }
        });
    });
    c.append(&thunderbird);
    c.append(&etat);
    let fermer = gtk::Button::with_label(&tr("Annuler"));
    fermer.add_css_class("fermer");
    fermer.set_halign(gtk::Align::Center);
    let f = fenetre.clone();
    fermer.connect_clicked(move |_| f.close());
    c.append(&fermer);
    c.upcast()
}

// =====================================================================
//  Construction
// =====================================================================

fn agrandir(fenetre: &gtk::ApplicationWindow) {
    // Taille de départ modeste (jamais plus grande que l'écran), puis agrandie à tout l'écran
    fenetre.set_default_size(800, 600);
    theme::adapter(fenetre);
    // Applications : en clair, comme les autres applications
    if let Some(r) = gtk::Settings::default() {
        r.set_gtk_application_prefer_dark_theme(false);
    }
    theme::verifier_si_demande(fenetre);
    if plasma() {
        // Bureau SoleOS : fenêtre à 70 % de l'écran (au moins 900 × 600), qu'on peut agrandir
        let (l, h) = gtk::gdk::Display::default()
            .and_then(|d| d.monitors().item(0))
            .and_downcast::<gtk::gdk::Monitor>()
            .map(|m| (m.geometry().width(), m.geometry().height()))
            .unwrap_or((1366, 768));
        fenetre.set_default_size((l * 70 / 100).max(900).min(l), (h * 70 / 100).max(600).min(h - 60));
    } else {
        theme::garder_plein_ecran(fenetre, false);
    }
}

/// Bureau SoleOS v12.5 (barre magique, barre du bas, éclair) : fenêtre agrandie comme les
/// autres, rien à épingler (toutes les applications sont dans le panneau de l'éclair)
fn plasma() -> bool {
    true
}

fn construire(app: &gtk::Application, mode: &str) {
    let fenetre = gtk::ApplicationWindow::builder().application(app).title(tr("Applications")).decorated(true).build();
    fenetre.add_css_class("applications");
    agrandir(&fenetre);

    if mode == "--mail" {
        fenetre.set_title(Some(&tr("Messagerie")));
        fenetre.set_child(Some(&theme::defilant(&page_mail(&fenetre))));
        fenetre.present();
        return;
    }

    // En-tête : onglets et retour
    let pile = gtk::Stack::new();
    pile.set_transition_type(gtk::StackTransitionType::Crossfade);
    let onglets = gtk::StackSwitcher::new();
    onglets.set_stack(Some(&pile));
    onglets.add_css_class("onglets");
    let titre = etiquette(&tr("Applications"), "titre");
    let fermer = gtk::Button::with_label(&if plasma() { format!("✕  {}", tr("Fermer")) } else { tr("✕  Retour à l'accueil") });
    fermer.add_css_class("fermer");
    let f = fenetre.clone();
    fermer.connect_clicked(move |_| f.close());
    let entete = gtk::CenterBox::new();
    entete.set_start_widget(Some(&titre));
    entete.set_center_widget(Some(&onglets));
    entete.set_end_widget(Some(&fermer));
    entete.set_margin_top(24);
    entete.set_margin_start(40);
    entete.set_margin_end(40);

    // Mes applications
    let info_mes = etiquette("", "discret");
    let grille_mes = gtk::FlowBox::new();
    grille_mes.set_selection_mode(gtk::SelectionMode::None);
    grille_mes.set_max_children_per_line(8);
    grille_mes.set_min_children_per_line(2);
    grille_mes.set_column_spacing(18);
    grille_mes.set_row_spacing(18);
    grille_mes.set_homogeneous(true);
    grille_mes.set_valign(gtk::Align::Start);
    let page_mes = gtk::Box::new(gtk::Orientation::Vertical, 16);
    page_mes.set_margin_top(20);
    page_mes.append(&info_mes);
    page_mes.append(&grille_mes);
    let defil_mes = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&page_mes).vexpand(true).build();
    pile.add_titled(&defil_mes, Some("mes"), &tr("Mes applications"));

    // Boutique
    let recherche = gtk::Entry::builder().placeholder_text(tr("Chercher parmi des milliers d'applications (VLC, GIMP, Steam…)")).width_request(560).build();
    recherche.add_css_class("recherche");
    recherche.set_halign(gtk::Align::Center);
    let etat_boutique = etiquette("", "sous-titre");
    etat_boutique.set_visible(false);
    let boutique = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let resultats = gtk::Box::new(gtk::Orientation::Vertical, 12);
    resultats.set_visible(false);
    let page_ajout = gtk::Box::new(gtk::Orientation::Vertical, 10);
    page_ajout.set_margin_top(20);
    if mode_essai() {
        let bandeau = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        bandeau.add_css_class("avertissement");
        bandeau.set_halign(gtk::Align::Center);
        let t = etiquette(&tr("Mode essai : installez SoleOS sur l'ordinateur pour ajouter des applications."), "");
        t.set_valign(gtk::Align::Center);
        let installer = gtk::Button::with_label(&tr("Installer SoleOS"));
        installer.add_css_class("principal");
        installer.connect_clicked(|_| lancer(&["soleos-installateur".to_string()]));
        bandeau.append(&t);
        bandeau.append(&installer);
        page_ajout.append(&bandeau);
    }
    page_ajout.append(&recherche);
    page_ajout.append(&etiquette(&tr("Une application n'est pas dans la sélection ? Tapez son nom : toute la logithèque Flathub est disponible."), "discret"));
    page_ajout.append(&etat_boutique);
    // Applications téléchargées sur le site de leur éditeur (.deb, .AppImage)
    if !mode_essai() {
        let zone = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone.add_css_class("carte");
        let t = etiquette(&tr("Depuis un fichier"), "nom-carte");
        t.set_xalign(0.0);
        zone.append(&t);
        let explication = etiquette(
            &tr("Une application téléchargée sur le site de son éditeur (Steam, Spotify, TeamViewer…) : fichier .deb ou .AppImage. Les .deb s'installent dans un espace Debian séparé, qui ne peut pas abîmer SoleOS."),
            "discret",
        );
        explication.set_xalign(0.0);
        explication.set_wrap(true);
        zone.append(&explication);
        let choisir = gtk::Button::with_label(&tr("Installer un fichier téléchargé…"));
        choisir.add_css_class("principal");
        choisir.set_halign(gtk::Align::Start);
        let resultat = etiquette("", "discret");
        resultat.set_xalign(0.0);
        resultat.set_wrap(true);
        zone.append(&choisir);
        zone.append(&resultat);
        let r = resultat.clone();
        choisir.connect_clicked(move |b| choisir_fichier_appli(b, &r));
        page_ajout.append(&zone);
    }
    page_ajout.append(&etiquette(&tr("Sélection SoleOS : des applications vérifiées, installées en un clic."), "discret"));
    page_ajout.append(&boutique);
    page_ajout.append(&resultats);
    let defil_ajout = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&page_ajout).vexpand(true).build();
    pile.add_titled(&defil_ajout, Some("ajouter"), &tr("Ajouter des applications"));

    // Largeur de lecture confortable, centrée
    let corps = gtk::Box::new(gtk::Orientation::Vertical, 0);
    corps.set_halign(gtk::Align::Center);
    // Au plus 1000 px de large, moins sur un petit écran (sinon la fenêtre déborderait)
    corps.set_size_request(1000.min(fenetre.width().max(800) - 64), -1);
    {
        let (f, c, t) = (fenetre.downgrade(), corps.clone(), titre.clone());
        glib::timeout_add_local(Duration::from_millis(400), move || {
            let Some(f) = f.upgrade() else { return glib::ControlFlow::Break };
            let l = f.width();
            if l > 0 {
                // Écran étroit : le grand titre laisse la place aux onglets
                t.set_visible(l >= 960);
                let voulu = 1000.min(l - 64).max(320);
                if c.width_request() != voulu {
                    c.set_size_request(voulu, -1);
                }
            }
            glib::ControlFlow::Continue
        });
    }
    corps.set_hexpand(false);
    corps.set_vexpand(true);
    corps.append(&pile);
    let tout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tout.append(&entete);
    tout.append(&corps);
    fenetre.set_child(Some(&tout));

    let f = Rc::new(Fenetre {
        fenetre: fenetre.clone(),
        pile: pile.clone(),
        grille_mes,
        info_mes,
        boutique,
        recherche: recherche.clone(),
        resultats,
        etat_boutique,
        en_cours: RefCell::new(Vec::new()),
        cartes: RefCell::new(Vec::new()),
    });
    // Fermer pendant des installations : la fenêtre se cache, les installations continuent,
    // puis tout se ferme à la fin (rouvrir « Applications » la fait réapparaître)
    let moi = Rc::downgrade(&f);
    fenetre.connect_close_request(move |w| {
        if moi.upgrade().map(|m| !m.en_cours.borrow().is_empty()).unwrap_or(false) {
            w.set_visible(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    let garde = RefCell::new(Some(f.clone()));
    fenetre.connect_destroy(move |_| {
        garde.borrow_mut().take();
    });
    f.afficher_mes();
    f.afficher_selection();
    if mode == "--ajouter" {
        pile.set_visible_child_name("ajouter");
        let r = recherche.clone();
        glib::idle_add_local_once(move || { r.grab_focus(); });
    }
    // Onglet « Ajouter » : on peut taper directement le nom d'une application
    let r = recherche.clone();
    pile.connect_visible_child_name_notify(move |p| {
        if p.visible_child_name().as_deref() == Some("ajouter") {
            r.grab_focus();
        }
    });
    let moi = Rc::downgrade(&f);
    recherche.connect_activate(move |_| {
        if let Some(m) = moi.upgrade() {
            m.chercher();
        }
    });
    let moi = Rc::downgrade(&f);
    recherche.connect_changed(move |e| {
        let texte = e.text().to_string();
        if texte.is_empty() {
            if let Some(m) = moi.upgrade() {
                m.chercher();
            }
            return;
        }
        // Recherche dès qu'on s'arrête de taper (pas besoin d'appuyer sur Entrée)
        if texte.trim().chars().count() >= 3 {
            let (moi, e) = (moi.clone(), e.clone());
            glib::timeout_add_local_once(Duration::from_millis(700), move || {
                if e.text() == texte.as_str() {
                    if let Some(m) = moi.upgrade() {
                        m.chercher();
                    }
                }
            });
        }
    });
    // Mes applications à jour quand on y revient (après une installation…)
    let moi = Rc::downgrade(&f);
    pile.connect_visible_child_name_notify(move |p| {
        if p.visible_child_name().as_deref() == Some("mes") {
            if let Some(m) = moi.upgrade() {
                m.afficher_mes();
            }
        }
    });
    // Catalogue de la boutique (icônes, recherche) mis à jour en arrière-plan
    let moi = Rc::downgrade(&f);
    en_fond(
        || {
            if mode_essai() {
                return Ok(());
            }
            let r = applis::preparer_depot();
            if r.is_ok() {
                applis::rafraichir_catalogue();
            }
            r
        },
        move |r| {
            let Some(m) = moi.upgrade() else { return };
            if let Err(e) = r {
                m.etat_boutique.set_visible(true);
                m.etat_boutique.set_label(&e);
            }
            m.afficher_selection();
        },
    );
    fenetre.present();
}

/// Paquet .deb ouvert depuis Fichiers : installation dans l'espace Debian (conteneur), avec confirmation
/// Choix d'un fichier .deb ou .AppImage (dossier Téléchargements), puis installation
fn choisir_fichier_appli(depuis: &gtk::Button, resultat: &gtk::Label) {
    let parent = depuis.root().and_then(|r| r.downcast::<gtk::Window>().ok());
    #[allow(deprecated)]
    let choix = gtk::FileChooserNative::new(
        Some(&tr("Installer une application")),
        parent.as_ref(),
        gtk::FileChooserAction::Open,
        Some(&tr("Installer")),
        Some(&tr("Annuler")),
    );
    let filtre = gtk::FileFilter::new();
    filtre.set_name(Some(&tr("Applications (.deb, .AppImage)")));
    for motif in ["*.deb", "*.AppImage", "*.appimage"] {
        filtre.add_pattern(motif);
    }
    #[allow(deprecated)]
    choix.add_filter(&filtre);
    if let Some(d) = glib::user_special_dir(glib::UserDirectory::Downloads) {
        #[allow(deprecated)]
        let _ = choix.set_current_folder(Some(&gtk::gio::File::for_path(d)));
    }
    let r = resultat.clone();
    #[allow(deprecated)]
    choix.connect_response(move |c, reponse| {
        #[allow(deprecated)]
        let fichier = if reponse == gtk::ResponseType::Accept { c.file().and_then(|f| f.path()) } else { None };
        let Some(f) = fichier else { return };
        let nom = f.to_string_lossy().to_lowercase();
        if nom.ends_with(".deb") {
            // Fenêtre dédiée : explications, progression, résultat
            let _ = std::process::Command::new("soleos-applications").arg("--deb").arg(&f).spawn();
            r.set_label(&tr("Installation du paquet dans une fenêtre à part…"));
        } else {
            r.set_label(&tr("Ajout…"));
            let r2 = r.clone();
            en_fond(move || applis::ajouter_appimage(&f), move |res| match res {
                Ok(n) => r2.set_label(&format!("✓ {}", trv("« {nom} » est dans « Mes applications ».", &[("nom", &n)]))),
                Err(e) => r2.set_label(&e),
            });
        }
    });
    #[allow(deprecated)]
    choix.show();
    // Le sélecteur doit rester en vie jusqu'à la réponse
    std::mem::forget(choix);
}

fn fenetre_deb(programme: &str, fichier: std::path::PathBuf) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id("fr.soleos.PaquetDeb").build();
    charger_style(&app);
    app.connect_activate(move |app| {
        let f = gtk::ApplicationWindow::builder().application(app).title(tr("Installer un paquet")).default_width(620).default_height(340).build();
        f.add_css_class("applications");
        let c = gtk::Box::new(gtk::Orientation::Vertical, 16);
        c.set_valign(gtk::Align::Center);
        c.set_margin_start(36);
        c.set_margin_end(36);
        let nom_fichier = fichier.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (paquet, lanceurs) = applis::contenu_deb(&fichier);
        c.append(&etiquette(&trv("Installer « {nom} » ?", &[("nom", if paquet.is_empty() { &nom_fichier } else { &paquet })]), "sous-titre"));
        let explication = etiquette(
            "Ce paquet sera installé dans un espace Debian séparé : il ne peut pas abîmer SoleOS. \
             La première fois, cet espace est téléchargé (environ 50 Mo) : comptez quelques minutes.",
            "discret",
        );
        explication.set_wrap(true);
        explication.set_max_width_chars(70);
        explication.set_justify(gtk::Justification::Center);
        explication.set_halign(gtk::Align::Center);
        c.append(&explication);
        if lanceurs.is_empty() {
            c.append(&etiquette(&tr("Ce paquet ne contient pas d'application à afficher (outil en ligne de commande) : il sera utilisable dans la Console avec « distrobox enter soleos-debian »."), "discret"));
        }
        let etat = etiquette("", "discret");
        etat.set_wrap(true);
        etat.set_max_width_chars(70);
        etat.set_justify(gtk::Justification::Center);
        etat.set_halign(gtk::Align::Center);
        let roue = gtk::Spinner::new();
        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        ligne.set_halign(gtk::Align::Center);
        let annuler = gtk::Button::with_label(&tr("Annuler"));
        annuler.add_css_class("secondaire");
        let installer = gtk::Button::with_label(&tr("Installer"));
        installer.add_css_class("principal");
        ligne.append(&annuler);
        ligne.append(&installer);
        c.append(&roue);
        c.append(&etat);
        c.append(&ligne);
        if mode_essai() {
            installer.set_sensitive(false);
            etat.set_label(&tr("En mode essai (clé USB), installez d'abord SoleOS sur l'ordinateur."));
        }
        // Docker : il lui faut un service administrateur, impossible dans l'espace Debian séparé.
        // SoleOS a déjà Podman, avec la commande « docker » (mêmes commandes, dans la Console).
        if matches!(paquet.as_str(), "docker.io" | "docker-ce" | "docker-ce-cli" | "containerd.io" | "docker-desktop" | "docker-compose-plugin") {
            installer.set_sensitive(false);
            etat.set_label(&tr("Docker n'a pas besoin d'être installé : SoleOS contient déjà Podman, qui fait la même chose. Dans la Console, tapez simplement les commandes « docker » habituelles (docker run, docker ps…)."));
        }
        let f2 = f.clone();
        annuler.connect_clicked(move |_| f2.close());
        let (fi, e, r, a) = (fichier.clone(), etat.clone(), roue.clone(), annuler.clone());
        installer.connect_clicked(move |b| {
            b.set_sensitive(false);
            a.set_sensitive(false);
            r.start();
            e.set_label(&tr("Installation en cours…"));
            // Temps écoulé affiché : la première fois, l'espace Debian se prépare (quelques minutes)
            let debut = std::time::Instant::now();
            let fini = std::rc::Rc::new(std::cell::Cell::new(false));
            {
                let (e, fini) = (e.clone(), fini.clone());
                glib::timeout_add_seconds_local(1, move || {
                    if fini.get() {
                        return glib::ControlFlow::Break;
                    }
                    let s = debut.elapsed().as_secs();
                    e.set_label(&format!(
                        "{} ({})",
                        tr("Installation en cours… la première fois, l'espace Debian se prépare"),
                        trv("{m} min {s} s", &[("m", &(s / 60).to_string()), ("s", &format!("{:02}", s % 60))])
                    ));
                    glib::ControlFlow::Continue
                });
            }
            let (fi, e, r, a, b) = (fi.clone(), e.clone(), r.clone(), a.clone(), b.clone());
            en_fond(move || applis::installer_deb(&fi), move |res| {
                fini.set(true);
                r.stop();
                a.set_sensitive(true);
                a.set_label(&tr("Fermer"));
                match res {
                    Ok(nom) => {
                        b.set_visible(false);
                        e.set_label(&format!(
                            "✓ {}",
                            trv("« {nom} » est installé. Ses applications sont dans « Applications » et dans le menu SoleOS.", &[("nom", &nom)])
                        ));
                    }
                    Err(err) => {
                        e.set_label(&err);
                        b.set_sensitive(true);
                    }
                }
            });
        });
        f.set_child(Some(&c));
        f.present();
    });
    app.run_with_args(&[programme.to_string()])
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    tr::initialiser();
    // Ajout d'un AppImage (ouvert depuis Fichiers) : sans fenêtre principale
    let mode = args.get(1).cloned().unwrap_or_default();
    // Commandes en texte (assistance, administration) : mêmes fonctions que les boutons
    let cible = args.get(2).cloned().unwrap_or_default();
    let fin = |r: Result<(), String>| match r {
        Ok(()) => glib::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            glib::ExitCode::FAILURE
        }
    };
    match mode.as_str() {
        "--liste" => {
            for a in applis::installees() {
                println!("{}\t{}\t{}{}", a.id, a.nom, match a.genre { applis::Genre::Flatpak => "flatpak", applis::Genre::AppImage(_) => "appimage", applis::Genre::Systeme(_) => "soleos", applis::Genre::Web(_) => "site", applis::Genre::Lanceur(..) => "raccourci" }, if applis::est_epinglee(&a.id) { "\tsur l'accueil" } else { "" });
            }
            return glib::ExitCode::SUCCESS;
        }
        "--installer" => return fin(applis::installer(&cible)),
        "--desinstaller" => {
            let Some(a) = applis::installees().into_iter().find(|a| a.id == cible) else { return fin(Err(tr("Pas installée"))) };
            return fin(applis::desinstaller(&a));
        }
        "--epingler" => {
            applis::epingler(&cible);
            return glib::ExitCode::SUCCESS;
        }
        "--chercher" => {
            for r in applis::chercher(&cible) {
                println!("{}\t{}\t{}", r.id, r.nom, r.description);
            }
            return glib::ExitCode::SUCCESS;
        }
        _ => {}
    }
    if mode == "--deb" {
        return fenetre_deb(&args[0], std::path::PathBuf::from(cible));
    }
    if mode == "--ajouter-appimage" {
        let fichier = args.get(2).cloned().unwrap_or_default();
        let r = if mode_essai() {
            Err(tr("en mode essai (clé USB), installez d'abord SoleOS sur l'ordinateur."))
        } else {
            applis::ajouter_appimage(std::path::Path::new(&fichier))
        };
        let app = gtk::Application::builder().application_id("fr.soleos.AjoutAppImage").build();
        app.connect_activate(move |app| {
            let f = gtk::ApplicationWindow::builder().application(app).title(tr("Application ajoutée")).default_width(520).default_height(260).build();
            f.add_css_class("applications");
            let c = gtk::Box::new(gtk::Orientation::Vertical, 16);
            c.set_valign(gtk::Align::Center);
            c.set_margin_start(30);
            c.set_margin_end(30);
            match &r {
                Ok(nom) => {
                    c.append(&etiquette(&trv("« {nom} » a été ajoutée à vos applications.", &[("nom", &nom)]), "sous-titre"));
                    c.append(&etiquette(&tr("Retrouvez-la dans la tuile « Applications » de l'accueil."), "discret"));
                }
                Err(e) => c.append(&etiquette(&trv("Impossible d'ajouter cette application : {e}", &[("e", &e)]), "sous-titre")),
            }
            let ok = gtk::Button::with_label(&tr("OK"));
            ok.add_css_class("principal");
            ok.set_halign(gtk::Align::Center);
            let f2 = f.clone();
            ok.connect_clicked(move |_| f2.close());
            c.append(&ok);
            f.set_child(Some(&c));
            f.present();
        });
        charger_style(&app);
        return app.run_with_args(&[args[0].clone()]);
    }

    let app = gtk::Application::builder().application_id("fr.soleos.Applications").build();
    charger_style(&app);
    app.connect_activate(move |app| {
        if let Some(f) = app.active_window() {
            f.present();
        } else {
            construire(app, &mode);
        }
    });
    // Nos options sont lues ci-dessus : GTK ne doit pas les voir
    app.run_with_args(&[args[0].clone()])
}

fn charger_style(app: &gtk::Application) {
    app.connect_startup(|_| {
        let style = gtk::CssProvider::new();
        #[allow(deprecated)]
        style.load_from_data(STYLE);
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&ecran, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    });
}
