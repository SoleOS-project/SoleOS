//! SoleOS – assistant de bienvenue (premier démarrage après l'installation).
//!
//! 1. Prénom
//! 2. Code à 6 chiffres (puce TPM), sinon mot de passe d'au moins 8 caractères ; tapé deux fois
//! 3. Clé de secours : affichée, à recopier sur papier ; vérification du premier groupe
//! 4. C'est prêt
//!
//! `--nouveau-code` : SoleOS a été ouvert avec la clé de secours (code oublié) :
//! on choisit simplement un nouveau code ; la clé de secours reste la même.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::securite::{self, Etat, Pave};
use crate::theme;
use crate::wifi::en_fond;
use crate::tr::{tr, trv};

pub const ID_BIENVENUE: &str = "fr.flashos.Bienvenue";

const STYLE: &str = r#"
window.bienvenue {
  background-image: linear-gradient(115deg, transparent 72%, alpha(#FFC23D, 0.07) 72.2%, transparent 72.6%);
}
.titre { font-size: 42px; }
.sous-titre { font-size: 17px; }
.discret { font-size: 14px; }
.entete { padding: 26px 44px 10px 44px; }
.compact .entete { padding: 16px 28px 6px 28px; }
.mini .entete { padding: 10px 18px 4px 18px; }
entry, passwordentry { min-height: 50px; padding: 0 16px; font-size: 19px; }
button.principal, button.secondaire { padding: 12px 30px; font-size: 17px; }
.mini button.principal, .mini button.secondaire { padding: 8px 20px; font-size: 16px; }
checkbutton { font-size: 16px; }
checkbutton.case-papier { font-size: 18px; font-weight: 600; margin-top: 6px; }
checkbutton.case-papier check { min-width: 26px; min-height: 26px; }
"#;

/// (le texte est traduit ici : on peut passer directement le texte français)
fn etiquette(texte: &str, classe: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&tr(texte)));
    l.add_css_class(classe);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_xalign(0.0);
    l.set_justify(gtk::Justification::Left);
    l.set_max_width_chars(60);
    l
}

/// Colonne de contenu : centrée, au plus ~680 px de large, réduite sur les petits écrans
fn colonne() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 18);
    b.set_halign(gtk::Align::Center);
    b.set_valign(gtk::Align::Center);
    b.set_vexpand(true);
    b.set_margin_start(24);
    b.set_margin_end(24);
    b.set_margin_top(12);
    b.set_margin_bottom(12);
    b.set_size_request(320, -1);
    b
}

fn bouton(texte: &str, classe: &str) -> gtk::Button {
    let b = gtk::Button::with_label(&tr(texte));
    b.add_css_class(classe);
    b
}

/// Barre du bas, toujours visible : à gauche (case, retour…), à droite le bouton principal
fn barre(gauche: Option<&gtk::Widget>, droite: &[&gtk::Widget]) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    b.add_css_class("barre-actions");
    if let Some(g) = gauche {
        g.set_valign(gtk::Align::Center);
        b.append(g);
    }
    let vide = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vide.set_hexpand(true);
    b.append(&vide);
    for d in droite {
        d.set_valign(gtk::Align::Center);
        b.append(*d);
    }
    b
}

struct Assistant {
    fenetre: gtk::ApplicationWindow,
    pages: gtk::Stack,
    etat: Etat,
    nouveau_code: bool,
    prenom: RefCell<String>,
    code: RefCell<String>,
    cle: RefCell<String>,
    clavier: RefCell<Option<gtk::EventControllerKey>>,
}

impl Assistant {
    /// Page complète : en-tête (marque, étape), contenu qui défile si besoin, barre d'actions fixe
    fn montrer(&self, contenu: &gtk::Box, nom: &str, etape: Option<u32>, actions: Option<gtk::Box>) {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let entete = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        entete.add_css_class("entete");
        entete.append(&theme::marque());
        if let Some(v) = theme::pastille_version() {
            entete.append(&v);
        }
        let vide = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        vide.set_hexpand(true);
        entete.append(&vide);
        if let (Some(n), false) = (etape, self.nouveau_code) {
            entete.append(&theme::segments(n, 3, &trv("Étape {n} sur 3", &[("n", &n.to_string())])));
        }
        page.append(&entete);
        page.append(&theme::defilant(contenu));
        if let Some(a) = actions {
            page.append(&a);
        }
        if let Some(ancienne) = self.pages.child_by_name(nom) {
            self.pages.remove(&ancienne);
        }
        self.pages.add_named(&page, Some(nom));
        self.pages.set_visible_child_name(nom);
    }

