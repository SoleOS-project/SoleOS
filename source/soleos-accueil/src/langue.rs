//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – choix de la langue et du clavier.
//!
//! - Sur la clé USB, c'est le premier écran (avant l'accueil) : langue, clavier, essai du clavier.
//! - Dans Réglages → Langue et clavier, pour changer plus tard.
//!
//! Le choix est appliqué à tout l'ordinateur par « soleos-systeme langue » (en administrateur) :
//! /run/soleos/langue, /etc/default/locale, /etc/default/keyboard, et sur un SoleOS installé,
//! gardé sur la partition de démarrage (il survit aux mises à jour). Dans la session, le clavier
//! change tout de suite (labwc), et l'accueil se relance dans la nouvelle langue.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::theme;
use crate::tr::{self, n_, tr, Langue, LANGUES};

pub const ID_LANGUE: &str = "fr.soleos.Langue";
const SYSTEME: &str = "/usr/local/sbin/soleos-systeme";

/// Un clavier proposé : disposition XKB, variante, nom affiché
pub struct Clavier {
    pub disposition: &'static str,
    pub variante: &'static str,
    pub nom: &'static str,
}

pub const CLAVIERS: &[Clavier] = &[
    Clavier { disposition: "fr", variante: "", nom: n_("Français (AZERTY)") },
    Clavier { disposition: "fr", variante: "soleos_ltgt", nom: n_("Français (AZERTY) sur clavier QWERTY : < > avec AltGr + W / X") },
    Clavier { disposition: "be", variante: "", nom: n_("Belge (AZERTY)") },
    Clavier { disposition: "ch", variante: "fr", nom: n_("Suisse romand (QWERTZ)") },
    Clavier { disposition: "ca", variante: "", nom: n_("Canadien français") },
    Clavier { disposition: "us", variante: "", nom: n_("Anglais, États-Unis (QWERTY)") },
    Clavier { disposition: "us", variante: "intl", nom: n_("Anglais international (avec accents)") },
    Clavier { disposition: "gb", variante: "", nom: n_("Anglais, Royaume-Uni") },
    Clavier { disposition: "it", variante: "", nom: n_("Italien") },
    Clavier { disposition: "es", variante: "", nom: n_("Espagnol") },
    Clavier { disposition: "latam", variante: "", nom: n_("Espagnol, Amérique latine") },
    Clavier { disposition: "de", variante: "", nom: n_("Allemand (QWERTZ)") },
    Clavier { disposition: "ch", variante: "", nom: n_("Suisse alémanique (QWERTZ)") },
    Clavier { disposition: "pt", variante: "", nom: n_("Portugais") },
    Clavier { disposition: "br", variante: "", nom: n_("Portugais, Brésil") },
];

fn index_clavier(disposition: &str, variante: &str) -> usize {
    CLAVIERS
        .iter()
        // (v1.1) « flashos_ltgt » : nom de la variante avant la v1.1
        .position(|c| c.disposition == disposition && (c.variante == variante || (c.variante == "soleos_ltgt" && variante == "flashos_ltgt")))
        .or_else(|| CLAVIERS.iter().position(|c| c.disposition == disposition))
        .unwrap_or(0)
}

