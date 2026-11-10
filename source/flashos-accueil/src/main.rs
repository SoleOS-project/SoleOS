//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – écran d'accueil natif.
//!
//! - horloge, date et recherche web
//! - tuiles des applications : un clic ouvre l'appli, ou la ramène si elle
//!   est déjà ouverte (jamais deux fenêtres pour la même appli) ;
//!   un point lumineux sous la tuile indique qu'elle est ouverte
//! - bandeau « Applis ouvertes » : passer à une appli ou la fermer (✕)
//! - barre d'état : réseau, son, batterie, Réglages ⚙ (imprimantes, Bluetooth, écrans,
//!   taille d'affichage, code et verrouillage, console), éteindre / redémarrer
//! - premier démarrage : assistant de bienvenue (prénom, code, clé de secours)
//! - relancer `flashos-accueil` (touche Windows) ramène l'accueil devant

mod alttab;
mod appareils;
mod applis;
mod bienvenue;
mod bureau;
mod essai_son;
mod signaler;
mod raccourcis;
mod sauvegarde;
mod imprimante_manuelle;
mod fenetres;
mod fonds;
mod icones;
mod navigateur;
mod plasma;
mod portail;
mod reglages;
mod bureautique;
mod langue;
mod rail;
mod securite;
mod systeme;
mod theme;
mod tr;
mod wifi;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use fenetres::{Fenetre, Gestionnaire};
use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use systeme::Reseau;
use reglages::carte_tete;
use crate::tr::{tr, trv};

const ID_APPLI: &str = "fr.flashos.Accueil";

/// Vrai si la fenêtre est un écran d'accueil SoleOS.
fn est_accueil(f: &Fenetre) -> bool {
    f.app_id == ID_APPLI || f.app_id == "flashos-accueil"
}

/// Une application proposée sur l'accueil.
struct Appli {
    nom: &'static str,
    icone: &'static str,
    commande: &'static [&'static str],
    /// Morceaux d'identifiant permettant de reconnaître ses fenêtres.
    motifs: &'static [&'static str],
}

/// Mail : Thunderbird, installé d'office (v12.4). L'ancien choix de webmails reste dans le code,
/// désactivé.
const MAIL: &[&str] = &["thunderbird"];
const MAIL_AU_CHOIX: bool = false;

const APPLIS: &[Appli] = &[
    Appli {
        nom: "Bureautique",
        icone: "tuile-bureautique",
        commande: &["libreoffice", "--writer"],
        motifs: &["libreoffice", "soffice"],
    },
    // La commande de « Mail » dépend de la messagerie choisie (voir ouvrir())
    Appli { nom: "Mail", icone: "tuile-mail", commande: MAIL, motifs: &["thunderbird"] },
    Appli {
        nom: "Internet",
        icone: "tuile-internet",
        commande: &["flashos-web", "--new-window", "https://www.google.fr"],
        motifs: &["chromium", "chrome"],
    },
    Appli { nom: "Fichiers", icone: "tuile-fichiers", commande: &["nautilus", "--new-window"], motifs: &["nautilus"] },
    Appli {
        nom: "Applications",
        icone: "tuile-applications",
        commande: &["flashos-applications"],
        motifs: &["fr.flashos.applications", "flashos-applications"],
    },
];

/// Noms des tuiles, repérés pour la traduction (affichés avec `tr(appli.nom)`)
#[allow(dead_code)]
fn noms_a_traduire() -> [String; 6] {
    [tr("Bureautique"), tr("Mail"), tr("Internet"), tr("Fichiers"), tr("Applications"), tr("Console")]
}

const CONSOLE: Appli = Appli { nom: "Console", icone: "tuile-console", commande: &["flashos-console"], motifs: &["foot"] };

const STYLE: &str = r#"
window.accueil {
  font-family: "Inter", "Inter Variable", "DejaVu Sans", sans-serif;
  color: #f2f4f8;
}