    // --- 1. Prénom -----------------------------------------------------------
    fn page_prenom(self: &Rc<Self>) {
        let c = colonne();
        c.append(&etiquette(&tr("Bienvenue sur SoleOS"), "titre"));
        c.append(&etiquette(&tr("Quelques secondes pour protéger vos fichiers, et c'est prêt."), "sous-titre"));
        c.append(&etiquette(&tr("Comment vous appelez-vous ?"), "sous-titre"));
        let saisie = gtk::Entry::builder().placeholder_text(tr("Votre prénom")).hexpand(true).build();
        saisie.set_text(&self.prenom.borrow());
        c.append(&saisie);
        let suivant = bouton(&tr("Suivant"), "principal");
        suivant.set_sensitive(!self.prenom.borrow().is_empty());
        let s2 = suivant.clone();
        saisie.connect_changed(move |e| s2.set_sensitive(!e.text().trim().is_empty()));
        let valider = {
            let moi = Rc::downgrade(self);
            let saisie = saisie.clone();
            move || {
                let Some(m) = moi.upgrade() else { return };
                let p = saisie.text().trim().to_string();
                if p.is_empty() {
                    return;
                }
                *m.prenom.borrow_mut() = p;
                m.page_code(false, None);
            }
        };
        let v2 = valider.clone();
        suivant.connect_clicked(move |_| v2());
        saisie.connect_activate(move |_| valider());
        self.montrer(&c, "prenom", Some(1), Some(barre(None, &[suivant.upcast_ref()])));
        saisie.grab_focus();
    }