/// Fichier qui porte la langue de l'ordinateur (remplaçable pour les essais)
fn fichier_langue() -> PathBuf {
    std::env::var_os("SOLEOS_FICHIER_LANGUE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(tr::FICHIER_LANGUE_SYSTEME))
}

/// Clé USB (ou réparation) et langue pas encore choisie : on commence par le choix de la langue
pub fn a_choisir() -> bool {
    if std::env::var_os("SOLEOS_TEST_CHOIX_LANGUE").is_some() {
        return true;
    }
    if !std::path::Path::new("/run/live/medium").exists() || fichier_langue().exists() {
        return false;
    }
    // Outil de réparation lancé depuis un SoleOS installé : sa langue et son clavier sont
    // sur la ligne de démarrage (soleos.langue=en soleos.clavier=us:intl) : pas de question
    let ligne = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    // (v1.1) aussi « flashos.langue=… » : outil de réparation lancé par un PC installé avant la v1.1
    let option = |nom: &str| {
        let ancien = nom.replacen("soleos.", "flashos.", 1);
        ligne.split_whitespace().find_map(|m| m.strip_prefix(nom).or_else(|| m.strip_prefix(ancien.as_str()))).map(str::to_string)
    };
    if let Some(l) = option("soleos.langue=").and_then(|c| tr::langue(&c)) {
        let clavier = option("soleos.clavier=").unwrap_or_default();
        let (d, v) = clavier.split_once(':').unwrap_or((clavier.as_str(), ""));
        let d = if d.is_empty() { l.clavier.0 } else { d };
        if appliquer(l, d, v).is_ok() {
            changer_langue_ici(l);
            return false;
        }
    }
    true
}

/// Clavier actuel de l'ordinateur (/etc/default/keyboard)
pub fn clavier_actuel() -> (String, String) {
    let texte = std::fs::read_to_string("/etc/default/keyboard").unwrap_or_default();
    let valeur = |cle: &str| {
        texte
            .lines()
            .find_map(|l| l.trim().strip_prefix(cle).map(|v| v.trim_matches(|c| c == '"' || c == '\'').to_string()))
            .unwrap_or_default()
    };
    let d = valeur("XKBLAYOUT=");
    // Plusieurs dispositions (« fr,us ») : la première
    let d = d.split(',').next().unwrap_or("").to_string();
    let v = valeur("XKBVARIANT=").split(',').next().unwrap_or("").to_string();
    if d.is_empty() { ("fr".into(), String::new()) } else { (d, v) }
}

/// Clavier de la session tout de suite (labwc relit son fichier « environment »)
pub fn appliquer_clavier_session(disposition: &str, variante: &str) {
    if crate::plasma::actif() {
        crate::plasma::clavier(disposition, variante);
        return;
    }
    let Some(maison) = std::env::var_os("HOME") else { return };
    let dossier = PathBuf::from(maison).join(".config/labwc");
    let _ = std::fs::create_dir_all(&dossier);
    let fichier = dossier.join("environment");
    let ancien = std::fs::read_to_string(&fichier).unwrap_or_default();
    let mut lignes: Vec<String> = ancien
        .lines()
        .filter(|l| !l.starts_with("XKB_DEFAULT_LAYOUT=") && !l.starts_with("XKB_DEFAULT_VARIANT="))
        .map(str::to_string)
        .collect();
    lignes.push(format!("XKB_DEFAULT_LAYOUT={disposition}"));
    lignes.push(format!("XKB_DEFAULT_VARIANT={variante}"));
    let _ = std::fs::write(&fichier, lignes.join("\n") + "\n");
    let _ = Command::new("labwc").arg("--reconfigure").status();
}

/// Applique la langue et le clavier à tout l'ordinateur, puis à la session
pub fn appliquer(langue: &Langue, disposition: &str, variante: &str) -> Result<(), String> {
    if let Some(dossier) = std::env::var_os("SOLEOS_TEST_LANGUE") {
        // Essais : pas d'administrateur, on écrit seulement le fichier de langue
        let _ = std::fs::create_dir_all(&dossier);
        std::fs::write(fichier_langue(), langue.code).map_err(|e| e.to_string())?;
    } else {
        let sortie = Command::new("sudo")
            .args(["-n", SYSTEME, "langue", langue.code, disposition, if variante.is_empty() { "-" } else { variante }])
            .output()
            .map_err(|e| e.to_string())?;
        if !sortie.status.success() {
            let e = String::from_utf8_lossy(&sortie.stderr).trim().to_string();
            return Err(if e.is_empty() { tr("Le réglage n'a pas pu être appliqué.") } else { e });
        }
    }
    appliquer_clavier_session(disposition, variante);
    if crate::plasma::actif() {
        crate::plasma::langue(langue.locale, langue.language);
    }
    // Applications lancées par D-Bus (Fichiers…) : nouvelle langue
    let _ = Command::new("dbus-update-activation-environment")
        .args([
            "--systemd".to_string(),
            format!("LANG={}", langue.locale),
            format!("LANGUAGE={}", langue.language),
            format!("GTK_IM_MODULE={}", if langue.code == "zh" { "fcitx" } else { "" }),
            format!("QT_IM_MODULE={}", if langue.code == "zh" { "fcitx" } else { "" }),
        ])
        .status();
    // Écriture du chinois (pinyin) : démarrée ou arrêtée selon la langue
    if langue.code == "zh" {
        let _ = Command::new("soleos-saisie").spawn().map(|mut e| std::thread::spawn(move || e.wait()));
    } else {
        let _ = Command::new("pkill").args(["-u", &uid(), "-x", "fcitx5"]).status();
    }
    // Sur la clé : dossiers personnels (Documents, Images…) nommés dans la nouvelle langue
    if std::path::Path::new("/run/live/medium").exists() {
        let _ = Command::new("xdg-user-dirs-update")
            .arg("--force")
            .env("LANG", langue.locale)
            .env("LC_ALL", langue.locale)
            .env("LANGUAGE", langue.language)
            .status();
    }
    Ok(())
}

fn uid() -> String {
    // SAFETY : getuid n'a pas de condition d'appel
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }.to_string()
}