.accueil label { color: #f2f4f8; }
.marque { font-size: 15px; letter-spacing: 1px; }
.marque-fort { font-weight: 700; }
.marque-leger { font-weight: 300; color: alpha(#f2f4f8, 0.7); }

.heure { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 112px; font-weight: 500; letter-spacing: -3px; font-feature-settings: "tnum"; }
.date { font-size: 19px; color: #A7B0C8; }
.salut { font-size: 22px; font-weight: 500; color: #DDE3F0; margin-bottom: -6px; }
.mini .salut { font-size: 17px; }
.compact .heure { font-size: 88px; }
.mini .heure { font-size: 64px; letter-spacing: -2px; }
.mini .date { font-size: 16px; }
.mini .aide { font-size: 0; min-height: 0; }

entry.recherche {
  min-height: 38px; padding: 8px 22px; border-radius: 16px; border: 1px solid #2A3352;
  box-shadow: none; outline: none; font-size: 17px;
  background: alpha(#141A2E, 0.92); color: #E9ECF5;
}
entry.recherche:focus-within { border-color: #FFC23D; }

button.tuile, button.tuile:hover, button.tuile:active, button.tuile:focus {
  background: none; border: none; box-shadow: none; outline: none;
  padding: 6px 10px; min-width: 150px;
}
button.tuile image.icone-tuile {
  border-radius: 30px;
  box-shadow: 0 10px 24px alpha(black, 0.30);
  transition: box-shadow 160ms ease, opacity 120ms ease;
}
button.tuile:hover image.icone-tuile {
  box-shadow: 0 0 0 2px #FFC23D, 0 12px 28px alpha(black, 0.40);
}
button.tuile:active image.icone-tuile { opacity: 0.75; }
button.tuile:focus-visible image.icone-tuile {
  box-shadow: 0 0 0 3px #FFC23D, 0 10px 24px alpha(black, 0.30);
}
.nom-appli { font-size: 15px; font-weight: 500; }
.compact button.tuile, .compact button.tuile:hover { min-width: 118px; padding: 4px 6px; }
.mini button.tuile, .mini button.tuile:hover { min-width: 92px; padding: 2px 4px; }
.mini .nom-appli { font-size: 13px; }
button.tuile.chargement image.icone-tuile { opacity: 0.55; }
spinner.roue { color: white; }
.point {
  min-width: 14px; min-height: 3px; border-radius: 2px;
  background-color: #FFC23D;
}

.titre-ouvertes { font-size: 13px; color: alpha(#f2f4f8, 0.65); margin-right: 4px; }
window.barre-taches { background-color: alpha(#0E1426, 0.97); background-image: none; border-top: 1px solid #1E2640; min-height: 52px; }
.barre-taches > centerbox { padding: 2px 8px; }
button.bouton-bureau { background: none; border: none; box-shadow: none; border-radius: 12px; padding: 6px 12px; }
button.bouton-bureau:hover { background: #1E2640; }
.barre-taches button.ouverte { min-width: 46px; min-height: 42px; border-radius: 12px; }
.barre-taches button.ouverte.devant { background: #1E2640; box-shadow: inset 0 -2px 0 #FFC23D; }
.heure-barre { font-size: 14px; font-weight: 600; }
.date-barre { font-size: 11px; color: #A7B0C8; }
.dock { background: alpha(#141A2E, 0.92); border: 1px solid #2A3352; border-radius: 18px; padding: 6px 8px; }
button.ouverte {
  background: none; border: none; box-shadow: none; outline: none;
  min-width: 54px; min-height: 54px; padding: 0; border-radius: 17px;
  transition: background 120ms ease;
}
button.ouverte:hover { background: #1E2640; }
button.ouverte:focus-visible { box-shadow: 0 0 0 2px #FFC23D; }
.fermer-mini {
  background: #d93025; color: white; border-radius: 999px; font-size: 10px; font-weight: 800;
  min-width: 19px; min-height: 19px; opacity: 0; transition: opacity 120ms ease;
}
.cadre-ouverte:hover .fermer-mini { opacity: 1; }
.fermer-mini:hover { background: #f04438; }
.compteur {
  background: #FFC23D; color: #0B2A5B; border-radius: 999px; font-size: 11px; font-weight: 700;
  min-width: 18px; min-height: 18px; box-shadow: 0 0 0 2px #0B1020;
}
.point-ouverte {
  min-width: 12px; min-height: 3px; border-radius: 2px; margin-top: 4px;
  background-color: #FFC23D;
}

button.etat {
  background: none; border: none; box-shadow: none; outline: none;
  min-width: 38px; min-height: 38px; padding: 0 8px; border-radius: 999px;
}
button.etat:hover { background: alpha(white, 0.10); }
button.etat:focus-visible { box-shadow: 0 0 0 2px alpha(white, 0.35); }
.icone-etat { opacity: 0.88; }
button.etat:hover .icone-etat { opacity: 1; }
.texte-etat { font-size: 15px; color: alpha(#f2f4f8, 0.85); }

popover > contents {
  background-color: #141A2E; color: #E9ECF5; border-radius: 18px; padding: 12px; font-size: 16px;
  box-shadow: 0 12px 34px alpha(black, 0.5); border: 1px solid #2A3352;
}
popover > arrow { background-color: #141A2E; border: none; }
popover label { color: #f2f4f8; }
popover .titre-menu { font-size: 14px; color: alpha(#f2f4f8, 0.6); margin: 4px 12px; }
popover button.menu {
  background: none; border: none; box-shadow: none; outline: none; color: #f2f4f8;
  padding: 10px 18px; border-radius: 10px; font-size: 16px;
}
popover button.menu:hover { background: alpha(white, 0.08); }
popover button.menu.danger:hover { background: alpha(#d93025, 0.8); }
popover .ligne-perso { padding: 4px 8px 4px 18px; }
popover .texte-menu { font-size: 16px; color: #f2f4f8; }
popover button.pas {
  background: alpha(white, 0.08); border: none; box-shadow: none; color: #f2f4f8;
  min-width: 34px; min-height: 34px; border-radius: 17px; font-size: 18px; padding: 0;
}
popover button.pas:hover { background: alpha(white, 0.16); }
.marque-version { font-size: 12px; color: alpha(#f2f4f8, 0.5); margin-left: 8px; }
popover scale { min-width: 220px; }
popover scale trough { background-color: alpha(white, 0.15); min-height: 6px; border-radius: 3px; }
popover scale highlight { background-color: #FFC23D; border-radius: 3px; }
popover scale slider { background-color: white; min-width: 18px; min-height: 18px; border-radius: 9px; border: none; }

popover button.choix-fond {
  background: none; border: none; box-shadow: none; outline: none; padding: 4px; border-radius: 12px;
}
popover button.choix-fond:hover { background: alpha(white, 0.08); }
popover .vignette { border-radius: 9px; }
popover button.choix-fond.actif .vignette { box-shadow: 0 0 0 3px #FFC23D; }
popover .vignette-nuit { background-image: radial-gradient(circle at 30% 20%, #26375f, #10172a 70%); }
popover .nom-vignette { font-size: 13px; color: alpha(#f2f4f8, 0.85); }
popover button.choix-fond.actif .nom-vignette { color: white; font-weight: 600; }

.aide { font-size: 14px; color: alpha(#f2f4f8, 0.6); }
.maj-prete {
  font-size: 14px; color: #FFE3A3; background: alpha(#FFC23D, 0.16);
  border-radius: 999px; padding: 6px 12px; margin-right: 6px;
}
popover button.menu.attention { color: #ffb38a; }
popover button.menu.attention:hover { background: alpha(#cf4a18, 0.55); color: white; }

.essai { font-size: 15px; color: alpha(#f2f4f8, 0.75); }
button.installer { padding: 12px 28px; font-size: 16px; }

popover .titre-panneau { font-size: 20px; font-weight: 600; }
popover .texte-info { font-size: 15px; color: alpha(#f2f4f8, 0.82); }
popover button.reseau-ligne {
  background: none; border: none; box-shadow: none; outline: none; color: #f2f4f8;
  padding: 10px 12px; border-radius: 10px; font-size: 16px;
}
popover button.reseau-ligne:hover { background: alpha(white, 0.08); }
popover button.reseau-actif { background: alpha(#FFC23D, 0.18); }
popover button.reseau-actif label { font-weight: 600; }
popover .zone-mdp { background: alpha(white, 0.05); border-radius: 12px; padding: 12px; }
popover entry, popover passwordentry, popover .champ {
  background: alpha(white, 0.08); color: #f2f4f8; border: none; box-shadow: none;
  border-radius: 10px; min-height: 36px; padding: 0 10px;
}
popover button.principal {
  background: #FFC23D; color: #0B2A5B; border: none; box-shadow: none;
  border-radius: 10px; padding: 8px 16px; font-weight: 600;
}
popover button.principal:hover { background: #FFD36B; }
popover button.petit {
  padding: 6px 12px; min-height: 0; border-radius: 9px; font-size: 15px;
  background: alpha(white, 0.10); color: #f2f4f8; border: none; box-shadow: none;
}
popover button.petit:hover { background: alpha(white, 0.18); }
popover button.petit.principal { background: #FFC23D; color: #0B2A5B; }
popover button.petit.principal:hover { background: #FFD36B; }
popover button.petit.danger:hover { background: alpha(#d93025, 0.8); }
popover .appareil-ligne { padding: 8px 10px; border-radius: 10px; }
popover .appareil-ligne:hover { background: alpha(white, 0.05); }
popover .appareil-actif { background: alpha(#FFC23D, 0.16); }
popover .nom-appareil { font-size: 16px; font-weight: 600; }
popover .onglets button { padding: 6px 14px; border-radius: 999px; background: none; border: none; box-shadow: none; color: alpha(#f2f4f8, 0.75); }
popover .onglets button:checked { background: alpha(#FFC23D, 0.28); color: white; }
popover .onglets button:hover { background: alpha(white, 0.08); }
popover switch { background: alpha(white, 0.2); border: none; border-radius: 999px; }
popover switch:checked { background: #FFC23D; }
popover switch slider { background: white; border: none; box-shadow: none; border-radius: 999px; min-width: 20px; min-height: 20px; }
popover scrollbar { background: none; }
popover button.reglage-ligne {
  background: none; border: none; box-shadow: none; outline: none; color: #f2f4f8;
  padding: 12px 12px; border-radius: 12px; font-size: 17px;
}
popover button.reglage-ligne:hover { background: alpha(white, 0.08); }
popover .fleche { font-size: 22px; color: alpha(#f2f4f8, 0.45); }
popover button.retour {
  background: none; border: none; box-shadow: none; color: #FFC23D; padding: 4px 8px; font-size: 15px;
}
popover button.retour:hover { background: alpha(white, 0.08); }
popover button.reglage-ligne.a-verifier .fleche { color: #ffb38a; }
popover button.reglage-ligne.a-verifier label { color: #ffd2b8; }
popover .attention-texte { color: #ffb38a; }
popover .cle-secours {
  font-family: "DejaVu Sans Mono", monospace; font-size: 17px; font-weight: 600;
  background: alpha(white, 0.08); border-radius: 10px; padding: 10px 14px;
}
"#;

/// Recopie les règles « popover … » en « .feuille … » (même apparence dans le panneau des Réglages)
/// Réglages SoleOS dans une fenêtre ordinaire (bureau Plasma, bouton « Réglages » de la barre
/// magique). Une seule fenêtre : un second lancement la ramène devant.
fn fenetre_reglages(page: Option<String>) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id("fr.flashos.Reglages").build();
    app.connect_startup(|_| {
        theme::charger();
        let style = gtk::CssProvider::new();
        let feuille = regles_pour_feuille(STYLE);
        #[allow(deprecated)]
        style.load_from_data(&format!("{STYLE}\n{feuille}\n{STYLE_V12}\n{}\n{}\n{STYLE_FENETRE_REGLAGES}", langue::STYLE, securite::STYLE_PAVE));
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&ecran, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    });
    app.connect_activate(move |app| {
        if let Some(f) = app.active_window() {
            f.present();
            return;
        }
        let fenetre = gtk::ApplicationWindow::builder()
            .application(app)
            .title(tr("Réglages"))
            .icon_name("flashos-reglages")
            .default_width(980)
            .default_height(760)
            .build();
        fenetre.add_css_class("fenetre-reglages");
    fenetre.add_css_class("flashos");
        let panneau = reglages::Reglages::nouveau();
        let f = fenetre.clone();
        *panneau.fermer_cb.borrow_mut() = Some(Box::new(move || f.close()));
        *panneau.changer_fond.borrow_mut() = Some(Box::new(|id: &str| {
            let image = if id == "perso" { fonds::perso() } else { fonds::image(id) };
            if let Some(chemin) = image {
                plasma::fond(&chemin);
            }
        }));
        *panneau.console.borrow_mut() = Some(Box::new(|| {
            if std::process::Command::new("flashos-console").spawn().is_err() {
                lancer(&["konsole"]);
            }
        }));
        panneau.au_debut();
        if let Some(p) = &page {
            panneau.ouvrir(p);
        }
        let carte = gtk::Box::new(gtk::Orientation::Vertical, 0);
        carte.add_css_class("feuille");
        carte.append(&panneau.racine);
        fenetre.set_child(Some(&carte));
        let touches = gtk::EventControllerKey::new();
        let fk = fenetre.clone();
        touches.connect_key_pressed(move |_, touche, _, _| {
            if touche == gdk::Key::Escape {
                fk.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        fenetre.add_controller(touches);
        // La fenêtre garde le panneau en vie jusqu'à sa fermeture
        let garde = RefCell::new(Some(panneau));
        fenetre.connect_close_request(move |_| {
            garde.borrow_mut().take();
            glib::Propagation::Proceed
        });
        fenetre.present();
    });
    let nom = std::env::args().next().unwrap_or_else(|| "flashos-accueil".into());
    app.run_with_args(&[nom])
}

/// Bureau Plasma (v13), lancé à l'ouverture de la session :
/// - clé USB : choix de la langue (premier démarrage), puis la fenêtre « Vous essayez SoleOS »
///   avec le bouton « Installer SoleOS sur cet ordinateur » ;
/// - outil de réparation : il s'ouvre tout seul ;
/// - après l'installation : assistant de bienvenue (prénom, code, clé de secours), et nouveau
///   code proposé quand l'ordinateur a été ouvert avec la clé de secours.
fn demarrage_plasma() -> glib::ExitCode {
    let cle = std::path::Path::new("/run/live/medium").exists();
    if cle && langue::a_choisir() {
        // Clé USB, premier écran : la langue et le clavier
        // (normalement fait avant l'ouverture du bureau, voir /etc/profile.d/zz-flashos.sh ;
        // ici en secours : le bureau prendra la langue à la prochaine ouverture de session)
        let _ = std::process::Command::new("flashos-accueil").arg("--langue").status();
        tr::initialiser();
    }
    let app = gtk::Application::builder().application_id("fr.flashos.Demarrage").build();
    app.connect_startup(|_| {
        theme::charger();
        let style = gtk::CssProvider::new();
        #[allow(deprecated)]
        style.load_from_data(&format!("{STYLE}\n{STYLE_V12}\n{STYLE_FENETRE_REGLAGES}\n{STYLE_ESSAI}"));
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&ecran, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    });
    app.connect_activate(move |app| {
        if app.windows().len() > 0 || app.is_busy() {
            return;
        }
        // Reste ouvert (sans fenêtre) pour surveiller la clé de secours
        let garde = app.hold();
        let reparation = mode_reparation();
        if reparation {
            glib::timeout_add_seconds_local_once(2, || lancer(&["flashos-installateur"]));
        } else if cle {
            fenetre_essai(app);
        } else {
            wifi::en_fond(securite::etat, move |e| {
                if e.methode == "provisoire" {
                    lancer(&["flashos-accueil", "--bienvenue"]);
                } else if e.secours_utilisee {
                    lancer(&["flashos-accueil", "--nouveau-code"]);
                }
            });
            // Mise à jour préparée : on prévient une fois (elle s'installe au prochain redémarrage)
            let app_maj = app.clone();
            let prevenu = Rc::new(Cell::new(false));
            glib::timeout_add_seconds_local(30, move || {
                if std::path::Path::new("/run/flashos/maj-prete").exists() && !prevenu.replace(true) {
                    let n = gtk::gio::Notification::new(&tr("↻ Mise à jour prête"));
                    n.set_body(Some(&tr("Elle s'installera toute seule au prochain redémarrage.")));
                    app_maj.send_notification(Some("maj"), &n);
                }
                glib::ControlFlow::Continue
            });
            let propose = Rc::new(Cell::new(false));
            // Code choisi pendant la session (assistant de bienvenue) : verrouillage automatique activé
            let verrouillable = Rc::new(Cell::new(std::path::Path::new("/run/flashos/verrouillable").exists()));
            glib::timeout_add_seconds_local(3, move || {
                let v = std::path::Path::new("/run/flashos/verrouillable").exists();
                if v && !verrouillable.get() {
                    plasma::delai_verrouillage(securite::delai_verrouillage());
                }
                verrouillable.set(v);
                let drapeau = std::path::Path::new("/run/flashos/secours-utilisee").exists();
                if drapeau && !propose.get() {
                    propose.set(true);
                    lancer(&["flashos-accueil", "--nouveau-code"]);
                } else if !drapeau {
                    propose.set(false);
                }
                glib::ControlFlow::Continue
            });
        }
        // La garde vit aussi longtemps que la session
        std::mem::forget(garde);
    });
    let nom = std::env::args().next().unwrap_or_else(|| "flashos-accueil".into());
    app.run_with_args(&[nom])
}

/// Clé USB : petite fenêtre « Vous essayez SoleOS », avec le bouton pour l'installer
fn fenetre_essai(app: &gtk::Application) {
    let fenetre = gtk::ApplicationWindow::builder()
        .application(app)
        .title(tr("Installer SoleOS sur cet ordinateur"))
        .icon_name("flashos-eclair")
        .resizable(false)
        .build();
    fenetre.add_css_class("fenetre-reglages");
    fenetre.add_css_class("flashos");
    let boite = gtk::Box::new(gtk::Orientation::Vertical, 18);
    boite.add_css_class("fenetre-essai");
    let logo = icones::image("logo", 140);
    let texte = gtk::Label::new(Some(&tr(
        "Vous essayez SoleOS depuis la clé USB : rien n'est modifié sur cet ordinateur.",
    )));
    texte.add_css_class("essai");
    texte.set_wrap(true);
    texte.set_max_width_chars(44);
    texte.set_justify(gtk::Justification::Center);
    let installer = gtk::Button::with_label(&tr("Installer SoleOS sur cet ordinateur"));
    installer.add_css_class("installer");
    installer.add_css_class("principal");
    installer.set_halign(gtk::Align::Center);
    let f = fenetre.clone();
    installer.connect_clicked(move |_| {
        lancer(&["flashos-installateur"]);
        f.close();
    });
    let plus_tard = gtk::Button::with_label(&tr("Continuer l'essai"));
    plus_tard.add_css_class("secondaire");
    plus_tard.set_halign(gtk::Align::Center);
    let f = fenetre.clone();
    plus_tard.connect_clicked(move |_| f.close());
    boite.append(&logo);
    boite.append(&texte);
    boite.append(&installer);
    boite.append(&plus_tard);
    fenetre.set_child(Some(&boite));
    fenetre.present();
}

const STYLE_ESSAI: &str = r#"
.fenetre-essai { padding: 30px 40px 26px 40px; }
.fenetre-essai label.essai { font-size: 17px; color: #f2f4f8; }
"#;

/// Fenêtre des Réglages (bureau Plasma) : fond sombre SoleOS, panneau sur toute la largeur
const STYLE_FENETRE_REGLAGES: &str = r#"
window.fenetre-reglages { background-color: #0E1426; color: #f2f4f8; }
window.fenetre-reglages .feuille { margin: 0; border: none; border-radius: 0; box-shadow: none; background-color: #0E1426; padding: 22px 28px; }
"#;

fn regles_pour_feuille(css: &str) -> String {
    let mut sortie = String::new();
    for bloc in css.split('}') {
        let Some((selecteurs, declarations)) = bloc.split_once('{') else { continue };
        let choisis: Vec<String> = selecteurs
            .split(',')
            .map(|x| x.trim())
            .filter(|x| x.starts_with("popover ") && !x.starts_with("popover >"))
            .map(|x| x.replacen("popover ", ".feuille ", 1))
            .collect();
        if !choisis.is_empty() {
            sortie.push_str(&format!("{} {{{}}}\n", choisis.join(", "), declarations));
        }
    }
    sortie
}

/// Bureau v12 : rail des applis ouvertes, panneau des Réglages, icône du bureau
const STYLE_V12: &str = r#"
.accueil label.pastille-version { font-family: "Space Grotesk", "Inter", sans-serif; font-weight: 500; font-size: 12px; color: #FFC23D;
  border: 1.5px solid alpha(#FFC23D, 0.9); background-color: alpha(#FFC23D, 0.11); border-radius: 999px; padding: 1px 10px; }

.choix-bureautique { padding: 10px 8px 6px 8px; min-width: 600px; }
.titre-bureautique { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 22px; font-weight: 600; color: #f2f4f8; margin: 0 12px; }
button.carte-doc {
  background: alpha(white, 0.05); border: 1px solid #2A3352; box-shadow: none; border-radius: 16px;
  padding: 14px 8px 12px 8px; color: #f2f4f8;
}
button.carte-doc:hover { background: alpha(white, 0.10); border-color: #FFC23D; }
button.carte-doc:focus-visible { box-shadow: 0 0 0 2px #FFC23D; }
.nom-doc { font-size: 16px; font-weight: 600; color: #f2f4f8; }
.detail-doc { font-size: 12px; color: #A7B0C8; }

window.rail-fenetre { background: none; background-color: transparent; }
.rail {
  background-color: alpha(#141A2E, 0.94); border: 1px solid #2A3352; border-radius: 26px;
  padding: 10px 8px; box-shadow: 0 18px 40px alpha(black, 0.45);
}
button.bouton-bureau {
  background: #FFC23D; border: none; box-shadow: none; border-radius: 16px;
  min-width: 48px; min-height: 48px; padding: 0;
}
button.bouton-bureau:hover { background: #FFD36B; }
.separateur-rail { background: #2A3352; min-height: 1px; margin: 4px 10px; }
.rail button.ouverte { min-width: 48px; min-height: 48px; border-radius: 16px; background: none; margin: 2px 0; }
.rail button.ouverte:hover { background: #1E2640; }
.rail button.ouverte.devant { background: #1E2640; box-shadow: inset 4px 0 0 #FFC23D; }
.rail .epinglee-ouverte button.ouverte { box-shadow: inset 3px 0 0 alpha(#FFC23D, 0.6); }
.rail .epinglee-ouverte button.ouverte.devant { box-shadow: inset 4px 0 0 #FFC23D; }
.rail .point-ouverte { min-height: 0; margin: 0; }
.rail .compteur { font-size: 10px; min-width: 16px; min-height: 16px; }
.rail.droite button.ouverte.devant { box-shadow: inset -4px 0 0 #FFC23D; }
.rail.bas button.ouverte.devant { box-shadow: inset 0 -4px 0 #FFC23D; }
.rail.bas { padding: 8px 10px; }
.rail.bas .separateur-rail { min-height: 0; min-width: 1px; margin: 10px 4px; }
.rail.bas button.ouverte { margin: 0 2px; }
.rail-bord { background-color: alpha(#FFC23D, 0.55); border-radius: 3px; margin: 2px; }
.texte-anneau { font-size: 11px; font-weight: 700; color: #E9ECF5; }
.heure-rail { font-size: 13px; font-weight: 600; color: #C9D0E2; margin: 2px 0 4px 0; }
.rail button.etat { min-width: 44px; min-height: 44px; border-radius: 14px; }
window.voile { background-color: alpha(#04060D, 0.62); background-image: none; }
.feuille {
  background-color: #0E1426; border: 1px solid #2A3352; border-radius: 28px;
  padding: 26px 28px; box-shadow: 0 30px 80px alpha(black, 0.6);
}
.feuille .titre-panneau { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 30px; font-weight: 700; }
button.fermer-panneau {
  background: #141A2E; border: 1px solid #2A3352; border-radius: 20px; min-width: 40px; min-height: 40px;
  padding: 0; color: #E9ECF5; box-shadow: none;
}
button.fermer-panneau:hover { background: #1E2640; }
.carte-tete { background: #141A2E; border: 1px solid #2A3352; border-radius: 20px; padding: 18px; }
.titre-carte { font-size: 17px; font-weight: 600; }
.carte-tete scale trough { background-color: #2A3352; min-height: 8px; border-radius: 4px; border: none; }
.carte-tete scale highlight { background-color: #FFC23D; border-radius: 4px; }
.carte-tete scale slider { background-color: #E9ECF5; min-width: 20px; min-height: 20px; border-radius: 10px; border: none; box-shadow: none; }
.carte-tete switch { background: #2A3352; border: none; border-radius: 999px; }
.carte-tete switch:checked { background: #FFC23D; }
.carte-tete switch slider { background: #E9ECF5; border: none; box-shadow: none; border-radius: 999px; min-width: 22px; min-height: 22px; }
.carte-tete switch:checked slider { background: #0B2A5B; }
button.carte-reglage {
  background: #141A2E; border: 1px solid #2A3352; border-radius: 18px; padding: 14px 16px;
  box-shadow: none; min-height: 48px;
}
button.carte-reglage:hover { background: #1A2138; border-color: #3A4570; }
button.carte-reglage.carte-danger { background: alpha(#DC2626, 0.14); border-color: alpha(#DC2626, 0.55); }
button.carte-reglage.carte-danger:hover { background: alpha(#DC2626, 0.24); border-color: #DC2626; }
button.carte-reglage.carte-danger .nom-reglage { color: #FECACA; }
.nom-reglage { font-size: 15px; font-weight: 600; color: #E9ECF5; }
.feuille flowboxchild { padding: 0; }
button.icone-reglages-bureau, button.icone-reglages-bureau:hover, button.icone-reglages-bureau:active { background: none; border: none; box-shadow: none; padding: 4px; }
button.icone-reglages-bureau:hover image.icone-bureau { box-shadow: 0 0 0 3px alpha(#FFC23D, 0.5), 0 10px 26px alpha(#FFC23D, 0.3); }
image.icone-bureau { border-radius: 22px; box-shadow: 0 10px 26px alpha(#FFC23D, 0.25); }
"#;

/// Les éléments de la barre d'état mis à jour régulièrement.
struct Barre {
    /// Réglages : état du réseau et du son en toutes lettres, interrupteur du Wi-Fi
    info_reseau: gtk::Label,
    info_son: gtk::Label,
    interrupteur_wifi: gtk::Switch,
    /// Rail : anneau de la batterie (niveau, en charge) et heure
    niveau_batterie: Rc<Cell<(u32, bool)>>,
    anneau: gtk::DrawingArea,
    texte_anneau: gtk::Label,
    reseau: gtk::Image,
    bouton_reseau: gtk::Button,
    bouton_son: gtk::Widget,
    son: gtk::Image,
    curseur_son: gtk::Scale,
    bouton_muet: gtk::Button,
    maj_en_cours: Cell<bool>,
    batterie: gtk::Box,
    image_batterie: gtk::Image,
    texte_batterie: gtk::Label,
    maj_prete: gtk::Label,
    /// Barre du bas (v12.5) : Wi-Fi et son, avec leurs bulles
    reseau_bas: gtk::Image,
    bouton_reseau_bas: gtk::Button,
    son_bas: gtk::Image,
    bouton_son_bas: gtk::Button,
    curseur_bas: gtk::Scale,
    muet_bas: gtk::Button,
}

/// L'état partagé de l'écran d'accueil.
struct Accueil {
    fenetre: gtk::ApplicationWindow,
    contenu: gtk::Box,
    bandeau: gtk::Box,
    /// Barre des tâches (en bas, toujours visible) ; None si le gestionnaire d'affichage ne la permet pas
    barre_taches: Option<gtk::Window>,
    /// Rail (masquage automatique, côté de l'écran)
    rail: Option<Rc<rail::Rail>>,
    /// Alt + Tab avec les icônes des applications
    selecteur: Option<Rc<alttab::Selecteur>>,
    /// Accueil affiché par-dessus les fenêtres (éclair du rail, touche ⊞) ; numéro d'activation
    /// de la dernière fenêtre au moment de l'affichage (une fenêtre activée ensuite le referme)
    devant: Cell<Option<u64>>,
    /// Recherche sur le web (prend le clavier quand l'accueil passe devant)
    recherche: gtk::Entry,
    /// Fenêtres réduites par « afficher le bureau » (remises au deuxième appui)
    bureau_montre: RefCell<Vec<u32>>,
    /// Référence faible vers soi-même (pour les actions demandées de l'extérieur)
    moi: RefCell<std::rc::Weak<Accueil>>,
    points: Vec<(gtk::Box, &'static Appli)>,
    /// Tuiles : (appli, bouton, roue de chargement, nom affiché)
    tuiles: Vec<(&'static Appli, gtk::Button, gtk::Spinner, gtk::Label)>,
    /// Applis en cours d'ouverture (nom, moment du clic)
    en_cours: RefCell<Vec<(&'static str, std::time::Instant)>>,
    barre: Barre,
    gestionnaire: Option<Gestionnaire>,
    affichees: RefCell<Vec<Fenetre>>,
    /// Dernier état connu du chargeur (pour la luminosité sur batterie)
    sur_secteur: Cell<Option<bool>>,
    /// Bureautique ouverte à l'avance, cachée (fenêtre réduite, absente du bandeau) : s'affiche aussitôt au clic
    prechargee: Cell<Option<(u32, u64)>>,
    /// Moment où la Bureautique a été lancée en arrière-plan (en attente de sa fenêtre)
    prechargement: Cell<Option<std::time::Instant>>,
    /// Bulles de la barre du haut, ouvrables à la demande (« flashos-ouvrir »)
    bulles: Vec<(&'static str, gtk::Popover)>,
    /// Réglages (bouton ⚙) : imprimantes, Bluetooth, écran, taille, code
    reglages: Rc<reglages::Reglages>,
    bulle_reglages: gtk::Popover,
    /// Réglages en grand panneau au centre de l'écran (bureau SoleOS v12)
    feuille: Option<gtk::Window>,
    /// Assistant « nouveau code » déjà proposé (après ouverture avec la clé de secours)
    nouveau_code_propose: Cell<bool>,
    /// Choix de la messagerie à ouvrir (quand il y en a plusieurs)
    choix_mail: gtk::Popover,
    /// Choix de la Bureautique : Texte, Tableur, Présentation, PDF, documents récents
    choix_bureautique: gtk::Popover,
    /// Fenêtres déjà ouvertes au moment du clic sur « Mail » (la messagerie s'ouvre dans le navigateur :
    /// c'est l'apparition d'une nouvelle fenêtre qui arrête « Ouverture… »)
    fenetres_avant: RefCell<Vec<u32>>,
    /// Bureau v12.5 : boutons des fenêtres de la barre du bas, panneau de l'éclair, barre magique
    taches: Option<gtk::Box>,
    taches_construites: Cell<bool>,
    panneau_eclair: Option<Rc<bureau::PanneauEclair>>,
    boutons_magique: Vec<(&'static Appli, gtk::Button)>,
    /// « À droite : » proposé après « À gauche »
    bulle_droite: RefCell<Option<(gtk::Popover, u32)>>,
}

impl Accueil {
    /// Ouvre l'appli, ou la ramène au premier plan si elle tourne déjà.
    fn ouvrir(self: &Rc<Self>, appli: &'static Appli) {
        if appli.nom == "Bureautique" && self.choix_bureautique.parent().is_some() {
            // Texte, Tableur, Présentation, PDF ou un document récent
            self.choix_bureautique.popup();
            return;
        }
        if let Some(g) = &self.gestionnaire {
            if let Some(f) = g.liste().iter().find(|f| correspond(appli, f)) {
                self.retirer_devant();
                g.activer(f.id);
                return;
            }
        }
        // Déjà en train de s'ouvrir : on ignore les clics répétés.
        if self.en_cours.borrow().iter().any(|(n, t)| *n == appli.nom && t.elapsed().as_secs() < 25) {
            return;
        }
        // Mail : Thunderbird (v12.4 ; il trouve seul les réglages de Gmail, Outlook…)
        if appli.nom == "Mail" {
            lancer(MAIL);
        } else if appli.nom == "Internet" {
            // Page d'accueil selon la langue (Bing en chinois)
            lancer(&["flashos-web", "--new-window", tr::courante().web]);
        } else {
            lancer(appli.commande);
        }
        self.commencer_ouverture(appli.nom);
    }

    /// « Ouverture… » sur la tuile jusqu'à l'apparition de la fenêtre (25 s au plus)
    fn commencer_ouverture(&self, nom: &'static str) {
        self.retirer_devant();
        self.en_cours.borrow_mut().retain(|(n, _)| *n != nom);
        self.en_cours.borrow_mut().push((nom, std::time::Instant::now()));
        if let Some(g) = &self.gestionnaire {
            *self.fenetres_avant.borrow_mut() = g.liste().iter().map(|f| f.id).collect();
        }
        self.afficher_chargement(nom, true);
        // Sécurité : au bout de 25 s, on rend la main même si rien n'est apparu.
        for (appli, bouton, roue, etiquette) in &self.tuiles {
            if appli.nom == nom {
                let (b, r, e, n) = (bouton.clone(), roue.clone(), etiquette.clone(), appli.nom);
                glib::timeout_add_seconds_local_once(25, move || {
                    if e.label() == tr("Ouverture…") {
                        r.set_visible(false);
                        r.set_spinning(false);
                        e.set_label(&tr(n));
                        b.remove_css_class("chargement");
                    }
                });
            }
        }
    }

    /// Affiche ou masque l'animation « Ouverture… » sur une tuile.
    fn afficher_chargement(&self, nom: &str, oui: bool) {
        for (appli, bouton, roue, etiquette) in &self.tuiles {
            if appli.nom == nom {
                roue.set_visible(oui);
                roue.set_spinning(oui);
                etiquette.set_label(&if oui { tr("Ouverture…") } else { tr(appli.nom) });
                if oui { bouton.add_css_class("chargement") } else { bouton.remove_css_class("chargement") }
            }
        }
    }

    /// Remet l'accueil devant toutes les autres fenêtres.
    /// Ouvre une bulle demandée par un autre programme : fichier
    /// $XDG_RUNTIME_DIR/flashos-ouvrir contenant « wifi », « appareils », « appareils/bluetooth »…
    /// Renvoie vrai si une application a été ouverte ou ramenée devant (l'accueil reste alors derrière).
    fn ouvrir_demande(&self) -> bool {
        let Some(dossier) = std::env::var_os("XDG_RUNTIME_DIR") else { return false };
        let fichier = PathBuf::from(dossier).join("flashos-ouvrir");
        let Ok(demande) = std::fs::read_to_string(&fichier) else { return false };
        let _ = std::fs::remove_file(&fichier);
        // « appli:<identifiant> » : application ajoutée (boutique, AppImage)
        if let Some(id) = demande.trim().strip_prefix("appli:") {
            if let Some(a) = applis::installees().into_iter().find(|a| a.id == id) {
                self.ouvrir_installee(&a);
                return true;
            }
            return false;
        }
        // « mail:<adresse> » : messagerie choisie dans la fenêtre « Messagerie »
        if let Some(adresse) = demande.trim().strip_prefix("mail:") {
            if let Some(cmd) = applis::commande_messagerie(adresse) {
                let cmd: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
                lancer(&cmd);
                self.commencer_ouverture("Mail");
            }
            return false;
        }
        // « gauche:<app_id> » / « droite:<app_id> » : place une appli ouverte sur une moitié de l'écran
        if let Some((cote, app)) = demande.trim().split_once(':').filter(|(c, _)| *c == "gauche" || *c == "droite") {
            if let Some(f) = self.gestionnaire.as_ref().and_then(|g| g.liste().into_iter().find(|f| f.app_id == app)) {
                if let Some(m) = self.moi.borrow().upgrade() {
                    m.placer(f.id, if cote == "gauche" { "Left" } else { "Right" });
                }
            }
            return true;
        }
        // « alttab » / « alttab-arriere » : passage d'une application à l'autre
        if matches!(demande.trim(), "alttab" | "alttab-arriere") {
            let sens = if demande.trim() == "alttab" { 1 } else { -1 };
            if let (Some(s), Some(g)) = (&self.selecteur, &self.gestionnaire) {
                let mut liste: Vec<Fenetre> = g.liste().into_iter().filter(|f| !est_accueil(f)).collect();
                if let Some((id, apparition)) = self.prechargee.get() {
                    liste.retain(|f| f.id != id || f.activation > apparition);
                }
                // De la plus récemment utilisée à la plus ancienne
                liste.sort_by(|a, b| b.activation.cmp(&a.activation));
                let installees = applis::installees();
                let fenetres = liste
                    .iter()
                    .map(|f| {
                        (f.id, titre_lisible(f), icone_fenetre(f, &installees))
                    })
                    .collect();
                s.alt_tab(fenetres, sens);
            }
            return true;
        }
        // « capture » : capture d'écran faite (touche Impr. écran) : confirmation à l'écran
        if demande.trim() == "capture" {
            if let Some(app) = self.fenetre.application() {
                bureau::bulle_info(&app, &tr("Capture d'écran enregistrée dans Images"));
            }
            return true;
        }
        // « bureau » : afficher le bureau (comme la touche ⊞)
        if demande.trim() == "bureau" {
            if self.panneau_eclair.is_some() {
                if let Some(m) = self.moi.borrow().upgrade() {
                    m.basculer_eclair();
                }
                return true;
            }
            self.basculer_devant();
            return true;
        }
        // « fond » : fond d'écran changé par une autre application (portail)
        if demande.trim() == "fond" {
            fonds::appliquer(&fonds::choisi());
            return false;
        }
        // « appareils/bluetooth » (ancienne forme) = « bluetooth »
        let nom = demande.trim().rsplit('/').next().unwrap_or("");
        let nom = if nom == "appareils" { "imprimantes" } else { nom };
        // Bureau v12.5 : le Wi-Fi s'ouvre dans sa bulle, au-dessus de la barre du bas
        if nom == "wifi" && self.taches.is_some() {
            // La bulle du Wi-Fi passe devant : la feuille des Réglages se ferme
            if let Some(f) = &self.feuille {
                f.set_visible(false);
            }
            if let Some((_, bulle)) = self.bulles.iter().find(|(n, _)| *n == "wifi") {
                bulle.popup();
            }
            return false;
        }
        // Pages des Réglages
        if let Some(feuille) = &self.feuille {
            if matches!(nom, "reglages" | "imprimantes" | "bluetooth" | "ecran" | "taille" | "securite" | "navigateur" | "fond" | "batterie" | "apropos" | "heure" | "wifi" | "langue" | "rail" | "camera" | "signaler" | "sauvegarde") {
                // Les bulles du bas (Wi-Fi…) se ferment : la page passe seule devant
                for (_, b) in &self.bulles {
                    b.popdown();
                }
                self.reglages.au_debut();
                feuille.present();
                if nom != "reglages" {
                    self.reglages.ouvrir(nom);
                }
                return false;
            }
        }
        if matches!(nom, "reglages" | "imprimantes" | "bluetooth" | "ecran" | "taille" | "securite" | "navigateur" | "fond" | "batterie" | "apropos" | "heure" | "langue") {
            if self.bulle_reglages.is_visible() {
                self.bulle_reglages.popdown();
            }
            self.bulle_reglages.popup();
            if nom != "reglages" {
                self.reglages.ouvrir(nom);
            }
            return false;
        }
        if let Some((_, bulle)) = self.bulles.iter().find(|(n, _)| *n == nom) {
            if bulle.is_visible() {
                bulle.popdown();
            }
            bulle.popup();
        }
        false
    }

    /// Application ajoutée : déjà ouverte → on la ramène devant ; sinon on la lance.
    fn ouvrir_installee(&self, a: &applis::Installee) {
        if let Some(g) = &self.gestionnaire {
            let motifs = a.motifs();
            if let Some(f) = g.liste().iter().find(|f| {
                let id = f.app_id.to_lowercase();
                !est_accueil(f) && !id.is_empty() && motifs.iter().any(|m| id.contains(m.as_str()) || m.contains(id.as_str()))
            }) {
                g.activer(f.id);
                return;
            }
        }
        // Roue de chargement jusqu'à l'apparition de sa fenêtre (25 s au plus)
        roue_epinglee(&a.id, true);
        let (id, motifs) = (a.id.clone(), a.motifs());
        CHARGEMENTS.with(|c| {
            let mut c = c.borrow_mut();
            c.retain(|(i, _, _)| i != &id);
            c.push((id, motifs, std::time::Instant::now()));
        });
        let id2 = a.id.clone();
        glib::timeout_add_seconds_local_once(25, move || roue_epinglee(&id2, false));
        lancer_installee(a);
    }

    fn revenir(&self) {
        // Demande transmise (page des Réglages, Wi-Fi…) : en mode bureau, elle ferme l'éclair
        // s'il est ouvert, mais ne le rouvre jamais par-dessus la page demandée.
        let demande = std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("flashos-ouvrir").exists()).unwrap_or(false);
        if self.ouvrir_demande() {
            return;
        }
        if demande && self.panneau_eclair.is_some() {
            self.fermer_eclair();
            return;
        }
        // Bureau v12.5 : touche ⊞ = panneau de l'éclair (et retour)
        if self.panneau_eclair.is_some() {
            if let Some(m) = self.moi.borrow().upgrade() {
                m.basculer_eclair();
            }
            return;
        }
        // Accueil en fond d'écran (rail) : touche ⊞ = accueil par-dessus les fenêtres (et retour)
        if self.barre_taches.is_some() {
            self.basculer_devant();
            return;
        }
        fondu(&self.contenu, 0.4);
        if let Some(g) = &self.gestionnaire {
            if let Some(f) = g.liste().iter().find(|f| est_accueil(f)) {
                g.activer(f.id);
                return;
            }
        }
        self.fenetre.present();
    }

    /// Place une fenêtre : moitié gauche (« Left »), moitié droite (« Right ») ou tout l'écran (« Up »).
    /// La fenêtre passe devant, puis le raccourci du gestionnaire d'affichage (⊞ + flèche) est envoyé.
    /// Après « à gauche », les autres applis ouvertes sont proposées pour la moitié droite.
    fn placer(self: &Rc<Self>, id: u32, touche: &'static str) {
        let Some(gest) = self.gestionnaire.clone() else { return };
        gest.activer(id);
        // Tout l'écran : demandé directement à la fenêtre (le raccourci ⊞ + ↑ ne fait que la moitié
        // du haut sur certaines versions du gestionnaire d'affichage)
        if touche == "Up" {
            gest.agrandir(id);
            return;
        }
        // Le raccourci agit sur la fenêtre active : on attend qu'elle le soit vraiment (3 s au plus)
        let mut essais = 0;
        glib::timeout_add_local(Duration::from_millis(100), move || {
            essais += 1;
            let active = gest.liste().iter().any(|f| f.id == id && f.active);
            if active || essais >= 30 {
                if active {
                    lancer(&["wtype", "-M", "logo", "-k", touche, "-m", "logo"]);
                }
                return glib::ControlFlow::Break;
            }
            if essais % 10 == 0 {
                gest.activer(id);
            }
            glib::ControlFlow::Continue
        });
        if touche == "Left" {
            let moi = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                if let Some(m) = moi.upgrade() {
                    m.proposer_moitie_droite(id);
                }
            });
        }
    }

    /// Après avoir mis une appli à gauche : choisir celle qui ira à droite
    fn proposer_moitie_droite(self: &Rc<Self>, deja: u32) {
        let Some(gest) = self.gestionnaire.clone() else { return };
        let autres: Vec<Fenetre> = gest.liste().into_iter().filter(|f| f.id != deja && !est_accueil(f)).collect();
        if autres.is_empty() {
            return;
        }
        let installees = applis::installees();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let titre = gtk::Label::new(Some(&tr("À droite :")));
        titre.add_css_class("titre-menu");
        titre.set_xalign(0.0);
        menu.append(&titre);
        let bulle = gtk::Popover::new();
        bulle.set_child(Some(&menu));
        match &self.taches {
            Some(t) => bulle.set_parent(t),
            None => bulle.set_parent(&self.bandeau),
        }
        bulle.set_position(gtk::PositionType::Top);
        bulle.set_autohide(true);
        for f in autres {
            let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let image = icone_fenetre(&f, &installees);
            image.set_pixel_size(24);
            ligne.append(&image);
            let nom = gtk::Label::new(Some(&nom_affiche(&f)));
            nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
            nom.set_max_width_chars(36);
            ligne.append(&nom);
            let b = gtk::Button::builder().child(&ligne).build();
            b.add_css_class("menu");
            let (moi, bl, id) = (Rc::downgrade(self), bulle.clone(), f.id);
            b.connect_clicked(move |_| {
                bl.popdown();
                if let Some(m) = moi.upgrade() {
                    m.placer(id, "Right");
                }
            });
            menu.append(&b);
        }
        bulle.connect_closed(|b| {
            let b = b.clone();
            glib::idle_add_local_once(move || b.unparent());
        });
        // Ouverte sans clic (après « À gauche ») : le gestionnaire d'affichage ne la referme pas
        // tout seul au clic suivant ; elle se ferme donc d'elle-même au bout de 12 s, ou dès
        // qu'une autre fenêtre est choisie
        if let Some((avant, _)) = self.bulle_droite.replace(Some((bulle.clone(), deja))) {
            avant.popdown();
        }
        let b = bulle.downgrade();
        glib::timeout_add_seconds_local_once(12, move || {
            if let Some(b) = b.upgrade() {
                b.popdown();
            }
        });
        bulle.popup();
    }

    /// Accueil par-dessus les fenêtres, ou retour aux fenêtres (éclair du rail, touche ⊞)
    fn basculer_devant(&self) {
        if self.devant.get().is_some() {
            self.retirer_devant();
        } else {
            self.montrer_devant();
        }
    }

    /// L'accueil (heure, recherche, tuiles, applis épinglées) passe devant les fenêtres, sans les
    /// déplacer ni les réduire. Échap, l'éclair, ⊞ ou l'ouverture d'une appli le remettent derrière.
    fn montrer_devant(&self) {
        if self.barre_taches.is_none() {
            return;
        }
        let derniere = self.gestionnaire.as_ref().map(|g| g.liste().iter().map(|f| f.activation).max().unwrap_or(0)).unwrap_or(0);
        self.devant.set(Some(derniere));
        // Clavier pour l'accueil : on peut taper sa recherche tout de suite. La surface est
        // remontrée pour que le gestionnaire d'affichage lui donne le clavier.
        self.fenetre.set_visible(false);
        self.fenetre.set_layer(Layer::Top);
        self.fenetre.set_keyboard_mode(KeyboardMode::Exclusive);
        self.fenetre.set_visible(true);
        self.recherche.grab_focus();
        self.fenetre.add_css_class("devant");
        fondu(&self.contenu, 0.3);
        if let Some(r) = &self.rail {
            r.etat_fenetres(false, false);
        }
    }

    fn retirer_devant(&self) {
        if self.devant.take().is_none() {
            return;
        }
        self.choix_bureautique.popdown();
        self.choix_mail.popdown();
        self.recherche.set_text("");
        self.fenetre.set_keyboard_mode(KeyboardMode::OnDemand);
        self.fenetre.set_layer(Layer::Bottom);
        self.fenetre.remove_css_class("devant");
        // Le rail retrouve l'état des fenêtres (replié si une appli occupe l'écran)
        if let (Some(r), Some(g)) = (&self.rail, &self.gestionnaire) {
            let liste = g.liste();
            let devant = || liste.iter().filter(|f| f.active && !f.reduite && !est_accueil(f));
            r.etat_fenetres(devant().any(|f| f.agrandie || f.plein_ecran), devant().any(|f| f.plein_ecran));
        }
    }

    /// Clic sur une appli épinglée du rail qui n'est pas ouverte
    fn lancer_depuis_rail(self: &Rc<Self>, appli: &'static Appli) {
        let choix = appli.nom == "Bureautique" && self.choix_bureautique.parent().is_some();
        if choix {
            // Texte / Tableur / Présentation, ou quelle messagerie : choix sur l'accueil, devant
            self.montrer_devant();
            let bulle = if appli.nom == "Mail" { self.choix_mail.clone() } else { self.choix_bureautique.clone() };
            bulle.set_position(gtk::PositionType::Top);
            // Un peu vers la droite : la bulle de la première tuile ne passe pas sous le rail
            if appli.nom == "Bureautique" {
                bulle.set_offset(56, 0);
            }
            glib::timeout_add_local_once(Duration::from_millis(150), move || bulle.popup());
            return;
        }
        self.ouvrir(appli);
    }

    /// Une appli épinglée (Bureautique, Mail, Internet, Fichiers, Applications) pas encore ouverte
    fn icone_epinglee(self: &Rc<Self>, appli: &'static Appli) -> gtk::Box {
        let image = icones::image(appli.icone, 32);
        let bouton = gtk::Button::builder().child(&image).build();
        bouton.add_css_class("ouverte");
        bouton.add_css_class("epinglee");
        bouton.set_tooltip_text(Some(&tr(appli.nom)));
        bouton.set_margin_top(7);
        bouton.set_margin_end(7);
        bouton.set_margin_start(7);
        let moi = Rc::downgrade(self);
        bouton.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.lancer_depuis_rail(appli);
            }
        });
        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 0);
        colonne.append(&bouton);
        let point = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        point.add_css_class("point-ouverte");
        point.set_halign(gtk::Align::Center);
        point.set_opacity(0.0);
        colonne.append(&point);
        colonne
    }

    /// Met à jour les points sous les tuiles et le bandeau « Applis ouvertes ».
    fn rafraichir(self: &Rc<Self>) {
        let Some(g) = &self.gestionnaire else { return };
        let mut liste: Vec<Fenetre> = g.liste().into_iter().filter(|f| !est_accueil(f)).collect();
        // Bureautique préparée à l'avance : sa fenêtre est réduite dès qu'elle apparaît, et cachée du bandeau
        let bureautique = &APPLIS[0];
        if let Some(debut) = self.prechargement.get() {
            if debut.elapsed().as_secs() > 180 {
                self.prechargement.set(None);
            } else if let Some(f) = liste.iter().find(|f| correspond(bureautique, f)) {
                self.prechargement.set(None);
                // On retient l'ordre d'activation de son apparition : une activation plus récente
                // (Alt+Tab, document ouvert depuis Fichiers…) la rendra visible
                self.prechargee.set(Some((f.id, f.activation)));
                g.minimiser(f.id);
            }
        }
        if let Some((id, apparition)) = self.prechargee.get() {
            match liste.iter().find(|f| f.id == id) {
                Some(f) if f.activation > apparition => self.prechargee.set(None),
                Some(_) => liste.retain(|f| f.id != id),
                None => self.prechargee.set(None),
            }
        }

        if self.en_cours.borrow().iter().any(|(n, _)| *n == "Mail") {
            let avant = self.fenetres_avant.borrow().clone();
            if liste.iter().any(|f| !avant.contains(&f.id)) {
                self.en_cours.borrow_mut().retain(|(n, _)| *n != "Mail");
                self.afficher_chargement("Mail", false);
            }
        }
        for (point, appli) in &self.points {
            let ouverte = liste.iter().any(|f| correspond(appli, f));
            point.set_opacity(if ouverte { 1.0 } else { 0.0 });
        }
        // Fin de l'« Ouverture… » dès que la fenêtre apparaît, pour toutes les applis
        // (tuiles et Console) : sinon un 2e clic dans les 25 s était ignoré.
        for appli in APPLIS.iter().chain(std::iter::once(&CONSOLE)) {
            let ouverte = liste.iter().any(|f| correspond(appli, f));
            if ouverte && self.en_cours.borrow().iter().any(|(n, _)| *n == appli.nom) {
                self.en_cours.borrow_mut().retain(|(n, _)| *n != appli.nom);
                self.afficher_chargement(appli.nom, false);
            }
        }

        // Bureau v12.5 : la barre du bas, un bouton par fenêtre
        if self.taches.is_some() {
            // Barre magique (v1.0.1) : recule tant qu'une fenêtre agrandie est ouverte (pas réduite),
            // même si une autre fenêtre passe devant : l'agrandie garde tout l'écran
            bureau::magique_fenetres(liste.iter().any(|f| !f.reduite && (f.agrandie || f.plein_ecran)));
            if *self.affichees.borrow() == liste && self.taches_construites.get() {
                return;
            }
            self.taches_construites.set(true);
            // Une autre fenêtre passe devant : la proposition « À droite » n'a plus lieu d'être
            let avant_actif = self.affichees.borrow().iter().find(|f| f.active).map(|f| f.id);
            let actif = liste.iter().find(|f| f.active).map(|f| f.id);
            let gauche = self.bulle_droite.borrow().as_ref().map(|(_, id)| *id);
            if gauche.is_some() && actif.is_some() && actif != avant_actif && actif != gauche {
                if let Some((b, _)) = self.bulle_droite.take() {
                    b.popdown();
                }
            }
            CHARGEMENTS.with(|c| {
                c.borrow_mut().retain(|(id, motifs, debut)| {
                    let arrivee = liste.iter().any(|f| {
                        let a = f.app_id.to_lowercase();
                        !a.is_empty() && motifs.iter().any(|m| a.contains(m.as_str()) || m.contains(a.as_str()))
                    });
                    if arrivee || debut.elapsed().as_secs() > 25 {
                        roue_epinglee(id, false);
                        false
                    } else {
                        true
                    }
                })
            });
            self.maj_taches(&liste);
            *self.affichees.borrow_mut() = liste;
            return;
        }
        // (rail : construit au moins une fois, même sans fenêtre, pour les applis épinglées)
        if *self.affichees.borrow() == liste && self.bandeau.first_child().is_some() {
            return;
        }
        while let Some(enfant) = self.bandeau.first_child() {
            self.bandeau.remove(&enfant);
        }
        // Applis épinglées en cours d'ouverture : la roue s'arrête quand leur fenêtre arrive
        CHARGEMENTS.with(|c| {
            c.borrow_mut().retain(|(id, motifs, debut)| {
                let arrivee = liste.iter().any(|f| {
                    let a = f.app_id.to_lowercase();
                    !a.is_empty() && motifs.iter().any(|m| a.contains(m.as_str()) || m.contains(a.as_str()))
                });
                if arrivee || debut.elapsed().as_secs() > 25 {
                    roue_epinglee(id, false);
                    false
                } else {
                    true
                }
            })
        });
        let groupes = grouper(&liste);
        // Accueil devant les fenêtres : une appli activée depuis (Alt + Tab, rail…) le remet derrière
        if let Some(depuis) = self.devant.get() {
            if liste.iter().any(|f| f.activation > depuis) {
                self.retirer_devant();
            }
        }
        // Barre des tâches : cachée quand une appli est en plein écran (vidéo, assistant…)
        if let Some(r) = &self.rail {
            if self.devant.get().is_none() {
                let devant = || liste.iter().filter(|f| f.active && !f.reduite);
                r.etat_fenetres(devant().any(|f| f.agrandie || f.plein_ecran), devant().any(|f| f.plein_ecran));
            }
        }
        self.bandeau.set_visible(self.barre_taches.is_some() || !groupes.is_empty());
        if !groupes.is_empty() && self.barre_taches.is_none() {
            let titre = gtk::Label::new(Some(&tr("Applis ouvertes")));
            titre.add_css_class("titre-ouvertes");
            titre.set_valign(gtk::Align::Center);
            self.bandeau.append(&titre);
        }
        // Appli utilisée en dernier : petit point lumineux sous son icône
        let derniere = liste.iter().filter(|f| f.activation > 0).max_by_key(|f| f.activation).map(|f| f.id);
        let installees = applis::installees();
        if self.barre_taches.is_some() {
            // Rail : les 5 applis principales toujours là (ouvertes ou non), puis les autres ouvertes
            for appli in APPLIS {
                match groupes.iter().find(|g| g.fenetres.iter().any(|f| correspond(appli, f))) {
                    Some(g) => {
                        let actif = derniere.map(|d| g.fenetres.iter().any(|f| f.id == d)).unwrap_or(false);
                        let icone = self.icone_ouverte(g, actif, &installees, Some(appli.icone));
                        // Repère : appli ouverte
                        icone.add_css_class("epinglee-ouverte");
                        self.bandeau.append(&icone);
                    }
                    None => self.bandeau.append(&self.icone_epinglee(appli)),
                }
            }
            let autres: Vec<&Groupe> = groupes.iter().filter(|g| !APPLIS.iter().any(|a| g.fenetres.iter().any(|f| correspond(a, f)))).collect();
            if !autres.is_empty() {
                let sep = gtk::Separator::new(if self.bandeau.orientation() == gtk::Orientation::Vertical {
                    gtk::Orientation::Horizontal
                } else {
                    gtk::Orientation::Vertical
                });
                sep.add_css_class("separateur-rail");
                self.bandeau.append(&sep);
            }
            for g in autres {
                let actif = derniere.map(|d| g.fenetres.iter().any(|f| f.id == d)).unwrap_or(false);
                self.bandeau.append(&self.icone_ouverte(g, actif, &installees, None));
            }
        } else {
            for g in &groupes {
                let actif = derniere.map(|d| g.fenetres.iter().any(|f| f.id == d)).unwrap_or(false);
                self.bandeau.append(&self.icone_ouverte(g, actif, &installees, None));
            }
        }
        *self.affichees.borrow_mut() = liste;
    }

    /// Une appli ouverte : son icône (clic = y aller), ✕ au survol (fermer),
    /// clic droit ou appui long = menu (chaque fenêtre, fermer).
    fn icone_ouverte(self: &Rc<Self>, g: &Groupe, actif: bool, installees: &[applis::Installee], epinglee: Option<&'static str>) -> gtk::Box {
        // Appli épinglée au rail : même icône, ouverte ou non
        let image = match epinglee {
            Some(nom) => icones::image(nom, 32),
            None => icone_fenetre(&g.fenetres[0], installees),
        };
        image.set_pixel_size(32);
        let bouton = gtk::Button::builder().child(&image).build();
        bouton.add_css_class("ouverte");
        let titres: Vec<String> = g.fenetres.iter().map(|f| nom_affiche(f)).collect();
        let info = if g.fenetres.len() == 1 {
            if titres[0] == g.nom { g.nom.clone() } else { format!("{} — {}", g.nom, titres[0]) }
        } else {
            trv("{nom} ({n} fenêtres)", &[("nom", &g.nom), ("n", &g.fenetres.len().to_string())])
        };
        bouton.set_tooltip_text(Some(&info));
        // Place pour le ✕ et le compteur, posés à cheval sur les coins
        bouton.set_margin_top(7);
        bouton.set_margin_end(7);
        bouton.set_margin_start(7);
        // Clic : la fenêtre de ce groupe utilisée en dernier passe devant ;
        // si elle est déjà devant, le groupe est réduit (comme la barre des tâches de Windows)
        let cible = g.fenetres.iter().max_by_key(|f| f.activation).map(|f| f.id).unwrap_or(g.fenetres[0].id);
        let devant = g.fenetres.iter().any(|f| f.active && !f.reduite);
        if devant {
            bouton.add_css_class("devant");
        }
        let ids_groupe: Vec<u32> = g.fenetres.iter().map(|f| f.id).collect();
        let moi = Rc::downgrade(self);
        bouton.connect_clicked(move |_| {
            let Some(m) = moi.upgrade() else { return };
            let Some(gest) = m.gestionnaire.clone() else { return };
            let encore_devant = gest.liste().iter().any(|f| ids_groupe.contains(&f.id) && f.active && !f.reduite);
            if encore_devant {
                for id in &ids_groupe {
                    gest.minimiser(*id);
                }
            } else {
                m.bureau_montre.borrow_mut().clear();
                gest.activer(cible);
            }
        });

        let superpose = gtk::Overlay::new();
        superpose.add_css_class("cadre-ouverte");
        superpose.set_child(Some(&bouton));
        if g.fenetres.len() > 1 {
            let compteur = gtk::Label::new(Some(&g.fenetres.len().to_string()));
            compteur.add_css_class("compteur");
            compteur.set_halign(gtk::Align::End);
            compteur.set_valign(gtk::Align::End);
            compteur.set_can_target(false);
            superpose.add_overlay(&compteur);
        }
        // ✕ au survol de la souris (pas au doigt : sur un écran tactile, on passe par l'appui long)
        let fermer = gtk::Label::new(Some(&tr("✕")));
        fermer.add_css_class("fermer-mini");
        fermer.set_halign(gtk::Align::End);
        fermer.set_valign(gtk::Align::Start);
        fermer.set_tooltip_text(Some(&if g.fenetres.len() > 1 { tr("Tout fermer") } else { tr("Fermer") }));
        let ids: Vec<u32> = g.fenetres.iter().map(|f| f.id).collect();
        let clic = gtk::GestureClick::new();
        clic.set_button(1);
        clic.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (moi, ids_f) = (Rc::downgrade(self), ids.clone());
        clic.connect_pressed(move |geste, _, _, _| {
            let tactile = geste
                .current_event_device()
                .map(|d| d.source() == gdk::InputSource::Touchscreen)
                .unwrap_or(false);
            if tactile {
                return;
            }
            geste.set_state(gtk::EventSequenceState::Claimed);
            if let Some(gest) = moi.upgrade().and_then(|m| m.gestionnaire.clone()) {
                for id in &ids_f {
                    gest.fermer(*id);
                }
            }
        });
        fermer.add_controller(clic);
        superpose.add_overlay(&fermer);

        // Menu : aller à chaque fenêtre, fermer
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let bulle = gtk::Popover::new();
        bulle.set_child(Some(&menu));
        bulle.set_parent(&bouton);
        bulle.set_position(rail::position_bulles());
        let titre = gtk::Label::new(Some(&g.nom));
        titre.add_css_class("titre-menu");
        titre.set_xalign(0.0);
        menu.append(&titre);
        for (f, t) in g.fenetres.iter().zip(titres.iter()) {
            let (moi, id) = (Rc::downgrade(self), f.id);
            let texte = if g.fenetres.len() == 1 { format!("↗  {}", tr("Y aller")) } else { format!("↗  {t}") };
            menu.append(&entree_menu(&texte, &bulle, move || {
                if let Some(gest) = moi.upgrade().and_then(|m| m.gestionnaire.clone()) {
                    gest.activer(id);
                }
            }));
        }
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        // Côte à côte : moitié gauche, moitié droite, tout l'écran
        for (texte, touche) in [
            (format!("◧  {}", tr("À gauche")), "Left"),
            (format!("◨  {}", tr("À droite")), "Right"),
            (format!("▣  {}", tr("Tout l'écran")), "Up"),
        ] {
            let moi = Rc::downgrade(self);
            menu.append(&entree_menu(&texte, &bulle, move || {
                if let Some(m) = moi.upgrade() {
                    m.placer(cible, touche);
                }
            }));
        }
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let moi = Rc::downgrade(self);
        let texte = if ids.len() > 1 {
            format!("✕  {}", trv("Tout fermer ({n})", &[("n", &ids.len().to_string())]))
        } else {
            format!("✕  {}", tr("Fermer"))
        };
        let f = entree_menu(&texte, &bulle, move || {
            if let Some(gest) = moi.upgrade().and_then(|m| m.gestionnaire.clone()) {
                for id in &ids {
                    gest.fermer(*id);
                }
            }
        });
        f.add_css_class("danger");
        menu.append(&f);
        attacher_menu_contextuel(&bouton, &bulle);

        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 0);
        colonne.append(&superpose);
        let point = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        point.add_css_class("point-ouverte");
        point.set_halign(gtk::Align::Center);
        point.set_opacity(if actif { 1.0 } else { 0.0 });
        colonne.append(&point);
        colonne
    }

    /// Lit l'état de la machine et met la barre à jour.
    fn mettre_a_jour_barre(&self) {
        let b = &self.barre;
        b.maj_prete.set_visible(std::path::Path::new("/run/flashos/maj-prete").exists());

        let (icone, info) = match systeme::reseau() {
            Reseau::Wifi(q) => ("wifi", trv("Wi-Fi connecté (signal {q} %)", &[("q", &q.to_string())])),
            Reseau::Filaire => ("filaire", tr("Connecté par câble")),
            Reseau::Aucun => ("wifi-off", tr("Pas de connexion")),
        };
        icones::changer(&b.reseau, &format!("r-{icone}"));
        // Barre du bas : on ne touche qu'à ce qui a changé (une bulle ouverte se refermerait)
        // Barre du bas (v13.1) : tuiles en couleur, comme la barre magique
        // Bleu : connecté (Wi-Fi ou câble) ; rouge : pas de connexion
        let tuile = if icone == "wifi-off" { "r-wifi-off-bas".to_string() } else { format!("r-{icone}") };
        if b.reseau_bas.widget_name() != tuile.as_str() {
            icones::changer(&b.reseau_bas, &tuile);
            b.reseau_bas.set_widget_name(&tuile);
        }
        if b.bouton_reseau_bas.tooltip_text().as_deref() != Some(info.as_str()) {
            b.bouton_reseau_bas.set_tooltip_text(Some(&info));
        }
        b.info_reseau.set_label(&info);
        if let Some(actif) = systeme::wifi_actif() {
            b.maj_en_cours.set(true);
            b.interrupteur_wifi.set_active(actif);
            b.maj_en_cours.set(false);
        }
        b.bouton_reseau.set_tooltip_text(Some(&trv("{info} — cliquer pour choisir un réseau", &[("info", &info)])));

        match systeme::batterie() {
            Some((niveau, charge)) => {
                // Tuile verte, orange (40 % et moins) ou rouge (20 % et moins) ; éclair en charge
                let nom = if charge {
                    "r-batterie-charge"
                } else if niveau <= 20 {
                    "r-batterie-faible"
                } else if niveau <= 40 {
                    "r-batterie-moyen"
                } else {
                    "r-batterie"
                };
                if b.image_batterie.widget_name() != nom {
                    icones::changer(&b.image_batterie, nom);
                    b.image_batterie.set_widget_name(nom);
                }
                b.texte_batterie.set_label(&crate::tr::pourcent(niveau));
                b.batterie.set_visible(true);
                b.niveau_batterie.set((niveau as u32, charge));
                b.texte_anneau.set_label(&niveau.to_string());
                b.anneau.queue_draw();
                b.anneau.set_tooltip_text(Some(&if charge {
                    trv("Batterie : {n}, en charge", &[("n", &crate::tr::pourcent(niveau))])
                } else {
                    trv("Batterie : {n}", &[("n", &crate::tr::pourcent(niveau))])
                }));
                if let Some(p) = b.anneau.parent() {
                    p.set_visible(true);
                }
            }
            None => {
                b.batterie.set_visible(false);
                if let Some(p) = b.anneau.parent() {
                    p.set_visible(false);
                }
            }
        }

        // Chargeur branché / débranché : luminosité adaptée (option des Réglages → Batterie)
        if let Some(secteur) = systeme::secteur() {
            if let Some(avant) = self.sur_secteur.replace(Some(secteur)) {
                if avant != secteur {
                    std::thread::spawn(move || systeme::luminosite_selon_alimentation(secteur));
                }
            }
        }

        match systeme::volume() {
            Some((niveau, coupe)) => {
                icones::changer(&b.son, if coupe || niveau == 0 { "r-son-coupe" } else { "r-son" });
                b.info_son.set_label(&if coupe { tr("Son coupé") } else { format!("{} · {}", tr("Volume"), crate::tr::pourcent(niveau)) });
                b.maj_en_cours.set(true);
                b.curseur_son.set_value(niveau.min(100) as f64);
                b.maj_en_cours.set(false);
                b.bouton_muet.set_label(&if coupe { tr("Rétablir le son") } else { tr("Couper le son") });
                b.bouton_son.set_visible(true);
                let nom = if coupe || niveau == 0 { "r-son-coupe-bas" } else { "r-son-bas" };
                if b.son_bas.widget_name() != nom {
                    icones::changer(&b.son_bas, nom);
                    b.son_bas.set_widget_name(nom);
                }
                let info = if coupe { tr("Son coupé") } else { format!("{} · {}", tr("Volume"), crate::tr::pourcent(niveau)) };
                if b.bouton_son_bas.tooltip_text().as_deref() != Some(info.as_str()) {
                    b.bouton_son_bas.set_tooltip_text(Some(&info));
                }
                if (b.curseur_bas.value() - niveau.min(100) as f64).abs() > 0.5 {
                    b.maj_en_cours.set(true);
                    b.curseur_bas.set_value(niveau.min(100) as f64);
                    b.maj_en_cours.set(false);
                }
                let texte = if coupe { tr("Rétablir le son") } else { tr("Couper le son") };
                if b.muet_bas.label().as_deref() != Some(texte.as_str()) {
                    b.muet_bas.set_label(&texte);
                }
                if !b.bouton_son_bas.is_visible() {
                    b.bouton_son_bas.set_visible(true);
                }
            }
            None => {
                b.bouton_son.set_visible(false);
                if b.bouton_son_bas.is_visible() {
                    b.bouton_son_bas.set_visible(false);
                }
            }
        }
    }

}

/// Démarré sur l'outil de réparation (menu de secours ou bouton « Réparer »)
fn mode_reparation() -> bool {
    std::fs::read_to_string("/proc/cmdline").map(|l| l.contains("flashos.reparer")).unwrap_or(false)
        || std::env::var_os("FLASHOS_TEST_REPARATION").is_some()
}

fn correspond(appli: &Appli, f: &Fenetre) -> bool {
    let id = f.app_id.to_lowercase();
    // Sites ouverts comme des applications (« chrome-<site>-Default ») : pas le navigateur lui-même
    if id.starts_with("chrome-") || id.starts_with("crx_") {
        return false;
    }
    appli.motifs.iter().any(|m| id.contains(m))
}

/// Titre d'une fenêtre pour la barre et Alt + Tab : « Console » plutôt que « foot » (v13.1),
/// sinon le titre donné par l'application (nom du document, de la page…).
pub(crate) fn titre_lisible(f: &Fenetre) -> String {
    if correspond(&CONSOLE, f) {
        return tr("Console");
    }
    let t = f.titre.trim();
    if t.is_empty() || t.eq_ignore_ascii_case(&f.app_id) {
        return nom_affiche(f);
    }
    t.trim_end_matches(" - Chromium").to_string()
}

/// Nom lisible d'une fenêtre : celui de l'appli connue, sinon son titre.
fn nom_affiche(f: &Fenetre) -> String {
    if let Some(a) = APPLIS.iter().chain(std::iter::once(&CONSOLE)).find(|a| correspond(a, f)) {
        // Pour le navigateur, le titre de la page est plus parlant.
        if a.icone != "tuile-internet" {
            return tr(a.nom);
        }
    }
    let titre = if f.titre.is_empty() { f.app_id.clone() } else { f.titre.clone() };
    let titre = titre.trim_end_matches(" - Chromium").to_string();
    if titre.chars().count() > 28 {
        format!("{}…", titre.chars().take(27).collect::<String>())
    } else {
        titre
    }
}

/// Fenêtres ouvertes d'une même appli (une seule icône dans le bandeau)
struct Groupe {
    nom: String,
    fenetres: Vec<Fenetre>,
}

/// Regroupe les fenêtres par appli, dans l'ordre d'ouverture
fn grouper(liste: &[Fenetre]) -> Vec<Groupe> {
    let mut groupes: Vec<(String, Groupe)> = Vec::new();
    for f in liste {
        let (cle, nom) = match APPLIS.iter().chain(std::iter::once(&CONSOLE)).find(|a| correspond(a, f)) {
            Some(a) => (a.nom.to_string(), tr(a.nom)),
            None => {
                let nom = if f.titre.is_empty() { f.app_id.clone() } else { nom_affiche(f) };
                let cle = if f.app_id.is_empty() { format!("#{}", f.id) } else { f.app_id.to_lowercase() };
                (cle, nom)
            }
        };
        match groupes.iter_mut().find(|(c, _)| *c == cle) {
            Some((_, g)) => g.fenetres.push(f.clone()),
            None => groupes.push((cle, Groupe { nom, fenetres: vec![f.clone()] })),
        }
    }
    groupes.into_iter().map(|(_, g)| g).collect()
}

/// Icône d'une fenêtre : appli SoleOS, appli ajoutée, sinon icône déclarée par l'appli
fn icone_fenetre(f: &Fenetre, installees: &[applis::Installee]) -> gtk::Image {
    let id = f.app_id.to_lowercase();
    // LibreOffice : une icône par module (Texte, Tableur, Présentation)
    for (motif, icone) in [("writer", "d-texte"), ("calc", "d-tableur"), ("impress", "d-presentation"), ("draw", "d-presentation")] {
        if (id.starts_with("libreoffice") || id.starts_with("soffice") || id.starts_with("org.libreoffice")) && id.contains(motif) {
            return icones::image(icone, 32);
        }
    }
    // Applis SoleOS : les mêmes icônes colorées que la barre magique (v13.1)
    if let Some(a) = APPLIS.iter().chain(std::iter::once(&CONSOLE)).find(|a| correspond(a, f)) {
        let icone = match a.icone {
            "tuile-bureautique" => "bureautique",
            "tuile-mail" => "mail",
            "tuile-internet" => "internet",
            "tuile-fichiers" => "fichiers",
            "tuile-applications" => "applications",
            "tuile-console" => "r-console-bas",
            autre => autre,
        };
        return icones::image(icone, 32);
    }
    if !id.is_empty() {
        if let Some(a) = installees.iter().find(|a| a.motifs().iter().any(|m| id.contains(m.as_str()) || m.contains(id.as_str()))) {
            if let Some(i) = icones::appli(a.icone.as_deref(), &[a.id.as_str(), &f.app_id], 32) {
                return i;
            }
        }
        // Lanceur .desktop portant le nom de l'appli (ex. org.gnome.Loupe.desktop)
        if let Some(icone) = icone_lanceur(&f.app_id).or_else(|| icone_lanceur(&id)) {
            let chemin = icone.starts_with('/').then(|| PathBuf::from(&icone));
            if let Some(i) = icones::appli(chemin.as_deref(), &[icone.as_str(), &f.app_id], 32) {
                return i;
            }
        }
        // Thème d'icônes (nom complet, puis dernier morceau : « org.videolan.VLC » → « vlc »)
        if let Some(i) = icones::appli(None, &[&f.app_id], 32) {
            return i;
        }
    }
    icones::image("appli", 32)
}

/// Icône déclarée dans le lanceur « <id>.desktop » (dossiers d'applications du système et de l'utilisateur)
fn icone_lanceur(id: &str) -> Option<String> {
    let mut dossiers: Vec<PathBuf> = Vec::new();
    if let Some(h) = std::env::var_os("HOME") {
        dossiers.push(PathBuf::from(h).join(".local/share"));
    }
    let systeme = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dossiers.extend(systeme.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    for d in dossiers {
        let f = d.join("applications").join(format!("{id}.desktop"));
        if let Ok(texte) = std::fs::read_to_string(&f) {
            if let Some(i) = texte.lines().find_map(|l| l.strip_prefix("Icon=")) {
                let i = i.trim();
                if !i.is_empty() {
                    return Some(i.to_string());
                }
            }
        }
    }
    None
}

/// « Définir comme arrière-plan » dans Fichiers (réglage GNOME) : la photo devient le fond de l'accueil
fn surveiller_fond_gnome() {
    let Some(source) = gtk::gio::SettingsSchemaSource::default() else { return };
    if source.lookup("org.gnome.desktop.background", true).is_none() {
        return;
    }
    let reglage = gtk::gio::Settings::new("org.gnome.desktop.background");
    let changement = |r: &gtk::gio::Settings, cle: &str| {
        let uri = r.string(cle).to_string();
        let Some(chemin) = gtk::gio::File::for_uri(&uri).path() else { return };
        // Le fond déjà importé (même fichier) : rien à faire
        if chemin.is_file() && fonds::perso().map(|p| p != chemin).unwrap_or(true) {
            if fonds::importer(&chemin).is_ok() {
                fonds::enregistrer("perso");
                fonds::appliquer("perso");
            }
        }
    };
    reglage.connect_changed(Some("picture-uri"), move |r, cle| changement(r, cle));
    reglage.connect_changed(Some("picture-uri-dark"), move |r, cle| changement(r, cle));
    // Lecture initiale : nécessaire pour recevoir les changements
    let _ = reglage.string("picture-uri");
    let _ = reglage.string("picture-uri-dark");
    // Garde le réglage en vie tant que l'accueil tourne
    std::mem::forget(reglage);
}

/// Lance un programme sans bloquer l'accueil.
pub(crate) fn lancer(commande: &[&str]) {
    let Some((prog, args)) = commande.split_first() else { return };
    let mut cmd = Command::new(prog);
    cmd.args(args);
    if let Some(maison) = std::env::var_os("HOME") {
        cmd.current_dir(maison);
    }
    match cmd.spawn() {
        // On attend la fin du programme dans un fil à part (évite les processus « zombies »).
        Ok(mut enfant) => {
            std::thread::spawn(move || {
                let _ = enfant.wait();
            });
        }
        Err(e) => eprintln!("flashos-accueil : impossible de lancer {prog} : {e}"),
    }
}

/// Encode un texte pour une adresse web (recherche).
fn encoder(texte: &str) -> String {
    texte
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn majuscule(texte: &str) -> String {
    let mut c = texte.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn mettre_a_jour_horloge(heure: &gtk::Label, date: &gtk::Label) {
    if let Ok(maintenant) = glib::DateTime::now_local() {
        if let Ok(h) = maintenant.format("%H:%M") {
            heure.set_label(&h);
        }
        if let Ok(d) = maintenant.format(&tr("%A %-e %B")) {
            date.set_label(&majuscule(&d));
        }
    }
}

/// « Bonjour Giuseppe » le jour, « Bonsoir Giuseppe » à partir de 18 h (caché sans prénom)
fn mettre_a_jour_salut(salut: &gtk::Label) {
    let Some(prenom) = systeme::prenom() else {
        salut.set_visible(false);
        return;
    };
    let h = glib::DateTime::now_local().map(|d| d.hour()).unwrap_or(12);
    let texte = if (5..18).contains(&h) {
        trv("Bonjour {prenom}", &[("prenom", &prenom)])
    } else {
        trv("Bonsoir {prenom}", &[("prenom", &prenom)])
    };
    salut.set_label(&texte);
    salut.set_visible(true);
}

/// Fait apparaître un élément en douceur.
fn fondu(element: &gtk::Box, depart: f64) {
    element.set_opacity(depart);
    let element = element.clone();
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let o = (element.opacity() + 0.07).min(1.0);
        element.set_opacity(o);
        if o >= 1.0 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
}

/// Bouton rond de la barre d'état.
fn bouton_etat(icone: &str, info: &str) -> (gtk::Button, gtk::Image) {
    let image = icones::image(icone, 20);
    image.add_css_class("icone-etat");
    let bouton = gtk::Button::builder().child(&image).tooltip_text(tr(info)).build();
    bouton.add_css_class("etat");
    (bouton, image)
}

/// Entrée de menu (dans une bulle).
pub(crate) fn entree_menu(texte: &str, bulle: &gtk::Popover, action: impl Fn() + 'static) -> gtk::Button {
    let bouton = gtk::Button::with_label(&tr(texte));
    bouton.add_css_class("menu");
    if let Some(label) = bouton.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
        label.set_xalign(0.0);
    }
    let bulle = bulle.clone();
    bouton.connect_clicked(move |_| {
        bulle.popdown();
        action();
    });
    bouton
}

/// Attache une bulle à un bouton : un clic l'ouvre.
/// Lance une application ajoutée (Flatpak ou AppImage)
fn lancer_installee(a: &applis::Installee) {
    let commande = a.commande();
    let mut c = Command::new(&commande[0]);
    c.args(&commande[1..]).env("APPIMAGE_EXTRACT_AND_RUN", "1");
    if let Some(maison) = std::env::var_os("HOME") {
        c.current_dir(maison);
    }
    if let Ok(mut e) = c.spawn() {
        std::thread::spawn(move || {
            let _ = e.wait();
        });
    }
}

/// Transmet une demande à l'accueil (fichier « flashos-ouvrir » + réveil de l'accueil)
fn demander(contenu: &str) {
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") {
        let _ = std::fs::write(PathBuf::from(d).join("flashos-ouvrir"), contenu);
    }
    lancer(&["flashos-accueil"]);
}

/// Clic droit ou appui long sur un bouton : ouvre la bulle donnée
pub(crate) fn attacher_menu_contextuel(bouton: &gtk::Button, bulle: &gtk::Popover) {
    let clic = gtk::GestureClick::new();
    clic.set_button(3);
    // Phase « capture » : le clic droit est vu avant que le bouton ne le garde pour lui
    clic.set_propagation_phase(gtk::PropagationPhase::Capture);
    let b = bulle.clone();
    clic.connect_pressed(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        b.popup();
    });
    bouton.add_controller(clic);
    let appui = gtk::GestureLongPress::new();
    appui.set_propagation_phase(gtk::PropagationPhase::Capture);
    let b = bulle.clone();
    // Appui long reconnu : on garde le geste pour nous, sinon le relâchement compte aussi
    // comme un clic sur la tuile (l'application s'ouvre et le menu disparaît aussitôt)
    appui.connect_pressed(move |g, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        b.popup();
    });
    bouton.add_controller(appui);
}

/// Tuile d'une application épinglée sur l'accueil
fn tuile_epinglee(a: &applis::Installee) -> gtk::Button {
    let image = gtk::Image::new();
    image.set_pixel_size(72);
    match a.icone.as_ref().and_then(|c| gdk::Texture::from_filename(c).ok()) {
        Some(t) => image.set_paintable(Some(&t)),
        None => icones::changer(&image, "appli"),
    }
    let cadre = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cadre.set_size_request(84, 84);
    cadre.set_valign(gtk::Align::Center);
    image.set_vexpand(true);
    // Roue de chargement par-dessus l'icône, pendant l'ouverture (comme les tuiles de base)
    let roue = gtk::Spinner::new();
    roue.set_size_request(40, 40);
    roue.set_halign(gtk::Align::Center);
    roue.set_valign(gtk::Align::Center);
    roue.add_css_class("roue");
    roue.set_visible(false);
    let superpose = gtk::Overlay::new();
    superpose.set_child(Some(&image));
    superpose.add_overlay(&roue);
    superpose.set_vexpand(true);
    cadre.append(&superpose);
    ROUES_EPINGLES.with(|r| {
        let mut r = r.borrow_mut();
        r.retain(|(i, _)| i != &a.id);
        r.push((a.id.clone(), roue.clone()));
    });
    let nom = gtk::Label::new(Some(&a.nom));
    nom.add_css_class("nom-appli");
    nom.set_max_width_chars(14);
    nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let contenu = gtk::Box::new(gtk::Orientation::Vertical, 10);
    contenu.append(&cadre);
    contenu.append(&nom);
    let bouton = gtk::Button::builder().child(&contenu).build();
    bouton.add_css_class("tuile");
    bouton.set_tooltip_text(Some(&a.nom));
    // Déjà ouverte : ramenée devant ; sinon lancée (géré par l'accueil, qui connaît les fenêtres)
    let id_appli = a.id.clone();
    bouton.connect_clicked(move |_| demander(&format!("appli:{id_appli}")));
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let bulle = gtk::Popover::new();
    bulle.set_child(Some(&menu));
    bulle.set_parent(&bouton);
    let id = a.id.clone();
    // L'accueil se met à jour tout seul (vérification toutes les 3 s)
    menu.append(&entree_menu(&tr("Retirer de l'accueil"), &bulle, move || applis::retirer_epingle(&id)));
    attacher_menu_contextuel(&bouton, &bulle);
    bouton
}

fn attacher_bulle(bouton: &gtk::Button, contenu: &impl IsA<gtk::Widget>) -> gtk::Popover {
    let bulle = gtk::Popover::new();
    bulle.set_child(Some(contenu));
    bulle.set_parent(bouton);
    // Boutons dans la barre des tâches (en bas) : la bulle s'ouvre vers le haut
    bulle.set_position(if BARRE_EN_BAS.with(|b| b.get()) { rail::position_bulles() } else { gtk::PositionType::Bottom });
    let b = bulle.clone();
    bouton.connect_clicked(move |_| b.popup());
    bulle
}

thread_local! {
    /// Roues de chargement des tuiles épinglées (identifiant de l'appli → roue)
    static ROUES_EPINGLES: RefCell<Vec<(String, gtk::Spinner)>> = const { RefCell::new(Vec::new()) };
    /// Applis épinglées en cours d'ouverture : identifiant, motifs de fenêtre, moment du clic
    static CHARGEMENTS: RefCell<Vec<(String, Vec<String>, std::time::Instant)>> = const { RefCell::new(Vec::new()) };
}

/// Montre ou cache la roue de chargement d'une tuile épinglée
fn roue_epinglee(id: &str, oui: bool) {
    ROUES_EPINGLES.with(|r| {
        for (i, roue) in r.borrow().iter() {
            if i == id {
                roue.set_visible(oui);
                roue.set_spinning(oui);
            }
        }
    });
}

thread_local! {
    /// Vrai quand la barre d'état est dans la barre des tâches, en bas de l'écran
    static BARRE_EN_BAS: Cell<bool> = const { Cell::new(false) };
}

/// Barre des tâches possible : le gestionnaire d'affichage accepte les « couches » (layer-shell)
fn barre_des_taches_possible() -> bool {
    std::env::var_os("FLASHOS_SANS_BARRE").is_none() && gtk4_layer_shell::is_supported()
}

/// Taille de l'écran qui affiche la fenêtre (ou du premier écran).
fn taille_ecran(fenetre: &gtk::ApplicationWindow) -> Option<(i32, i32)> {
    let ecran = gdk::Display::default()?;
    let moniteur = fenetre
        .surface()
        .and_then(|s| ecran.monitor_at_surface(&s))
        .or_else(|| ecran.monitors().item(0).and_then(|m| m.downcast::<gdk::Monitor>().ok()))?;
    let g = moniteur.geometry();
    Some((g.width(), g.height()))
}

/// Réapplique l'agrandissement si l'écran change (pilote graphique chargé
/// tardivement, écran externe…) ou si la fenêtre n'occupe pas tout l'écran.
fn surveiller_plein_ecran(fenetre: &gtk::ApplicationWindow) {
    // Banc de captures d'écran (barre de titre du gestionnaire d'affichage) : pas de surveillance
    if std::env::var_os("FLASHOS_TEST_FENETRE").is_some() {
        return;
    }
    let reappliquer = |f: &gtk::ApplicationWindow| {
        if std::env::var_os("FLASHOS_DEBUG").is_some() {
            eprintln!("taille réappliquée ({}x{})", f.width(), f.height());
        }
        if let Some((l, h)) = taille_ecran(f) {
            f.set_default_size(l, h);
        }
        f.unmaximize();
        let f = f.clone();
        glib::timeout_add_local_once(Duration::from_millis(250), move || f.maximize());
    };
    if let Some(ecran) = gdk::Display::default() {
        let f = fenetre.clone();
        ecran.monitors().connect_items_changed(move |_, _, _, _| {
            // Écran branché / débranché : dupliquer, étendre… selon le choix mémorisé
            glib::timeout_add_local_once(Duration::from_millis(300), appareils::ecran_change);
            let f2 = f.clone();
            glib::timeout_add_local_once(Duration::from_millis(500), move || reappliquer(&f2));
        });
    }
    let f = fenetre.downgrade();
    glib::timeout_add_seconds_local(2, move || {
        let Some(f) = f.upgrade() else { return glib::ControlFlow::Break };
        if let Some((l, h)) = taille_ecran(&f) {
            // Fenêtre plus petite OU plus grande que l'écran (zoom ou résolution changés) : on recale
            if f.is_visible() && ((f.width() - l).abs() > 8 || (f.height() - h).abs() > 40) {
                reappliquer(&f);
            }
        }
        glib::ControlFlow::Continue
    });
}

fn construire(app: &gtk::Application, suivi: Option<(Gestionnaire, async_channel::Receiver<()>)>) -> Rc<Accueil> {
    let fenetre = gtk::ApplicationWindow::builder()
        .application(app)
        .title(tr("Accueil"))
        .decorated(false)
        .default_width(800)
        .default_height(600)
        .build();
    fenetre.add_css_class("accueil");
    theme::adapter(&fenetre);
    // Avec la barre des tâches, l'accueil devient le fond de l'écran : toujours derrière les
    // applications (qui restent visibles et côte à côte), au-dessus de rien.
    let couches = barre_des_taches_possible();
    BARRE_EN_BAS.with(|b| b.set(couches));
    if couches {
        fenetre.init_layer_shell();
        fenetre.set_layer(Layer::Bottom);
        fenetre.set_namespace(Some("flashos-accueil"));
        for bord in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            fenetre.set_anchor(bord, true);
        }
        // Tout l'écran, y compris derrière le rail
        fenetre.set_exclusive_zone(-1);
        fenetre.set_keyboard_mode(KeyboardMode::OnDemand);
    }
    // L'accueil ne se ferme jamais (Alt + F4 n'a pas d'effet).
    fenetre.connect_close_request(|_| glib::Propagation::Stop);

    // --- Barre du haut : marque et barre d'état ----------------------------
    let marque = theme::marque();
    // Version (fichier /etc/flashos-version écrit à la construction)
    if let Some(version) = theme::pastille_version() {
        version.set_margin_start(6);
        marque.append(&version);
    }

    let (bouton_reseau, image_reseau) = bouton_etat("wifi-off", &tr("Réseau"));
    let (bouton_son, image_son) = bouton_etat("son", &tr("Son"));
    let (bouton_reglages, _) = bouton_etat("reglages", &tr("Réglages : imprimantes, Bluetooth, écran, code…"));
    let (bouton_alim, _) = bouton_etat("alimentation", &tr("Éteindre, redémarrer…"));

    // Réseau : ouvre l'outil de choix du Wi-Fi.
    let panneau_wifi = wifi::Panneau::nouveau();
    let bulle_wifi = if couches { gtk::Popover::new() } else { attacher_bulle(&bouton_reseau, &panneau_wifi.racine) };
    if !couches {
        let p = panneau_wifi.clone();
        bulle_wifi.connect_show(move |_| p.actualiser());
    }

    // Réglages : tout ce qu'on règle une fois (imprimantes, Bluetooth, écran, taille, code…)
    let panneau_reglages = reglages::Reglages::nouveau();
    let bulle_reglages = if couches { gtk::Popover::new() } else { attacher_bulle(&bouton_reglages, &panneau_reglages.racine) };
    if !couches {
        *panneau_reglages.bulle.borrow_mut() = Some(bulle_reglages.clone());
    }
    if !couches {
        let p = panneau_reglages.clone();
        bulle_reglages.connect_show(move |_| p.au_debut());
        if std::env::var_os("FLASHOS_DEBUG").is_some() {
            bulle_reglages.connect_closed(|_| eprintln!("bulle des réglages fermée"));
        }
    }

    // Son : bulle avec un curseur et « couper ».
    let curseur_son = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 5.0);
    curseur_son.set_draw_value(false);
    let bouton_muet = gtk::Button::with_label(&tr("Couper le son"));
    bouton_muet.add_css_class("menu");
    let boite_son = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let titre_son = gtk::Label::new(Some(&tr("Volume")));
    titre_son.add_css_class("titre-menu");
    titre_son.set_xalign(0.0);
    // Cartes Wi-Fi et Son en tête des Réglages (bureau v12)
    let info_reseau = gtk::Label::new(None);
    let info_son = gtk::Label::new(None);
    let interrupteur_wifi = gtk::Switch::new();
    let carte_son = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let (image_reseau, image_son) = if couches {
        let (carte_wifi, img_r) = carte_tete("r-wifi-off", &tr("Wi-Fi"), &info_reseau);
        interrupteur_wifi.set_valign(gtk::Align::Center);
        if let Some(ligne) = carte_wifi.first_child().and_downcast::<gtk::Box>() {
            ligne.append(&interrupteur_wifi);
        }
        let choisir = gtk::Button::with_label(&tr("Choisir un réseau"));
        choisir.add_css_class("secondaire");
        carte_wifi.append(&choisir);
        let pr = panneau_reglages.clone();
        let pw = panneau_wifi.clone();
        choisir.connect_clicked(move |_| {
            pw.actualiser();
            pr.ouvrir("wifi");
        });
        panneau_reglages.ajouter_page_externe("wifi", &panneau_wifi.racine);
        let (carte_s, img_s) = carte_tete("r-son", &tr("Son"), &info_son);
        let ligne_son = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        curseur_son.set_hexpand(true);
        ligne_son.append(&curseur_son);
        bouton_muet.remove_css_class("menu");
        bouton_muet.add_css_class("secondaire");
        ligne_son.append(&bouton_muet);
        carte_s.append(&ligne_son);
        // Pas de son ? Essai des réglages connus pour la puce son de cet ordinateur
        let essai = essai_son::EssaiSon::nouveau();
        panneau_reglages.ajouter_page_externe("essai-son", &essai.racine);
        // Signaler un problème (éclair, Réglages → À propos)
        let signalement = signaler::Signaler::nouveau();
        panneau_reglages.ajouter_page_externe("signaler", &signalement.racine);
        std::mem::forget(signalement);
        let pas_de_son = gtk::Button::with_label(&tr("Le son ne marche pas ?"));
        pas_de_son.add_css_class("lien-son");
        pas_de_son.set_halign(gtk::Align::Start);
        {
            let (pr, e) = (panneau_reglages.clone(), essai.clone());
            pas_de_son.connect_clicked(move |_| {
                e.au_debut();
                pr.ouvrir("essai-son");
            });
        }
        // (v13 : essai du son retiré de l'interface ; corrections par modèle appliquées au démarrage)
        let _ = &pas_de_son;
        carte_son.append(&carte_s);
        panneau_reglages.tete.append(&carte_wifi);
        panneau_reglages.tete.append(&carte_son);
        (img_r, img_s)
    } else {
        boite_son.append(&titre_son);
        boite_son.append(&curseur_son);
        boite_son.append(&bouton_muet);
        attacher_bulle(&bouton_son, &boite_son);
        (image_reseau, image_son)
    };
    {
        let moi_w = std::rc::Rc::new(std::cell::Cell::new(false));
        let _ = moi_w;
        interrupteur_wifi.connect_state_set(|_, oui| {
            std::thread::spawn(move || systeme::allumer_wifi(oui));
            glib::Propagation::Proceed
        });
    }

    // Batterie (cachée sur un PC fixe).
    let image_batterie = icones::image("r-batterie", 26);
    let texte_batterie = gtk::Label::new(None);
    texte_batterie.add_css_class("texte-etat");
    let batterie = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    batterie.set_margin_start(6);
    batterie.set_margin_end(6);
    batterie.append(&image_batterie);
    batterie.append(&texte_batterie);
    batterie.set_visible(false);

    let barre_droite = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if !couches {
        barre_droite.append(&bouton_reseau);
        barre_droite.append(&bouton_son);
        barre_droite.append(&batterie);
    }
    let maj_prete = gtk::Label::new(Some(&tr("↻ Mise à jour prête")));
    maj_prete.add_css_class("maj-prete");
    maj_prete.set_tooltip_text(Some(&tr("Elle s'installera toute seule au prochain redémarrage.")));
    maj_prete.set_visible(false);
    if !couches {
        barre_droite.prepend(&maj_prete);
        barre_droite.append(&bouton_reglages);
        barre_droite.append(&bouton_alim);
    }

    let haut = gtk::CenterBox::new();
    haut.set_start_widget(Some(&marque));
    if !couches {
        haut.set_end_widget(Some(&barre_droite));
    }
    let icone_reglages = gtk::Button::new();
    if couches {
        let image = icones::image("r-reglages", 68);
        image.add_css_class("icone-bureau");
        let nom = gtk::Label::new(Some(&tr("Réglages")));
        nom.add_css_class("nom-appli");
        let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
        b.append(&image);
        b.append(&nom);
        icone_reglages.set_child(Some(&b));
        icone_reglages.add_css_class("icone-reglages-bureau");
        icone_reglages.set_tooltip_text(Some(&tr("Réglages : Wi-Fi, son, Bluetooth, écran, imprimantes, code…")));
        icone_reglages.set_valign(gtk::Align::Start);
        icone_reglages.set_margin_top(14);
        icone_reglages.set_margin_end(18);
        haut.set_end_widget(Some(&icone_reglages));
    }
    haut.set_margin_top(10);
    haut.set_margin_start(22);
    haut.set_margin_end(14);

    // --- Horloge ---------------------------------------------------------
    // « Bonjour Giuseppe » : le prénom donné au premier démarrage
    let salut = gtk::Label::new(None);
    salut.add_css_class("salut");
    salut.set_visible(false);
    let heure = gtk::Label::new(Some(&tr("--:--")));
    heure.add_css_class("heure");
    let date = gtk::Label::new(None);
    date.add_css_class("date");
    mettre_a_jour_horloge(&heure, &date);
    mettre_a_jour_salut(&salut);
    {
        let (heure, date) = (heure.clone(), date.clone());
        glib::timeout_add_seconds_local(1, move || {
            mettre_a_jour_horloge(&heure, &date);
            glib::ControlFlow::Continue
        });
        let salut = salut.clone();
        glib::timeout_add_seconds_local(60, move || {
            mettre_a_jour_salut(&salut);
            glib::ControlFlow::Continue
        });
    }

    // --- Recherche -------------------------------------------------------
    let recherche = gtk::Entry::builder()
        .placeholder_text(tr("Rechercher sur le web…"))
        .width_request(620)
        .halign(gtk::Align::Center)
        .build();
    recherche.add_css_class("recherche");
    recherche.set_hexpand(false);
    recherche.set_margin_top(34);
    recherche.set_margin_bottom(44);
    recherche.connect_activate(|e| {
        let q = e.text().trim().to_string();
        if !q.is_empty() {
            let adresse = format!("{}{}", tr::courante().recherche, encoder(&q));
            lancer(&["flashos-web", "--new-window", &adresse]);
        }
        e.set_text("");
    });

    // --- Tuiles ----------------------------------------------------------
    let grille = gtk::Box::new(gtk::Orientation::Horizontal, 46);
    grille.set_halign(gtk::Align::Center);
    let mut points = Vec::new();
    let mut boutons_tuiles = Vec::new();
    let mut tuiles = Vec::new();
    let mut images_tuiles = Vec::new();
    for appli in APPLIS {
        let icone = icones::image(appli.icone, 112);
        images_tuiles.push(icone.clone());
        icone.add_css_class("icone-tuile");
        icone.set_halign(gtk::Align::Center);
        let nom = gtk::Label::new(Some(&tr(appli.nom)));
        nom.add_css_class("nom-appli");
        let point = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        point.add_css_class("point");
        point.set_halign(gtk::Align::Center);
        point.set_opacity(0.0);
        let roue = gtk::Spinner::new();
        roue.set_size_request(46, 46);
        roue.set_halign(gtk::Align::Center);
        roue.set_valign(gtk::Align::Center);
        roue.set_visible(false);
        roue.add_css_class("roue");
        let superpose = gtk::Overlay::new();
        superpose.set_child(Some(&icone));
        superpose.add_overlay(&roue);
        superpose.set_halign(gtk::Align::Center);
        let contenu = gtk::Box::new(gtk::Orientation::Vertical, 10);
        contenu.append(&superpose);
        contenu.append(&nom);
        contenu.append(&point);
        let bouton = gtk::Button::builder().child(&contenu).build();
        bouton.add_css_class("tuile");
        grille.append(&bouton);
        points.push((point, appli));
        tuiles.push((appli, bouton.clone(), roue, nom.clone()));
        boutons_tuiles.push((bouton, appli));
    }

    // Petits écrans : tuiles, espacements et recherche réduits
    {
        let (f, g, r) = (fenetre.downgrade(), grille.clone(), recherche.clone());
        let mut avant = "";
        glib::timeout_add_local(Duration::from_millis(400), move || {
            let Some(f) = f.upgrade() else { return glib::ControlFlow::Break };
            let d = if f.has_css_class("mini") { "mini" } else if f.has_css_class("compact") { "compact" } else { "normal" };
            if d != avant {
                avant = d;
                let (icone, ecart, largeur, haut, bas) = match d {
                    "mini" => (68, 18, 460, 16, 22),
                    "compact" => (88, 30, 560, 26, 34),
                    _ => (112, 46, 620, 34, 44),
                };
                for i in &images_tuiles {
                    i.set_pixel_size(icone);
                }
                g.set_spacing(ecart);
                r.set_width_request(largeur);
                r.set_margin_top(haut);
                r.set_margin_bottom(bas);
            }
            glib::ControlFlow::Continue
        });
    }

    // --- Applis ouvertes -------------------------------------------------
    let bandeau = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bandeau.set_halign(gtk::Align::Center);
    bandeau.set_margin_top(18);
    bandeau.add_css_class("dock");
    bandeau.set_visible(false);

    let aide = gtk::Label::new(Some(&if couches {
        tr("Touche Windows : l'accueil par-dessus les fenêtres  ·  Alt + Tab : changer d'appli  ·  Glisser une fenêtre sur un bord : la moitié de l'écran")
    } else {
        tr("Touche Windows ou Ctrl + Alt + A : revenir à l'accueil  ·  Alt + Tab : changer d'appli")
    }));
    aide.add_css_class("aide");
    aide.set_margin_bottom(14);
    aide.set_valign(gtk::Align::End);

    // --- Assemblage ------------------------------------------------------
    let centre = gtk::Box::new(gtk::Orientation::Vertical, 0);
    centre.set_halign(gtk::Align::Center);
    // Contenu centré verticalement, quelle que soit la taille de l'écran
    centre.set_valign(gtk::Align::Center);
    centre.set_vexpand(true);
    centre.set_margin_bottom(40);
    centre.append(&salut);
    centre.append(&heure);
    centre.append(&date);
    centre.append(&recherche);
    centre.append(&grille);
    // Applications épinglées par l'utilisateur (depuis « Applications ») : une rangée en dessous
    let rangee_epingles = gtk::Box::new(gtk::Orientation::Horizontal, 46);
    rangee_epingles.set_halign(gtk::Align::Center);
    rangee_epingles.set_margin_top(14);
    centre.append(&rangee_epingles);
    {
        let signature = RefCell::new(String::new());
        let rangee = rangee_epingles.clone();
        let maj = move || {
            let installees = applis::installees();
            let liste: Vec<applis::Installee> = applis::epingles()
                .iter()
                .filter_map(|id| installees.iter().find(|a| &a.id == id).cloned())
                .collect();
            let sig = liste.iter().map(|a| format!("{}:{}", a.id, a.nom)).collect::<Vec<_>>().join("|");
            if *signature.borrow() == sig {
                return;
            }
            *signature.borrow_mut() = sig;
            while let Some(e) = rangee.first_child() {
                rangee.remove(&e);
            }
            for a in liste {
                rangee.append(&tuile_epinglee(&a));
            }
            rangee.set_visible(rangee.first_child().is_some());
        };
        maj();
        glib::timeout_add_seconds_local(3, move || {
            maj();
            glib::ControlFlow::Continue
        });
    }

    // --- Mode essai (démarrage depuis la clé USB) ----------------------------
    let reparation = mode_reparation();
    if reparation {
        let texte = gtk::Label::new(Some(&tr("Mode réparation de SoleOS")));
        texte.add_css_class("essai");
        texte.set_margin_top(36);
        let ouvrir = gtk::Button::with_label(&tr("Ouvrir l'outil de réparation"));
        ouvrir.add_css_class("installer");
        ouvrir.set_halign(gtk::Align::Center);
        ouvrir.set_margin_top(12);
        ouvrir.connect_clicked(|_| lancer(&["flashos-installateur"]));
        centre.append(&texte);
        centre.append(&ouvrir);
        // L'outil s'ouvre tout seul
        glib::timeout_add_seconds_local_once(2, || lancer(&["flashos-installateur"]));
    } else if std::path::Path::new("/run/live/medium").exists() {
        let essai = gtk::Label::new(Some(&tr(
            "Vous essayez SoleOS depuis la clé USB : rien n'est modifié sur cet ordinateur.",
        )));
        essai.add_css_class("essai");
        essai.set_margin_top(36);
        let installer = gtk::Button::with_label(&tr("Installer SoleOS sur cet ordinateur"));
        installer.add_css_class("installer");
        installer.set_halign(gtk::Align::Center);
        installer.set_margin_top(12);
        installer.connect_clicked(|_| {
            lancer(&["flashos-installateur"]);
        });
        centre.append(&essai);
        centre.append(&installer);
    }
    // Rail, à gauche, toujours visible : éclair (afficher l'accueil), applis ouvertes,
    // batterie et heure, marche/arrêt. Pas de barre : le bureau reste dégagé.
    let niveau_batterie = Rc::new(Cell::new((100u32, false)));
    let anneau = gtk::DrawingArea::new();
    let texte_anneau = gtk::Label::new(None);
    let feuille: Option<gtk::Window>;
    let rail_objet: Option<Rc<rail::Rail>> = None;
    let mut objets_bureau = None;
    // Barre du bas : Wi-Fi et son (avec leurs bulles)
    let (bouton_reseau_bas, reseau_bas) = bureau::bouton_bas("wifi-off", &tr("Réseau"));
    let (bouton_son_bas, son_bas) = bureau::bouton_bas("son", &tr("Son"));
    let curseur_bas = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 5.0);
    curseur_bas.set_draw_value(false);
    let muet_bas = gtk::Button::with_label(&tr("Couper le son"));
    let (barre_taches, bouton_bureau) = if couches {
        // Bureau v12.5 : barre magique à gauche, barre des fenêtres en bas, panneau de l'éclair
        // Fenêtre du grand panneau Réglages (remplie plus bas ; la bulle du son y renvoie)
        let f_reglages = gtk::Window::builder().application(app).decorated(false).title(tr("Réglages")).build();
        let bas = bureau::barre_bas(app);
        // Wi-Fi : la liste des réseaux dans une bulle au-dessus du bouton
        let wifi_bas = wifi::Panneau::nouveau();
        let bulle_wifi_bas = gtk::Popover::new();
        bulle_wifi_bas.add_css_class("bulle-bas");
        bulle_wifi_bas.set_child(Some(&wifi_bas.racine));
        bulle_wifi_bas.set_parent(&bouton_reseau_bas);
        bulle_wifi_bas.set_position(gtk::PositionType::Top);
        {
            let p = wifi_bas.clone();
            bulle_wifi_bas.connect_show(move |_| p.actualiser());
        }
        // (v13.2) Tuiles Wi-Fi et son : chacune ouvre directement son réglage (le plus utilisé) ;
        // bleues quand c'est allumé, rouges quand c'est coupé. La batterie (tuile + pourcentage)
        // est à côté de l'heure ; un clic ouvre Réglages → Batterie.
        let groupe = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        groupe.add_css_class("groupe-etat");
        groupe.set_valign(gtk::Align::Center);
        // Les curseurs des bulles ne doivent pas élargir le groupe
        groupe.set_hexpand(false);
        let bouton_batterie_bas = gtk::Button::builder().child(&batterie).build();
        bouton_batterie_bas.add_css_class("etat-bas");
        bouton_batterie_bas.add_css_class("tuile-batterie");
        bouton_batterie_bas.set_valign(gtk::Align::Center);
        bouton_batterie_bas.set_hexpand(false);
        bouton_batterie_bas.set_visible(false);
        {
            let bb = bouton_batterie_bas.clone();
            // « visible » (la propriété) : is_visible() tient compte du bouton parent, encore caché
            batterie.connect_visible_notify(move |b| bb.set_visible(b.property::<bool>("visible")));
        }
        bouton_batterie_bas.set_visible(batterie.property::<bool>("visible"));
        // Son : volume et « couper le son »
        let boite_son = gtk::Box::new(gtk::Orientation::Vertical, 10);
        boite_son.set_margin_top(6);
        boite_son.set_margin_bottom(6);
        boite_son.set_margin_start(6);
        boite_son.set_margin_end(6);
        let titre_son = gtk::Label::new(Some(&tr("Volume")));
        titre_son.add_css_class("titre-menu");
        titre_son.set_xalign(0.0);
        boite_son.append(&titre_son);
        let ligne_son = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        curseur_bas.set_hexpand(true);
        curseur_bas.set_size_request(220, -1);
        curseur_bas.set_valign(gtk::Align::Center);
        ligne_son.append(&curseur_bas);
        muet_bas.add_css_class("secondaire");
        muet_bas.add_css_class("petit");
        muet_bas.set_valign(gtk::Align::Center);
        ligne_son.append(&muet_bas);
        boite_son.append(&ligne_son);
        let bulle_son_bas = gtk::Popover::new();
        bulle_son_bas.add_css_class("bulle-bas");
        bulle_son_bas.set_child(Some(&boite_son));
        bulle_son_bas.set_parent(&bouton_son_bas);
        bulle_son_bas.set_position(gtk::PositionType::Top);
        {
            let bl = bulle_son_bas.clone();
            bouton_son_bas.connect_clicked(move |_| bl.popup());
            let bl = bulle_wifi_bas.clone();
            bouton_reseau_bas.connect_clicked(move |_| bl.popup());
            bulle_son_bas.connect_show(|b| bureau::apparition(b));
            bulle_wifi_bas.connect_show(|b| bureau::apparition(b));
            let (pr, f) = (panneau_reglages.clone(), f_reglages.clone());
            bouton_batterie_bas.connect_clicked(move |_| {
                pr.au_debut();
                f.present();
                pr.ouvrir("batterie");
            });
        }
        // Le mode nuit choisi reste d'une session à l'autre
        systeme::mode_nuit_au_demarrage();
        // Sauvegarde automatique (Réglages → Sauvegarde) : contrôle 5 min après l'ouverture de
        // session, puis toutes les heures ; ne fait rien si elle n'est pas due ou si la clé est absente
        {
            let app2 = app.clone();
            let controler = move || {
                if !sauvegarde::due() {
                    return;
                }
                let app3 = app2.clone();
                wifi::en_fond(sauvegarde::sauvegarder, move |r| {
                    let texte = match r {
                        Ok(()) => tr("Sauvegarde automatique terminée"),
                        Err(_) => tr("La sauvegarde automatique n'a pas pu se faire (Réglages → Sauvegarde)"),
                    };
                    bureau::bulle_info(&app3, &texte);
                });
            };
            let c = controler.clone();
            glib::timeout_add_seconds_local_once(300, move || c());
            glib::timeout_add_seconds_local(3600, move || {
                controler();
                glib::ControlFlow::Continue
            });
        }
        maj_prete.set_valign(gtk::Align::Center);
        bas.droite.append(&maj_prete);
        groupe.append(&bouton_reseau_bas);
        groupe.append(&bouton_son_bas);
        bas.droite.append(&groupe);
        // Batterie juste à gauche de l'heure
        bas.droite.append(&bouton_batterie_bas);
        let (magique, boutons, reglages_magique) = bureau::barre_magique(app);
        let panneau = bureau::panneau_eclair(app);
        {
            let e = bas.eclair.clone();
            panneau.fenetre.connect_visible_notify(move |f| {
                if f.is_visible() { e.add_css_class("ouvert") } else { e.remove_css_class("ouvert") }
            });
        }
        // La barre du bas d'abord (toute la largeur), puis la barre magique au-dessus d'elle
        bas.fenetre.present();
        magique.present();
        let b = bas.fenetre.clone();
        let bouton_bureau = bas.eclair.clone();
        objets_bureau = Some((bas.taches.clone(), panneau, boutons, reglages_magique, wifi_bas, bulle_wifi_bas));
        // L'accueil d'avant (heure, recherche, tuiles) laisse la place au fond d'écran
        for w in [haut.upcast_ref::<gtk::Widget>(), aide.upcast_ref()] {
            w.set_visible(false);
        }
        for w in [salut.upcast_ref::<gtk::Widget>(), heure.upcast_ref(), date.upcast_ref(), recherche.upcast_ref(), grille.upcast_ref(), rangee_epingles.upcast_ref()] {
            centre.remove(w);
        }
        // Raccourcis sur le bureau (v13.1) : le contenu du dossier « Bureau », en icônes
        let icones_bureau = raccourcis::Bureau::nouveau(|id| {
            if let Some(a) = applis::installees().into_iter().find(|a| a.id == id) {
                lancer_installee(&a);
            }
        });
        centre.set_halign(gtk::Align::Fill);
        centre.set_valign(gtk::Align::Fill);
        centre.set_hexpand(true);
        centre.set_margin_bottom(0);
        centre.append(&icones_bureau.racine);
        raccourcis::accepter_depots(&centre);
        std::mem::forget(icones_bureau);

        // Réglages : grand panneau au centre, sur un voile (Échap ou clic à côté pour fermer)
        let f = f_reglages.clone();
        f.add_css_class("voile");
        f.add_css_class("flashos");
        f.init_layer_shell();
        f.set_layer(Layer::Overlay);
        f.set_namespace(Some("flashos-reglages"));
        for bord in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            f.set_anchor(bord, true);
        }
        f.set_exclusive_zone(-1);
        f.set_keyboard_mode(KeyboardMode::OnDemand);
        let carte = gtk::Box::new(gtk::Orientation::Vertical, 0);
        carte.add_css_class("feuille");
        carte.set_halign(gtk::Align::Center);
        carte.set_valign(gtk::Align::Center);
        carte.append(&panneau_reglages.racine);
        f.set_child(Some(&carte));
        let touches = gtk::EventControllerKey::new();
        let fk = f.clone();
        touches.connect_key_pressed(move |_, touche, _, _| {
            if touche == gdk::Key::Escape {
                fk.set_visible(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
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
        let fermer = f.clone();
        *panneau_reglages.fermer_cb.borrow_mut() = Some(Box::new(move || fermer.set_visible(false)));
        feuille = Some(f);
        (Some(b), Some(bouton_bureau))
    } else {
        feuille = None;
        centre.append(&bandeau);
        (None, None)
    };

    let contenu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    contenu.append(&haut);
    contenu.append(&theme::defilant(&centre));
    contenu.append(&aide);
    // Bureau : la version, discrète, sur le fond d'écran
    if couches {
        if let Ok(v) = std::fs::read_to_string("/etc/flashos-version") {
            let version = gtk::Label::new(Some(&format!("SoleOS {}", v.trim())));
            version.add_css_class("version-bureau");
            // En haut à droite : jamais sous le texte « SoleOS » des fonds d'écran (en bas à droite)
            version.set_halign(gtk::Align::End);
            version.set_margin_end(18);
            version.set_margin_top(12);
            contenu.prepend(&version);
        }
    }
    fenetre.set_child(Some(&contenu));

    // --- Suivi des fenêtres ouvertes --------------------------------------
    let (gestionnaire, reception) = match suivi {
        Some((g, r)) => (Some(g), Some(r)),
        None => {
            eprintln!("flashos-accueil : suivi des fenêtres indisponible (fonctions réduites)");
            (None, None)
        }
    };

    let menu_choix_mail = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let choix_mail = gtk::Popover::new();
    choix_mail.set_child(Some(&menu_choix_mail));
    let menu_bureautique = gtk::Box::new(gtk::Orientation::Vertical, 8);
    menu_bureautique.add_css_class("choix-bureautique");
    let choix_bureautique = gtk::Popover::new();
    choix_bureautique.set_child(Some(&menu_bureautique));
    let accueil = Rc::new(Accueil {
        choix_mail: choix_mail.clone(),
        choix_bureautique: choix_bureautique.clone(),
        fenetre: fenetre.clone(),
        contenu: contenu.clone(),
        bandeau,
        barre_taches,
        rail: rail_objet.clone(),
        selecteur: gestionnaire.as_ref().filter(|_| couches).map(|g| alttab::Selecteur::nouveau(app, g.clone())),
        devant: Cell::new(None),
        recherche: recherche.clone(),
        bureau_montre: RefCell::new(Vec::new()),
        moi: RefCell::new(std::rc::Weak::new()),
        points,
        tuiles,
        en_cours: RefCell::new(Vec::new()),
        barre: Barre {
            info_reseau,
            info_son,
            interrupteur_wifi,
            niveau_batterie: niveau_batterie.clone(),
            anneau: anneau.clone(),
            texte_anneau: texte_anneau.clone(),
            reseau: image_reseau,
            bouton_reseau,
            bouton_son: if couches { carte_son.clone().upcast() } else { bouton_son.clone().upcast() },
            son: image_son,
            curseur_son: curseur_son.clone(),
            bouton_muet: bouton_muet.clone(),
            maj_en_cours: Cell::new(false),
            batterie,
            image_batterie,
            texte_batterie,
            maj_prete: maj_prete.clone(),
            reseau_bas,
            bouton_reseau_bas,
            son_bas,
            bouton_son_bas,
            curseur_bas: curseur_bas.clone(),
            muet_bas: muet_bas.clone(),
        },
        gestionnaire,
        affichees: RefCell::new(Vec::new()),
        sur_secteur: Cell::new(None),
        prechargee: Cell::new(None),
        prechargement: Cell::new(None),
        bulles: vec![("wifi", objets_bureau.as_ref().map(|o| o.5.clone()).unwrap_or_else(|| bulle_wifi.clone())), ("mail", choix_mail.clone()), ("bureautique", choix_bureautique.clone())],
        reglages: panneau_reglages.clone(),
        bulle_reglages: bulle_reglages.clone(),
        feuille: feuille.clone(),
        nouveau_code_propose: Cell::new(false),
        fenetres_avant: RefCell::new(Vec::new()),
        taches: objets_bureau.as_ref().map(|o| o.0.clone()),
        taches_construites: Cell::new(false),
        panneau_eclair: objets_bureau.as_ref().map(|o| o.1.clone()),
        boutons_magique: objets_bureau.as_ref().map(|o| o.2.clone()).unwrap_or_default(),
        bulle_droite: RefCell::new(None),
    });
    // Bureau v12.5 : barre magique, panneau de l'éclair, Wi-Fi de la barre du bas
    if let Some((_, panneau, boutons, reglages_magique, wifi_bas, _)) = &objets_bureau {
        for (appli, bouton) in boutons {
            let (moi, appli, b) = (Rc::downgrade(&accueil), *appli, bouton.clone());
            bouton.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.lancer_magique(appli, &b);
                }
            });
        }
        if let Some(f) = &feuille {
            let (f, r, moi) = (f.clone(), panneau_reglages.clone(), Rc::downgrade(&accueil));
            reglages_magique.connect_clicked(move |_| {
                if let Some(m) = moi.upgrade() {
                    m.fermer_eclair();
                    m.choix_bureautique.popdown();
                }
                r.au_debut();
                f.present();
            });
        }
        let moi = Rc::downgrade(&accueil);
        panneau.recherche.connect_changed(move |e| {
            if let Some(m) = moi.upgrade() {
                m.remplir_eclair(&e.text());
            }
        });
        let moi = Rc::downgrade(&accueil);
        panneau.recherche.connect_activate(move |e| {
            if let Some(m) = moi.upgrade() {
                m.valider_eclair(&e.text());
            }
        });
        let moi = Rc::downgrade(&accueil);
        let r = panneau.recherche.clone();
        panneau.web.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.chercher_web(&r.text());
            }
        });
        let moi = Rc::downgrade(&accueil);
        *wifi_bas.apres_changement.borrow_mut() = Some(Box::new(move || {
            if let Some(m) = moi.upgrade() {
                m.mettre_a_jour_barre();
            }
        }));
    }
    // Le panneau Réglages ferme les bulles de l'accueil restées ouvertes (Bureautique, Mail)
    if let Some(f) = &feuille {
        let (cb, cm) = (choix_bureautique.clone(), choix_mail.clone());
        f.connect_map(move |_| {
            cb.popdown();
            cm.popdown();
        });
    }
    fonds::appliquer(&fonds::choisi());
    surveiller_fond_gnome();
    // Lanceurs des applications de la boutique adaptés aux fichiers des partages réseau (NAS)
    std::thread::spawn(applis::adapter_lanceurs);
    {
        let a = Rc::downgrade(&accueil);
        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            if let Some(a) = a.upgrade() {
                a.ouvrir_demande();
            }
        });
    }

    // --- Actions ---------------------------------------------------------
    {
        let moi = Rc::downgrade(&accueil);
        *panneau_wifi.apres_changement.borrow_mut() = Some(Box::new(move || {
            if let Some(m) = moi.upgrade() {
                m.mettre_a_jour_barre();
            }
        }));
    }
    for (bouton, appli) in boutons_tuiles {
        if appli.nom == "Bureautique" && bureautique::disponible() {
            choix_bureautique.set_parent(&bouton);
            // Contenu préparé à l'avance (le remplir à l'ouverture change la taille de la bulle,
            // ce qui la referme sur certains compositeurs) et remis à jour quand elle se ferme
            let (m, b, moi) = (menu_bureautique.clone(), choix_bureautique.clone(), Rc::downgrade(&accueil));
            let remplir = Rc::new(move || {
                let moi = moi.clone();
                bureautique::remplir(&m, &b, move || {
                    if let Some(a) = moi.upgrade() {
                        a.commencer_ouverture("Bureautique");
                    }
                });
            });
            remplir();
            choix_bureautique.connect_closed(move |_| {
                let r = remplir.clone();
                glib::idle_add_local_once(move || r());
            });
        }
        if appli.nom == "Mail" && MAIL_AU_CHOIX {
            // Clic droit / appui long : ajouter ou retirer une messagerie
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let bulle = gtk::Popover::new();
            bulle.set_child(Some(&menu));
            bulle.set_parent(&bouton);
            let (m, b) = (menu.clone(), bulle.clone());
            bulle.connect_show(move |_| {
                while let Some(e) = m.first_child() {
                    m.remove(&e);
                }
                m.append(&entree_menu(&tr("＋  Ajouter une messagerie…"), &b, || lancer(&["flashos-applications", "--mail"])));
                for adresse in applis::messageries() {
                    let nom = applis::nom_messagerie(&adresse);
                    let e = entree_menu(&trv("Retirer « {nom} »", &[("nom", &nom)]), &b, move || applis::retirer_messagerie(&adresse));
                    e.add_css_class("attention");
                    m.append(&e);
                }
            });
            attacher_menu_contextuel(&bouton, &bulle);
            // Clic normal avec plusieurs messageries : liste pour choisir laquelle ouvrir
            let (m2, b2) = (menu_choix_mail.clone(), choix_mail.clone());
            choix_mail.set_parent(&bouton);
            choix_mail.connect_show(move |_| {
                while let Some(e) = m2.first_child() {
                    m2.remove(&e);
                }
                let titre = gtk::Label::new(Some(&tr("Ouvrir quelle messagerie ?")));
                titre.add_css_class("titre-menu");
                titre.set_xalign(0.0);
                m2.append(&titre);
                for adresse in applis::messageries() {
                    let nom = applis::nom_messagerie(&adresse);
                    m2.append(&entree_menu(&format!("✉  {nom}"), &b2, move || {
                        if let Some(cmd) = applis::commande_messagerie(&adresse) {
                            let cmd: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
                            lancer(&cmd);
                        }
                    }));
                }
                m2.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                m2.append(&entree_menu(&tr("＋  Ajouter une messagerie…"), &b2, || lancer(&["flashos-applications", "--mail"])));
            });
        }
        let moi = Rc::downgrade(&accueil);
        bouton.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.ouvrir(appli);
            }
        });
    }

    *panneau_reglages.changer_fond.borrow_mut() = Some(Box::new(|id: &str| fonds::appliquer(id)));
    let moi = Rc::downgrade(&accueil);
    *panneau_reglages.console.borrow_mut() = Some(Box::new(move || {
        if let Some(m) = moi.upgrade() {
            m.ouvrir(&CONSOLE);
        }
    }));

    let moi = Rc::downgrade(&accueil);
    curseur_son.connect_value_changed(move |c| {
        if let Some(m) = moi.upgrade() {
            if !m.barre.maj_en_cours.get() {
                systeme::regler_volume(c.value() as u8);
                m.mettre_a_jour_barre();
            }
        }
    });
    let moi = Rc::downgrade(&accueil);
    bouton_muet.connect_clicked(move |_| {
        if let Some(m) = moi.upgrade() {
            let coupe = systeme::volume().map(|(_, c)| c).unwrap_or(false);
            systeme::couper_son(!coupe);
            m.mettre_a_jour_barre();
        }
    });

    // Menu d'alimentation.
    let menu_alim = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let bulle_alim = attacher_bulle(&bouton_alim, &menu_alim);
    // Verrouiller (seulement quand un code a été choisi)
    let verrouiller = entree_menu(&tr("🔒  Verrouiller l'écran"), &bulle_alim, securite::verrouiller);
    menu_alim.append(&verrouiller);
    {
        let v = verrouiller.clone();
        bulle_alim.connect_show(move |_| v.set_visible(std::path::Path::new("/run/flashos/verrouillable").exists()));
    }
    let eteindre = entree_menu(&tr("⏻  Éteindre"), &bulle_alim, || lancer(&["systemctl", "poweroff"]));
    eteindre.add_css_class("danger");
    menu_alim.append(&eteindre);
    menu_alim.append(&entree_menu(&tr("↻  Redémarrer"), &bulle_alim, || lancer(&["systemctl", "reboot"])));
    menu_alim.append(&entree_menu(&tr("☾  Mettre en veille"), &bulle_alim, || lancer(&["systemctl", "suspend"])));
    // (La taille d'affichage est dans Réglages ⚙.)
    // Au démarrage : taille adaptée à l'écran (automatique, ou celle choisie)
    glib::timeout_add_local_once(Duration::from_millis(800), || {
        std::thread::spawn(appareils::appliquer_taille);
    });
    if !std::path::Path::new("/run/live/medium").exists() {
        menu_alim.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let reparer = entree_menu(&tr("⚠  Réparer ou réinitialiser…"), &bulle_alim, || {
            // Le menu se ferme d'abord : l'écran de réparation s'ouvre ensuite, devant
            glib::timeout_add_local_once(Duration::from_millis(250), || {
                lancer(&["flashos-installateur", "--reinitialiser"]);
            });
        });
        reparer.add_css_class("attention");
        menu_alim.append(&reparer);
    }
    // Version de SoleOS (en bas du menu, discrète)
    if let Ok(v) = std::fs::read_to_string("/etc/flashos-version") {
        let version = gtk::Label::new(Some(&format!("SoleOS {}", v.trim())));
        version.add_css_class("titre-menu");
        version.set_xalign(0.0);
        version.set_margin_top(6);
        menu_alim.append(&version);
    }

    // --- Mises à jour ------------------------------------------------------
    let moi = Rc::downgrade(&accueil);
    if let Some(reception) = reception { glib::spawn_future_local(async move {
        while reception.recv().await.is_ok() {
            match moi.upgrade() {
                Some(m) => m.rafraichir(),
                None => break,
            }
        }
    }); }
    accueil.rafraichir();

    accueil.mettre_a_jour_barre();
    let moi = Rc::downgrade(&accueil);
    glib::timeout_add_seconds_local(4, move || match moi.upgrade() {
        Some(m) => {
            m.mettre_a_jour_barre();
            glib::ControlFlow::Continue
        }
        None => glib::ControlFlow::Break,
    });

    // Taille de départ = celle de l'écran (au cas où le plein écran tarderait)
    if let Some((l, h)) = taille_ecran(&fenetre) {
        fenetre.set_default_size(l, h);
    }
    *accueil.moi.borrow_mut() = Rc::downgrade(&accueil);
    if let Some(f) = &feuille {
        let (f, r) = (f.clone(), panneau_reglages.clone());
        icone_reglages.connect_clicked(move |_| {
            r.au_debut();
            f.present();
        });
    }
    fenetre.present();
    if let Some(b) = &bouton_bureau {
        let moi = Rc::downgrade(&accueil);
        b.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                if m.panneau_eclair.is_some() {
                    m.basculer_eclair();
                } else {
                    m.basculer_devant();
                }
            }
        });
    }
    // Son de la barre du bas
    let moi = Rc::downgrade(&accueil);
    curseur_bas.connect_value_changed(move |c| {
        if let Some(m) = moi.upgrade() {
            if !m.barre.maj_en_cours.get() {
                systeme::regler_volume(c.value() as u8);
                m.mettre_a_jour_barre();
            }
        }
    });
    let moi = Rc::downgrade(&accueil);
    muet_bas.connect_clicked(move |_| {
        if let Some(m) = moi.upgrade() {
            let coupe = systeme::volume().map(|(_, c)| c).unwrap_or(false);
            systeme::couper_son(!coupe);
            m.mettre_a_jour_barre();
        }
    });
    // Accueil devant les fenêtres : Échap le remet derrière
    {
        let touches = gtk::EventControllerKey::new();
        let moi = Rc::downgrade(&accueil);
        touches.connect_key_pressed(move |_, touche, _, _| {
            if std::env::var_os("FLASHOS_DEBUG").is_some() {
                eprintln!("accueil : touche {touche:?}");
            }
            if touche == gdk::Key::Escape {
                if let Some(m) = moi.upgrade() {
                    if m.devant.get().is_some() {
                        m.retirer_devant();
                        return glib::Propagation::Stop;
                    }
                }
            }
            glib::Propagation::Proceed
        });
        touches.set_propagation_phase(gtk::PropagationPhase::Capture);
        fenetre.add_controller(touches);
    }
    if !couches {
        // Agrandie (et non « plein écran ») : sinon l'accueil resterait au-dessus des applis.
        fenetre.maximize();
        surveiller_plein_ecran(&fenetre);
    }
    theme::verifier_si_demande(&fenetre);
    fondu(&contenu, 0.0);

    // Premier démarrage après l'installation : assistant de bienvenue (prénom, code, clé de secours).
    // Ouverture avec la clé de secours (code oublié) : choix d'un nouveau code.
    if !reparation && !std::path::Path::new("/run/live/medium").exists() {
        let moi = Rc::downgrade(&accueil);
        wifi::en_fond(securite::etat, move |e| {
            let Some(m) = moi.upgrade() else { return };
            if e.methode == "provisoire" {
                lancer(&["flashos-accueil", "--bienvenue"]);
            } else if e.secours_utilisee {
                m.nouveau_code_propose.set(true);
                lancer(&["flashos-accueil", "--nouveau-code"]);
            }
        });
        // Écran déverrouillé avec la clé de secours : même chose
        let moi = Rc::downgrade(&accueil);
        glib::timeout_add_seconds_local(3, move || {
            let Some(m) = moi.upgrade() else { return glib::ControlFlow::Break };
            let drapeau = std::path::Path::new("/run/flashos/secours-utilisee").exists();
            if drapeau && !m.nouveau_code_propose.get() {
                m.nouveau_code_propose.set(true);
                lancer(&["flashos-accueil", "--nouveau-code"]);
            } else if !drapeau {
                m.nouveau_code_propose.set(false);
            }
            glib::ControlFlow::Continue
        });
    }
    accueil
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    tr::initialiser();
    if args.get(1).map(String::as_str) == Some("--appareils") {
        return glib::ExitCode::from(appareils::diagnostic(&args[2..]) as u8);
    }
    match args.get(1).map(String::as_str) {
        Some("--bienvenue") => return bienvenue::lancer(false),
        // Au début de la session (avant toute fenêtre) : résolution d'origine et taille d'affichage
        Some("--taille") => {
            appareils::appliquer_taille();
            return glib::ExitCode::SUCCESS;
        }
        Some("--nouveau-code") => return bienvenue::lancer(true),
        // Alt + Tab (lancé par le gestionnaire d'affichage) : transmis à l'accueil déjà ouvert
        Some(o @ ("--alttab" | "--alttab-arriere")) => {
            if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") {
                let _ = std::fs::write(PathBuf::from(d).join("flashos-ouvrir"), &o[2..]);
            }
            let app = gtk::Application::builder().application_id(ID_APPLI).build();
            // Pas d'accueil ouvert : rien à faire (on ne le construit pas pour un Alt + Tab)
            app.connect_activate(|app| app.quit());
            return app.run_with_args(&args[..1]);
        }
        // Choix de la langue (ouvert tout seul sur la clé USB, avant l'accueil)
        Some("--langue") => return langue::lancer_choix(),
        // Bureau Plasma (v13) : les Réglages SoleOS dans leur fenêtre (« --reglages [page] »)
        Some("--reglages") => return fenetre_reglages(args.get(2).cloned()),
        // Bureau Plasma (v13) : ce que l'accueil faisait à l'ouverture de la session
        Some("--demarrage") => return demarrage_plasma(),
        // Assistance : liste des fenêtres ouvertes (identifiant d'appli et titre)
        Some("--fenetres") => {
            if let Some(g) = Gestionnaire::demarrer(|| {}) {
                for f in g.liste() {
                    println!("{}\t{}{}{}{}", f.app_id, f.titre, if f.active { "\t(active)" } else { "" }, if f.agrandie { "\t(agrandie)" } else { "" }, if f.plein_ecran { "\t(plein écran)" } else { "" });
                }
            }
            return glib::ExitCode::SUCCESS;
        }
        // Service D-Bus « fond d'écran » (lancé à la demande par le portail : « Définir comme arrière-plan »)
        Some("--portail-fond") => {
            let boucle = glib::MainLoop::new(None, false);
            portail::demarrer(|chemin| {
                if fonds::importer(chemin).is_err() {
                    return false;
                }
                fonds::enregistrer("perso");
                // L'accueil applique le nouveau fond et revient devant
                demander("fond");
                true
            });
            // Le service s'arrête de lui-même au bout de 2 minutes
            let b = boucle.clone();
            glib::timeout_add_seconds_local_once(120, move || b.quit());
            boucle.run();
            return glib::ExitCode::SUCCESS;
        }
        // Dépannage (console) : même chose que Réglages → Navigateur → Remettre à neuf
        Some("--remettre-navigateur") => {
            return match navigateur::remettre_a_neuf() {
                Ok(t) => { println!("{t}"); glib::ExitCode::SUCCESS }
                Err(e) => { eprintln!("{e}"); glib::ExitCode::FAILURE }
            };
        }
        _ => {}
    }
    // Clé USB : la langue et le clavier d'abord
    if args.len() == 1 && langue::a_choisir() {
        return langue::lancer_choix();
    }
    let app = gtk::Application::builder().application_id(ID_APPLI).build();

    app.connect_startup(|_| {
        let style = gtk::CssProvider::new();
        #[allow(deprecated)]
        // Les règles des bulles valent aussi pour le grand panneau des Réglages (.feuille)
        let feuille = regles_pour_feuille(STYLE);
        style.load_from_data(&format!("{STYLE}\n{feuille}\n{STYLE_V12}\n{}\n{}\n{}\n{}", langue::STYLE, alttab::STYLE, bureau::STYLE, raccourcis::STYLE));
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &ecran,
                &style,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    // Premier lancement : construit l'accueil.
    // Lancements suivants (touche Windows) : ramène l'accueil devant.
    let accueil: Rc<RefCell<Option<Rc<Accueil>>>> = Rc::new(RefCell::new(None));
    app.connect_activate(move |app| {
        let mut a = accueil.borrow_mut();
        match a.as_ref() {
            Some(existant) => existant.revenir(),
            None => {
                let (envoi, reception) = async_channel::bounded::<()>(1);
                let suivi = Gestionnaire::demarrer(move || {
                    let _ = envoi.try_send(());
                });
                // Un accueil est déjà affiché (lancé sans D-Bus) : on le ramène et on s'arrête.
                if let Some(g) = &suivi {
                    if let Some(f) = g.liste().iter().find(|f| est_accueil(f)) {
                        g.activer(f.id);
                        app.quit();
                        return;
                    }
                }
                *a = Some(construire(app, suivi.map(|g| (g, reception))));
            }
        }
    });

    app.run()
}