    // --- 2. Code -------------------------------------------------------------
    /// `confirmation` : deuxième saisie ; `erreur` : message à afficher
    fn page_code(self: &Rc<Self>, confirmation: bool, erreur: Option<&str>) {
        if !self.etat.par_code() {
            return self.page_mot_de_passe(erreur);
        }
        let c = colonne();
        c.set_spacing(14);
        let titre = match (confirmation, self.nouveau_code) {
            (true, _) => tr("Tapez-le une deuxième fois"),
            (false, true) => tr("Choisissez un nouveau code"),
            (false, false) => tr("Choisissez votre code"),
        };
        c.append(&etiquette(&titre, "titre"));
        c.append(&etiquette(
            &if confirmation {
                tr("Pour être sûr qu'il n'y a pas de faute de frappe.")
            } else {
                tr("6 chiffres, à taper à chaque démarrage et pour déverrouiller l'écran. Évitez 123456 ou votre date de naissance.")
            },
            "sous-titre",
        ));
        let pave = Pave::nouveau();
        if let Some(e) = erreur {
            pave.dire(e);
        }
        c.append(&pave.racine);
        let mut actions = None;
        if confirmation {
            let retour = bouton(&tr("Recommencer"), "secondaire");
            let moi = Rc::downgrade(self);
            retour.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.page_code(false, None);
                }
            });
            actions = Some(barre(Some(retour.upcast_ref()), &[]));
        } else if !self.nouveau_code {
            let retour = bouton(&tr("Retour"), "secondaire");
            let moi = Rc::downgrade(self);
            retour.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.page_prenom();
                }
            });
            actions = Some(barre(Some(retour.upcast_ref()), &[]));
        }
        let moi = Rc::downgrade(self);
        pave.quand_complet(move |code| {
            let Some(m) = moi.upgrade() else { return };
            if !confirmation {
                if trop_simple(&code) {
                    m.page_code(false, Some(&tr("Ce code est trop facile à deviner. Choisissez-en un autre.")));
                    return;
                }
                *m.code.borrow_mut() = code;
                m.page_code(true, None);
            } else if *m.code.borrow() != code {
                m.code.borrow_mut().clear();
                m.page_code(false, Some(&tr("Les deux codes sont différents. Recommencez.")));
            } else {
                m.apres_code();
            }
        });
        // Clavier : écouté sur la fenêtre (un seul pavé actif à la fois)
        if let Some(ancien) = self.clavier.borrow_mut().take() {
            self.fenetre.remove_controller(&ancien);
        }
        *self.clavier.borrow_mut() = Some(pave.ecouter_clavier(&self.fenetre));
        self.montrer(&c, if confirmation { "code2" } else { "code1" }, Some(2), actions);
    }

    /// PC sans puce de sécurité : mot de passe d'au moins 8 caractères
    fn page_mot_de_passe(self: &Rc<Self>, erreur: Option<&str>) {
        let c = colonne();
        c.append(&etiquette(
            &if self.nouveau_code { tr("Choisissez un nouveau mot de passe") } else { tr("Choisissez votre mot de passe") },
            "titre",
        ));
        c.append(&etiquette(
            &tr("Au moins 8 caractères, à taper à chaque démarrage et pour déverrouiller l'écran. \
             (Cet ordinateur n'a pas de puce de sécurité : un code à 6 chiffres ne suffirait pas.)"),
            "sous-titre",
        ));
        let mdp1 = gtk::PasswordEntry::new();
        mdp1.set_show_peek_icon(true);
        mdp1.set_property("placeholder-text", tr("Mot de passe"));
        let mdp2 = gtk::PasswordEntry::new();
        mdp2.set_show_peek_icon(true);
        mdp2.set_property("placeholder-text", tr("Encore une fois"));
        for m in [&mdp1, &mdp2] {
            m.set_hexpand(true);
            c.append(m);
        }
        let message = etiquette(erreur.unwrap_or(""), "erreur");
        c.append(&message);
        let suivant = bouton(&tr("Suivant"), "principal");
        suivant.set_sensitive(false);
        let maj = {
            let (a, b, s, msg) = (mdp1.clone(), mdp2.clone(), suivant.clone(), message.clone());
            move || {
                let (x, y) = (a.text().to_string(), b.text().to_string());
                let assez = x.chars().count() >= securite::LONGUEUR_MDP;
                s.set_sensitive(assez && x == y);
                msg.set_label(&if !x.is_empty() && !assez {
                    tr("Encore un peu : au moins 8 caractères.")
                } else if assez && !y.is_empty() && x != y && y.chars().count() >= x.chars().count() {
                    tr("Les deux mots de passe sont différents.")
                } else {
                    String::new()
                });
            }
        };
        let m1 = maj.clone();
        mdp1.connect_changed(move |_| m1());
        let m2 = maj.clone();
        mdp2.connect_changed(move |_| m2());
        let valider = {
            let moi = Rc::downgrade(self);
            let (a, b) = (mdp1.clone(), mdp2.clone());
            move || {
                let Some(m) = moi.upgrade() else { return };
                let x = a.text().to_string();
                if x.chars().count() < securite::LONGUEUR_MDP || x != b.text().as_str() {
                    return;
                }
                *m.code.borrow_mut() = x;
                m.apres_code();
            }
        };
        let v2 = valider.clone();
        suivant.connect_clicked(move |_| v2());
        let v3 = valider.clone();
        mdp2.connect_activate(move |_| v3());
        let b2 = mdp2.clone();
        mdp1.connect_activate(move |_| {
            b2.grab_focus();
        });
        self.montrer(&c, "mdp", Some(2), Some(barre(None, &[suivant.upcast_ref()])));
        mdp1.grab_focus();
    }

    fn apres_code(self: &Rc<Self>) {
        if self.nouveau_code {
            self.enregistrer();
        } else {
            self.page_cle();
        }
    }

    // --- 3. Clé de secours ---------------------------------------------------
    fn page_cle(self: &Rc<Self>) {
        let c = colonne();
        c.append(&etiquette(&tr("Votre clé de secours"), "titre"));
        let attente = gtk::Spinner::new();
        attente.set_spinning(true);
        attente.set_size_request(36, 36);
        c.append(&attente);
        self.montrer(&c, "cle", Some(3), None);
        // Création de la clé (et effacement de toute clé précédente)
        let moi = Rc::downgrade(self);
        en_fond(
            || securite::appeler(&["preparer-secours"], ""),
            move |r| {
                let Some(m) = moi.upgrade() else { return };
                match r.map(|l| securite::cle_dans(&l)) {
                    Ok(Some(cle)) => {
                        *m.cle.borrow_mut() = cle;
                        m.page_cle_affichee();
                    }
                    Ok(None) => m.page_probleme(&tr("La clé de secours n'a pas pu être créée.")),
                    Err(e) => m.page_probleme(&e),
                }
            },
        );
    }

    fn page_cle_affichee(self: &Rc<Self>) {
        let c = colonne();
        c.append(&etiquette(&tr("Votre clé de secours"), "titre"));
        c.append(&etiquette(
            &self.etat.selon(
                "Si un jour vous oubliez votre code, cette clé est le seul moyen d'ouvrir vos fichiers. \
                 Écrivez-la sur papier et rangez-la en lieu sûr, pas dans l'ordinateur.",
                "Si un jour vous oubliez votre mot de passe, cette clé est le seul moyen d'ouvrir vos fichiers. \
                 Écrivez-la sur papier et rangez-la en lieu sûr, pas dans l'ordinateur.",
            ),
            "sous-titre",
        ));
        c.append(&grille_cle(&self.cle.borrow()));
        c.append(&etiquette(
            &self.etat.selon(
                "Uniquement des lettres minuscules. Au démarrage, on la tape à la place du code.",
                "Uniquement des lettres minuscules. Au démarrage, on la tape à la place du mot de passe.",
            ),
            "discret",
        ));
        let note = gtk::CheckButton::with_label(&tr("Je l'ai écrite sur papier"));
        // Juste sous la clé, bien en vue (et non dans un coin)
        note.add_css_class("case-papier");
        note.set_halign(gtk::Align::Start);
        c.append(&note);
        let suivant = bouton(&tr("Continuer"), "principal");
        suivant.set_sensitive(false);
        let s2 = suivant.clone();
        note.connect_toggled(move |n| {
            s2.set_sensitive(n.is_active());
            // Case cochée : « Entrée » continue directement
            if n.is_active() {
                s2.grab_focus();
            }
        });
        let moi = Rc::downgrade(self);
        suivant.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.page_verification(None);
            }
        });
        self.montrer(&c, "cle-affichee", Some(3), Some(barre(None, &[suivant.upcast_ref()])));
    }

    fn page_verification(self: &Rc<Self>, erreur: Option<&str>) {
        let c = colonne();
        c.append(&etiquette(&tr("Vérifions votre papier"), "titre"));
        c.append(&etiquette(&tr("Recopiez le premier groupe de votre clé de secours (8 lettres)."), "sous-titre"));
        let saisie = gtk::Entry::builder().hexpand(true).max_length(9).build();
        c.append(&saisie);
        let message = etiquette(erreur.unwrap_or(""), "erreur");
        c.append(&message);
        let revoir = bouton(&tr("Revoir la clé"), "secondaire");
        let valider = bouton(&tr("Valider"), "principal");
        let moi = Rc::downgrade(self);
        revoir.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.page_cle_affichee();
            }
        });
        let verifier = {
            let moi = Rc::downgrade(self);
            let saisie = saisie.clone();
            move || {
                let Some(m) = moi.upgrade() else { return };
                let tape = saisie.text().trim().to_lowercase().replace([' ', '-'], "");
                let premier = m.cle.borrow().split('-').next().unwrap_or("").to_string();
                if !premier.is_empty() && tape == premier {
                    m.enregistrer();
                } else {
                    m.page_verification(Some(&tr("Ce n'est pas tout à fait ça. Vérifiez votre papier (ou revoyez la clé).")));
                }
            }
        };
        let v2 = verifier.clone();
        valider.connect_clicked(move |_| v2());
        saisie.connect_activate(move |_| verifier());
        self.montrer(&c, "verification", Some(3), Some(barre(Some(revoir.upcast_ref()), &[valider.upcast_ref()])));
        saisie.grab_focus();
    }

    // --- Enregistrement --------------------------------------------------------
    fn enregistrer(self: &Rc<Self>) {
        let c = colonne();
        let attente = gtk::Spinner::new();
        attente.set_spinning(true);
        attente.set_size_request(40, 40);
        c.append(&attente);
        c.append(&etiquette(&tr("Protection de vos fichiers…"), "sous-titre"));
        self.montrer(&c, "enregistrement", None, None);
        let code = format!("{}\n", self.code.borrow());
        let prenom = self.prenom.borrow().clone();
        let nouveau = self.nouveau_code;
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                if nouveau || prenom.is_empty() {
                    securite::appeler(&["definir"], &code)
                } else {
                    securite::appeler(&["definir", &prenom], &code)
                }
            },
            move |r| {
                let Some(m) = moi.upgrade() else { return };
                m.code.borrow_mut().clear();
                match r {
                    Ok(_) => m.page_fin(),
                    Err(e) => m.page_probleme(&e),
                }
            },
        );
    }

    fn page_fin(self: &Rc<Self>) {
        let c = colonne();
        if self.nouveau_code {
            c.append(&etiquette(&tr("Nouveau code enregistré"), "titre"));
            c.append(&etiquette(
                &self.etat.selon(
                    "Votre clé de secours reste la même : gardez votre papier. Au prochain démarrage, tapez votre nouveau code.",
                    "Votre clé de secours reste la même : gardez votre papier. Au prochain démarrage, tapez votre nouveau mot de passe.",
                ),
                "sous-titre",
            ));
        } else {
            let prenom = self.prenom.borrow().clone();
            c.append(&etiquette(&trv("C'est prêt, {prenom} !", &[("prenom", &prenom)]), "titre"));
            c.append(&etiquette(&tr("Vos fichiers sont protégés."), "sous-titre"));
            c.append(&etiquette(
                &self.etat.selon(
                    "Au démarrage, tapez votre code pour ouvrir SoleOS. Sans utilisation pendant 10 minutes, \
                     l'écran se verrouille (réglable dans Réglages).",
                    "Au démarrage, tapez votre mot de passe pour ouvrir SoleOS. Sans utilisation pendant 10 minutes, \
                     l'écran se verrouille (réglable dans Réglages).",
                ),
                "sous-titre",
            ));
        }
        let commencer = bouton(&tr("Commencer"), "principal");
        let f = self.fenetre.clone();
        commencer.connect_clicked(move |_| f.destroy());
        self.montrer(&c, "fin", None, Some(barre(None, &[commencer.upcast_ref()])));
    }

    fn page_probleme(self: &Rc<Self>, texte: &str) {
        let c = colonne();
        c.append(&etiquette(&tr("Un problème est survenu"), "titre"));
        c.append(&etiquette(texte, "sous-titre"));
        c.append(&etiquette(&tr("Rien n'a été perdu : vos fichiers restent accessibles comme avant."), "discret"));
        let reessayer = bouton(&tr("Réessayer"), "principal");
        let moi = Rc::downgrade(self);
        reessayer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                if m.nouveau_code { m.page_code(false, None) } else { m.page_prenom() }
            }
        });
        self.montrer(&c, "probleme", None, Some(barre(None, &[reessayer.upcast_ref()])));
    }
}