/// Relance l'accueil (même programme, sans argument) : il démarre dans la nouvelle langue
pub fn relancer_accueil() -> ! {
    use std::os::unix::process::CommandExt;
    // Bureau Plasma : le choix de la langue est lancé par « --demarrage », qui prend la suite
    if crate::plasma::actif() && std::env::args().nth(1).as_deref() == Some("--langue") {
        std::process::exit(0);
    }
    let programme = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/local/bin/soleos-accueil"));
    // Même programme, mêmes options (fenêtre des Réglages du bureau Plasma : « --reglages »)
    let options: Vec<String> = std::env::args().skip(1).filter(|a| a == "--reglages").collect();
    let e = Command::new(programme).args(options).exec();
    eprintln!("soleos-accueil : relance impossible ({e})");
    std::process::exit(1);
}

/// Passe ce programme dans une autre langue (les textes affichés ensuite seront traduits)
fn changer_langue_ici(l: &Langue) {
    // SAFETY : fil principal de l'interface ; aucun autre fil ne lit l'environnement à ce moment
    unsafe {
        std::env::set_var("LANGUAGE", l.language);
        std::env::set_var("LANG", l.locale);
        std::env::remove_var("LC_ALL");
        std::env::remove_var("LC_MESSAGES");
    }
    tr::initialiser_locale();
}