/// Clé de secours en grille : 2 colonnes de 4 groupes numérotés (tient sur tous les écrans)
fn grille_cle(cle: &str) -> gtk::Box {
    let cadre = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cadre.add_css_class("cle-grille");
    let grille = gtk::Grid::new();
    grille.set_column_spacing(28);
    grille.set_row_spacing(10);
    grille.set_column_homogeneous(true);
    let groupes: Vec<&str> = cle.split('-').filter(|g| !g.is_empty()).collect();
    for (i, g) in groupes.iter().enumerate() {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let n = gtk::Label::new(Some(&(i + 1).to_string()));
        n.add_css_class("cle-numero");
        n.set_width_chars(2);
        n.set_xalign(1.0);
        n.set_valign(gtk::Align::Baseline);
        let t = gtk::Label::new(Some(g));
        t.add_css_class("cle-groupe");
        t.set_selectable(false);
        t.set_xalign(0.0);
        b.append(&n);
        b.append(&t);
        // Ordre de lecture : 1 2 / 3 4 / 5 6 / 7 8
        grille.attach(&b, (i % 2) as i32, (i / 2) as i32, 1, 1);
    }
    cadre.append(&grille);
    cadre
}

/// Codes refusés : tous identiques ou qui se suivent
pub fn trop_simple(code: &str) -> bool {
    let c: Vec<u32> = code.chars().filter_map(|c| c.to_digit(10)).collect();
    if c.len() < 2 {
        return false;
    }
    let pareils = c.windows(2).all(|w| w[0] == w[1]);
    let monte = c.windows(2).all(|w| w[1] == (w[0] + 1) % 10);
    let descend = c.windows(2).all(|w| w[0] == (w[1] + 1) % 10);
    pareils || monte || descend
}

fn construire(app: &gtk::Application, nouveau_code: bool, etat: Etat) {
    let fenetre = gtk::ApplicationWindow::builder()
        .application(app)
        .title(tr("Bienvenue sur SoleOS"))
        .decorated(false)
        .default_width(800)
        .default_height(600)
        .build();
    fenetre.add_css_class("bienvenue");
    theme::adapter(&fenetre);
    // On ne ferme pas l'assistant par erreur (Alt + F4) : il faut aller au bout
    fenetre.connect_close_request(|_| glib::Propagation::Stop);
    let pages = gtk::Stack::new();
    pages.set_transition_type(gtk::StackTransitionType::SlideLeft);
    pages.set_transition_duration(260);
    fenetre.set_child(Some(&pages));
    let a = Rc::new(Assistant {
        fenetre: fenetre.clone(),
        pages,
        etat,
        nouveau_code,
        prenom: RefCell::new(String::new()),
        code: RefCell::new(String::new()),
        cle: RefCell::new(String::new()),
        clavier: RefCell::new(None),
    });
    let garde = RefCell::new(Some(a.clone()));
    fenetre.connect_destroy(move |_| {
        garde.borrow_mut().take();
    });
    // Plein écran, quel que soit l'écran, et redemandé tant qu'il n'est pas obtenu
    // (résolution, zoom ou écran qui changent ; demande ignorée au premier affichage)
    theme::garder_plein_ecran(&fenetre, true);
    // Banc de test : aller directement à une page (captures d'écran)
    match std::env::var("FLASHOS_TEST_PAGE").as_deref() {
        Ok("code") => a.page_code(false, Some(&tr("Les deux codes sont différents. Recommencez."))),
        Ok("cle") => {
            *a.cle.borrow_mut() = "rhutvtfe-bngclvei-rvhreiun-etiuehiv-inhiidfi-vlcrbcbk-uflvlkkj-hlcrthhd".into();
            a.page_cle_affichee()
        }
        Ok("verification") => a.page_verification(None),
        Ok("fin") => {
            *a.prenom.borrow_mut() = "Giuseppe".into();
            a.page_fin()
        }
        _ if nouveau_code => a.page_nouveau_code_intro(),
        _ => a.page_prenom(),
    }
    fenetre.present();
    theme::verifier_si_demande(&fenetre);
}