pub const STYLE: &str = r#"
window.bienvenue .entete { padding: 26px 44px 10px 44px; }
.compact window.bienvenue .entete, window.bienvenue.compact .entete { padding: 16px 28px 6px 28px; }
window.bienvenue.mini .entete { padding: 10px 18px 4px 18px; }
.choix-langue dropdown > button, .reglages-langue dropdown > button {
  background: #141A2E; border: 1px solid #2A3352; border-radius: 12px; min-height: 40px; color: #E9ECF5; box-shadow: none;
}
.choix-langue button.principal { padding: 12px 30px; font-size: 17px; }
.choix-langue .titre-langue { font-family: "Space Grotesk", "Inter", sans-serif; font-size: 46px; font-weight: 600; }
.choix-langue .sous-titre { font-size: 18px; color: #A7B0C8; }
button.carte-langue {
  min-width: 190px; min-height: 84px; padding: 10px 16px; border-radius: 18px;
  background: alpha(white, 0.05); border: 2px solid #2A3352; box-shadow: none; color: #f2f4f8;
}
button.carte-langue:hover { background: alpha(white, 0.09); }
button.carte-langue.choisie { border-color: #FFC23D; background: alpha(#FFC23D, 0.10); }
.nom-langue { font-size: 22px; font-weight: 600; }
.bonjour-langue { font-size: 14px; color: #A7B0C8; }
.compact button.carte-langue { min-width: 160px; min-height: 70px; }
.compact .nom-langue { font-size: 19px; }
.mini button.carte-langue { min-width: 130px; min-height: 56px; padding: 6px 10px; }
.mini .nom-langue { font-size: 17px; }
.mini .choix-langue .titre-langue { font-size: 34px; }
.titre-clavier { font-size: 15px; font-weight: 600; color: #A7B0C8; margin-top: 8px; }
entry.essai-clavier { min-height: 44px; font-size: 17px; }
.reglages-langue button.carte-langue { min-width: 120px; min-height: 52px; padding: 6px 10px; }
.reglages-langue .nom-langue { font-size: 16px; }
.reglages-langue .bonjour-langue { font-size: 12px; }
"#;

/// Choix en cours (langue, clavier) et éléments à retraduire quand la langue change
pub struct Selecteur {
    pub racine: gtk::Box,
    pub langue: Cell<usize>,
    pub clavier: Cell<usize>,
    /// Textes à remettre dans la nouvelle langue : (étiquette, texte français)
    textes: RefCell<Vec<(gtk::Label, &'static str)>>,
    liste_claviers: gtk::StringList,
    menu_claviers: gtk::DropDown,
    essai: gtk::Entry,
    boutons: RefCell<Vec<gtk::Button>>,
    /// Appelé après un changement (pour retraduire le reste de la page)
    pub apres: RefCell<Option<Box<dyn Fn()>>>,
    /// Le clavier change aussi tout de suite dans la session (essai du clavier)
    apercu_clavier: bool,
}

impl Selecteur {
    /// `changer_ici` : la page passe tout de suite dans la langue touchée (premier écran)
    pub fn nouveau(langue: usize, clavier: usize, changer_ici: bool, colonnes: u32) -> Rc<Selecteur> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let grille = gtk::FlowBox::new();
        grille.set_selection_mode(gtk::SelectionMode::None);
        grille.set_max_children_per_line(colonnes);
        grille.set_min_children_per_line(2);
        grille.set_column_spacing(12);
        grille.set_row_spacing(12);
        grille.set_homogeneous(true);
        racine.append(&grille);

        let titre_clavier = gtk::Label::new(None);
        titre_clavier.add_css_class("titre-clavier");
        titre_clavier.set_xalign(0.0);
        racine.append(&titre_clavier);
        let liste_claviers = gtk::StringList::new(&[]);
        for c in CLAVIERS {
            liste_claviers.append(&tr(c.nom));
        }
        let menu_claviers = gtk::DropDown::new(Some(liste_claviers.clone()), gtk::Expression::NONE);
        menu_claviers.set_selected(clavier as u32);
        racine.append(&menu_claviers);
        let essai = gtk::Entry::new();
        essai.add_css_class("essai-clavier");
        racine.append(&essai);

        let s = Rc::new(Selecteur {
            racine,
            langue: Cell::new(langue),
            clavier: Cell::new(clavier),
            textes: RefCell::new(vec![(titre_clavier, n_("Clavier"))]),
            liste_claviers,
            menu_claviers: menu_claviers.clone(),
            essai,
            boutons: RefCell::new(Vec::new()),
            apres: RefCell::new(None),
            apercu_clavier: changer_ici,
        });

        for (i, l) in LANGUES.iter().enumerate() {
            let boite = gtk::Box::new(gtk::Orientation::Vertical, 2);
            boite.set_valign(gtk::Align::Center);
            let nom = gtk::Label::new(Some(l.nom));
            nom.add_css_class("nom-langue");
            let bonjour = gtk::Label::new(Some(l.bonjour));
            bonjour.add_css_class("bonjour-langue");
            boite.append(&nom);
            boite.append(&bonjour);
            let b = gtk::Button::builder().child(&boite).build();
            b.add_css_class("carte-langue");
            let moi = Rc::downgrade(&s);
            b.connect_clicked(move |_| {
                if let Some(s) = moi.upgrade() {
                    s.choisir_langue(i, changer_ici);
                }
            });
            grille.insert(&b, -1);
            s.boutons.borrow_mut().push(b);
        }
        let moi = Rc::downgrade(&s);
        menu_claviers.connect_selected_notify(move |m| {
            if let Some(s) = moi.upgrade() {
                let i = m.selected() as usize;
                if i < CLAVIERS.len() && i != s.clavier.get() {
                    s.clavier.set(i);
                    if s.apercu_clavier {
                        appliquer_clavier_session(CLAVIERS[i].disposition, CLAVIERS[i].variante);
                    }
                }
            }
        });
        s.marquer();
        s.retraduire();
        s
    }

    pub fn langue_choisie(&self) -> &'static Langue {
        &LANGUES[self.langue.get()]
    }

    pub fn clavier_choisi(&self) -> &'static Clavier {
        &CLAVIERS[self.clavier.get()]
    }

    fn marquer(&self) {
        for (i, b) in self.boutons.borrow().iter().enumerate() {
            if i == self.langue.get() { b.add_css_class("choisie") } else { b.remove_css_class("choisie") }
        }
    }

    fn choisir_langue(&self, i: usize, changer_ici: bool) {
        self.langue.set(i);
        let l = &LANGUES[i];
        if changer_ici {
            changer_langue_ici(l);
        }
        // Clavier habituel de cette langue
        let k = index_clavier(l.clavier.0, l.clavier.1);
        if k != self.clavier.get() {
            self.clavier.set(k);
            if self.apercu_clavier {
                appliquer_clavier_session(CLAVIERS[k].disposition, CLAVIERS[k].variante);
            }
        }
        self.marquer();
        self.retraduire();
    }

    /// Remet les textes dans la langue en cours
    pub fn retraduire(&self) {
        for (l, t) in self.textes.borrow().iter() {
            l.set_label(&tr(t));
        }
        let n = self.liste_claviers.n_items();
        let noms: Vec<String> = CLAVIERS.iter().map(|c| tr(c.nom)).collect();
        let noms: Vec<&str> = noms.iter().map(String::as_str).collect();
        self.liste_claviers.splice(0, n, &noms);
        self.menu_claviers.set_selected(self.clavier.get() as u32);
        self.essai.set_placeholder_text(Some(&tr("Tapez ici pour essayer le clavier")));
        if let Some(f) = self.apres.borrow().as_ref() {
            f();
        }
    }

    /// Texte de la page à retraduire avec le reste
    pub fn suivre(&self, etiquette: &gtk::Label, texte: &'static str) {
        etiquette.set_label(&tr(texte));
        self.textes.borrow_mut().push((etiquette.clone(), texte));
    }
}

/// Langue proposée au départ : celle déjà choisie, sinon celle de l'environnement
fn langue_de_depart() -> usize {
    let code = std::fs::read_to_string(fichier_langue()).ok().map(|c| c.trim().to_string());
    let code = code.unwrap_or_else(|| tr::courante().code.to_string());
    LANGUES.iter().position(|l| l.code == code).unwrap_or(0)
}

/// Premier écran de la clé USB : choix de la langue et du clavier, puis l'accueil
pub fn lancer_choix() -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(ID_LANGUE).build();
    app.connect_startup(|_| {
        let style = gtk::CssProvider::new();
        #[allow(deprecated)]
        style.load_from_data(STYLE);
        if let Some(ecran) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&ecran, &style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    });
    app.connect_activate(|app| {
        if let Some(f) = app.active_window() {
            f.present();
            return;
        }
        construire_choix(app);
    });
    let nom = std::env::args().next().unwrap_or_else(|| "soleos-accueil".into());
    app.run_with_args(&[nom])
}

fn construire_choix(app: &gtk::Application) {
    theme::charger();
    let fenetre = gtk::ApplicationWindow::builder().application(app).title("SoleOS").default_width(800).default_height(600).build();
    fenetre.add_css_class("bienvenue");
    theme::adapter(&fenetre);

    let depart = langue_de_depart();
    changer_langue_ici(&LANGUES[depart]);
    let (d, v) = clavier_actuel();
    let l0 = &LANGUES[depart];
    // Sur la clé, le clavier n'a pas encore été choisi : celui de la langue proposée
    let k = if a_choisir() { index_clavier(l0.clavier.0, l0.clavier.1) } else { index_clavier(&d, &v) };
    let selecteur = Selecteur::nouveau(depart, k, true, 3);

    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let entete = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    entete.add_css_class("entete");
    entete.append(&theme::marque());
    // Version de SoleOS sur la clé (la même qu'à l'écran de démarrage)
    if let Some(v) = theme::pastille_version() {
        entete.append(&v);
    }
    page.append(&entete);

    let colonne = gtk::Box::new(gtk::Orientation::Vertical, 14);
    colonne.add_css_class("choix-langue");
    colonne.set_halign(gtk::Align::Center);
    colonne.set_valign(gtk::Align::Center);
    colonne.set_vexpand(true);
    colonne.set_margin_start(24);
    colonne.set_margin_end(24);
    // « Bonjour » dans chaque langue, à tour de rôle
    let bonjour = gtk::Label::new(Some(LANGUES[depart].bonjour));
    bonjour.add_css_class("titre-langue");
    bonjour.set_xalign(0.0);
    colonne.append(&bonjour);
    let n = Rc::new(Cell::new(depart));
    let b2 = bonjour.clone();
    glib::timeout_add_local(Duration::from_millis(1600), move || {
        if b2.root().is_none() {
            return glib::ControlFlow::Break;
        }
        n.set((n.get() + 1) % LANGUES.len());
        b2.set_label(LANGUES[n.get()].bonjour);
        glib::ControlFlow::Continue
    });
    let sous = gtk::Label::new(None);
    sous.add_css_class("sous-titre");
    sous.set_xalign(0.0);
    selecteur.suivre(&sous, n_("Choisissez votre langue et votre clavier"));
    colonne.append(&sous);
    colonne.append(&selecteur.racine);
    page.append(&theme::defilant(&colonne));

    let continuer = gtk::Button::with_label(&tr("Continuer"));
    continuer.add_css_class("principal");
    let message = gtk::Label::new(None);
    message.add_css_class("discret");
    message.set_wrap(true);
    let barre = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    barre.add_css_class("barre-actions");
    message.set_hexpand(true);
    message.set_xalign(0.0);
    barre.append(&message);
    barre.append(&continuer);
    page.append(&barre);
    {
        let c = continuer.clone();
        *selecteur.apres.borrow_mut() = Some(Box::new(move || c.set_label(&tr("Continuer"))));
    }
    let (s, m) = (selecteur.clone(), message.clone());
    continuer.connect_clicked(move |b| {
        b.set_sensitive(false);
        let k = s.clavier_choisi();
        match appliquer(s.langue_choisie(), k.disposition, k.variante) {
            Ok(()) => relancer_accueil(),
            Err(e) => {
                m.set_label(&e);
                b.set_sensitive(true);
            }
        }
    });
    fenetre.set_child(Some(&page));
    fenetre.present();
    theme::garder_plein_ecran(&fenetre, true);
    theme::verifier_si_demande(&fenetre);
    continuer.grab_focus();
}

/// Page « Langue et clavier » des Réglages
pub fn page_reglages(page: &gtk::Box) {
    while let Some(e) = page.first_child() {
        page.remove(&e);
    }
    page.add_css_class("reglages-langue");
    let titre = gtk::Label::new(Some(&tr("Langue et clavier")));
    titre.add_css_class("nom-appareil");
    titre.set_xalign(0.0);
    page.append(&titre);
    let (d, v) = clavier_actuel();
    let selecteur = Selecteur::nouveau(langue_de_depart(), index_clavier(&d, &v), false, 3);
    page.append(&selecteur.racine);
    let info = gtk::Label::new(Some(&tr(if crate::plasma::actif() {
        n_("Le clavier change tout de suite. Le bureau et les applications passeront dans la nouvelle langue au prochain démarrage de l'ordinateur.")
    } else {
        n_("L'accueil s'ouvre aussitôt dans la nouvelle langue. Les applications déjà ouvertes la prendront quand vous les rouvrirez.")
    })));
    info.add_css_class("texte-info");
    info.set_wrap(true);
    info.set_xalign(0.0);
    info.set_max_width_chars(46);
    page.append(&info);
    let appliquer_b = gtk::Button::with_label(&tr("Appliquer"));
    appliquer_b.add_css_class("petit");
    appliquer_b.add_css_class("principal");
    appliquer_b.set_halign(gtk::Align::End);
    page.append(&appliquer_b);
    let s = selecteur.clone();
    appliquer_b.connect_clicked(move |b| {
        let k = s.clavier_choisi();
        match appliquer(s.langue_choisie(), k.disposition, k.variante) {
            Ok(()) => relancer_accueil(),
            Err(e) => b.set_tooltip_text(Some(&e)),
        }
    });
    // Garder le sélecteur en vie avec la page
    unsafe {
        page.set_data("selecteur", selecteur);
    }
}