impl Assistant {
    fn page_nouveau_code_intro(self: &Rc<Self>) {
        let c = colonne();
        c.append(&etiquette(&tr("Vous avez ouvert SoleOS avec votre clé de secours"), "titre"));
        c.append(&etiquette(
            &self.etat.selon(
                "Choisissez maintenant un nouveau code : il remplacera celui que vous avez oublié.",
                "Choisissez maintenant un nouveau mot de passe : il remplacera celui que vous avez oublié.",
            ),
            "sous-titre",
        ));
        let continuer = bouton(&tr("Continuer"), "principal");
        let moi = Rc::downgrade(self);
        continuer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.page_code(false, None);
            }
        });
        self.montrer(&c, "intro", None, Some(barre(None, &[continuer.upcast_ref()])));
    }
}

/// Point d'entrée : `flashos-accueil --bienvenue` ou `--nouveau-code`
pub fn lancer(nouveau_code: bool) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(ID_BIENVENUE).build();
    app.connect_startup(|_| {
        let style = gtk::CssProvider::new();
        #[allow(deprecated)]
        style.load_from_data(&format!("{STYLE}\n{}", securite::STYLE_PAVE));
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&ecran, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    });
    app.connect_activate(move |app| {
        if let Some(f) = app.active_window() {
            f.present();
            return;
        }
        let mut etat = securite::etat();
        // Essai sans outil de sécurité (banc de test) : FLASHOS_TEST_TPM=0/1
        if let Ok(t) = std::env::var("FLASHOS_TEST_TPM") {
            etat.tpm = t == "1";
        }
        construire(app, nouveau_code, etat);
    });
    let nom = std::env::args().next().unwrap_or_else(|| "flashos-accueil".into());
    app.run_with_args(&[nom])
}
