//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – installateur graphique.
//!
//! 1. Recherche du disque interne et confirmation (nom du disque affiché).
//! 2. Installation automatique, avec diaporama et barre de progression.
//! 3. Fin : « Retirez la clé puis redémarrez ».
//!
//! Le travail sur le disque est fait par /usr/local/sbin/soleos-installer
//! (lancé en administrateur), qui renvoie sa progression ligne par ligne.

#[path = "../icones.rs"]
mod icones;
#[path = "../theme.rs"]
mod theme;
#[path = "../tr.rs"]
mod tr;
use tr::{n_, tr, trv};

use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

const MOTEUR: &str = "/usr/local/sbin/soleos-installer";
const SYSTEME: &str = "/usr/local/sbin/soleos-systeme";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Depuis la clé : installation sur le disque
    Installation,
    /// Démarré sur l'outil de réparation : réparer ou tout effacer
    Reparation,
    /// Depuis SoleOS installé : redémarrer vers l'outil de réparation
    Reinitialiser,
}

fn mode_demande() -> Mode {
    if std::env::args().any(|a| a == "--reinitialiser") {
        return Mode::Reinitialiser;
    }
    let ligne = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    if ligne.contains("soleos.reparer") || ligne.contains("flashos.reparer") || std::env::var_os("SOLEOS_TEST_REPARATION").is_some() {
        Mode::Reparation
    } else {
        Mode::Installation
    }
}

const STYLE: &str = r#"
window.installateur {
  background-image: linear-gradient(115deg, transparent 72%, alpha(#FFC23D, 0.07) 72.2%, transparent 72.6%);
}
.installateur label { color: #f2f4f8; }
.titre { font-size: 40px; }
.sous-titre { font-size: 18px; color: alpha(#f2f4f8, 0.75); }
.carte { padding: 26px 34px; }
.disque { font-size: 22px; font-weight: 600; }
.discret { font-size: 14px; color: alpha(#f2f4f8, 0.6); }
.alerte {
  background: alpha(#e5484d, 0.16); border: 1px solid alpha(#e5484d, 0.45);
  border-radius: 14px; padding: 14px 18px;
}
.alerte label { color: #ffd7d8; font-size: 15px; }
button.principal, button.secondaire {
  border: none; box-shadow: none; outline: none; border-radius: 999px;
  padding: 13px 30px; font-size: 16px; font-weight: 600;
}

.diapo-titre { font-size: 40px; }
.diapo-texte { font-size: 19px; color: alpha(#f2f4f8, 0.78); }
.etape { font-size: 16px; }
.carte-choix { padding: 24px 26px; }
.carte-orange { border: 1px solid alpha(#f39257, 0.55); }
.carte-rouge { border: 1px solid alpha(#e5484d, 0.6); }
.titre-choix { font-size: 22px; font-weight: 600; }
.texte-choix { font-size: 15px; color: alpha(#f2f4f8, 0.78); }

entry.code, passwordentry.code {
  background: alpha(white, 0.08); color: #f2f4f8; border: 1px solid alpha(white, 0.18);
  border-radius: 14px; min-height: 48px; padding: 0 16px; font-size: 20px;
}
.erreur-code { color: #ffb38a; font-size: 16px; }
.afficher-code { color: #e8ecf5; font-size: 16px; }
.afficher-code check { min-width: 20px; min-height: 20px; border: 2px solid #e8ecf5; border-radius: 5px; background: transparent; }
.afficher-code check:checked { background: #FFC23D; border-color: #FFC23D; }
entry.confirmation {
  background: alpha(white, 0.08); color: #f2f4f8; border: 1px solid alpha(#e5484d, 0.5);
  border-radius: 12px; min-height: 40px; padding: 0 14px; font-size: 16px;
}
button:disabled { opacity: 0.4; }
progressbar trough { min-height: 10px; border-radius: 5px; background: alpha(white, 0.12); border: none; }
progressbar progress { min-height: 10px; border-radius: 5px; border: none; }
"#;

/// Diaporama pendant l'installation : (image, titre, texte)
const DIAPOS: &[(&str, &str, &str)] = &[
    ("logo", n_("Bienvenue dans SoleOS"), n_("Rapide et simple.")),
    ("bureautique", n_("Vos documents"), n_("Textes, tableaux et présentations, compatibles Word, Excel et PowerPoint — même sans Internet.")),
    ("mail", n_("Votre messagerie"), n_("Tous vos comptes mail réunis au même endroit.")),
    ("internet", n_("Le web en un clic"), n_("Un navigateur moderne et rapide, avec vos favoris prêts à l'emploi.")),
    ("fichiers", n_("Vos fichiers"), n_("Documents, photos et clés USB, rangés simplement.")),
];

#[derive(Clone)]
struct Disque {
    chemin: String,
    modele: String,
    octets: u64,
    nombre: u32,
    /// Réparation : données chiffrées, code (ou clé de secours) nécessaire
    code: bool,
}

enum Message {
    Etape(String),
    Progres(f64),
    Termine,
    Erreur(String),
}

fn texte_taille(octets: u64) -> String {
    let go = octets as f64 / 1e9;
    if go >= 1000.0 {
        trv("{n} To", &[("n", &tr::decimal(go / 1000.0, 1))])
    } else {
        trv("{n} Go", &[("n", &format!("{go:.0}"))])
    }
}

/// Commande du moteur : en administrateur via sudo (ou remplacée pour les tests).
fn commande_moteur(args: &[&str]) -> Command {
    if let Some(autre) = std::env::var_os("SOLEOS_MOTEUR_TEST") {
        let mut c = Command::new(autre);
        c.args(args);
        return c;
    }
    let mut c = Command::new("sudo");
    c.arg("-n").arg(MOTEUR).args(args);
    c
}

fn detecter(mode: Mode) -> Result<Disque, String> {
    let option = if mode == Mode::Reparation { "--detecter-reparation" } else { "--detecter" };
    let sortie = commande_moteur(&[option]).output().map_err(|e| e.to_string())?;
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let ligne = texte.lines().last().unwrap_or("").trim();
    let champs: Vec<&str> = ligne.split('|').collect();
    match champs.first() {
        Some(&"DISQUE") if champs.len() >= 5 => Ok(Disque {
            chemin: champs[1].to_string(),
            modele: champs[2].to_string(),
            octets: champs[3].parse().unwrap_or(0),
            nombre: champs[4].parse().unwrap_or(1),
            code: champs.get(5).map(|c| c.trim() == "CODE").unwrap_or(false),
        }),
        Some(&"AUCUN") => Err(champs.get(1).map(|t| t.to_string()).unwrap_or_else(|| tr("Aucun disque trouvé."))),
        _ => Err(format!(
            "{}{}",
            tr("Le moteur d'installation n'a pas répondu correctement."),
            if sortie.stderr.is_empty() { String::new() } else { format!("\n{}", String::from_utf8_lossy(&sortie.stderr).trim()) }
        )),
    }
}

fn logo(largeur: i32) -> gtk::Box {
    // Éclair + « SoleOS » (thème Éclair), à peu près à la largeur demandée
    let m = theme::marque_taille((largeur / 6).clamp(24, 60));
    m.set_halign(gtk::Align::Center);
    // Version installée, dans la pastille jaune de l'écran de démarrage
    let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
    b.set_halign(gtk::Align::Center);
    b.append(&m);
    if let Some(v) = theme::pastille_version() {
        v.add_css_class("grande");
        b.append(&v);
    }
    b
}

/// (le texte est traduit ici : on peut passer directement le texte français)
fn etiquette(texte: &str, classe: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&tr(texte)));
    l.add_css_class(classe);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_justify(gtk::Justification::Center);
    l.set_max_width_chars(60);
    l
}

fn colonne(espacement: i32) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, espacement);
    b.set_halign(gtk::Align::Center);
    b.set_valign(gtk::Align::Center);
    b.set_vexpand(true);
    b
}

struct Installateur {
    fenetre: gtk::ApplicationWindow,
    pages: gtk::Stack,
}

impl Installateur {
    fn afficher(&self, page: &impl IsA<gtk::Widget>, nom: &str) {
        if self.pages.child_by_name(nom).is_none() {
            // Contenu qui défile si l'écran est petit : rien n'est jamais coupé
            let p = page.as_ref();
            p.set_margin_start(24);
            p.set_margin_end(24);
            p.set_margin_top(16);
            p.set_margin_bottom(16);
            self.pages.add_named(&theme::defilant(p), Some(nom));
        }
        self.pages.set_visible_child_name(nom);
    }

    fn page_recherche(&self) {
        let c = colonne(24);
        c.append(&logo(360));
        let roue = gtk::Spinner::new();
        roue.set_spinning(true);
        roue.set_size_request(40, 40);
        c.append(&roue);
        c.append(&etiquette(&tr("Recherche du disque de cet ordinateur…"), "sous-titre"));
        self.afficher(&c, "recherche");
    }

    fn page_aucun(self: &Rc<Self>, raison: &str) {
        let c = colonne(22);
        c.append(&logo(300));
        c.append(&etiquette(&tr("SoleOS ne peut pas s'installer ici"), "titre"));
        c.append(&etiquette(raison, "sous-titre"));
        c.append(&etiquette(
            "Sur certains ordinateurs (Dell, HP, Lenovo…), le disque n'est visible qu'en mode « AHCI » :\n\
             au démarrage, entrez dans le BIOS (F2 ou Échap) et changez « RAID » en « AHCI » dans le stockage.",
            "discret",
        ));
        let fermer = gtk::Button::with_label(&tr("Fermer"));
        fermer.add_css_class("secondaire");
        fermer.set_halign(gtk::Align::Center);
        let f = self.fenetre.clone();
        fermer.connect_clicked(move |_| f.close());
        c.append(&fermer);
        self.afficher(&c, "aucun");
    }

    /// Conditions d'utilisation (SoleOS v1) : à accepter avant d'installer, sinon on arrête
    fn page_conditions(self: &Rc<Self>, d: Disque) {
        let c = colonne(20);
        c.append(&logo(260));
        c.append(&etiquette(&tr("Avant d'installer SoleOS"), "titre"));
        let carte = gtk::Box::new(gtk::Orientation::Vertical, 10);
        carte.add_css_class("carte");
        carte.set_size_request(620, -1);
        for texte in [
            n_("SoleOS est gratuit pour les particuliers, les associations, les écoles, les administrations et l'usage interne des entreprises. Toute utilisation commerciale demande une licence de GDH."),
            n_("SoleOS est fourni « tel quel », sans aucune garantie : ni de bon fonctionnement, ni d'adaptation à un usage particulier, ni de compatibilité avec votre matériel."),
            n_("L'installation efface le disque choisi. Sauvegardez vos données avant de continuer."),
            n_("Vous utilisez SoleOS sous votre seule responsabilité. Dans toute la mesure permise par la loi, GDH, son créateur et les contributeurs ne pourront être tenus responsables d'aucun dommage : perte de données, panne, perte d'exploitation, ou problème lié aux logiciels tiers et aux sites visités."),
            n_("« Signaler un problème » ne vaut pas engagement d'assistance ni de délai de correction."),
        ] {
            let l = etiquette(&format!("•  {}", tr(texte)), "sous-titre");
            l.set_justify(gtk::Justification::Left);
            l.set_xalign(0.0);
            l.set_max_width_chars(72);
            carte.append(&l);
        }
        let lire = gtk::Button::with_label(&tr("Lire la licence complète"));
        lire.add_css_class("secondaire");
        lire.set_halign(gtk::Align::Start);
        lire.connect_clicked(|_| {
            let _ = Command::new("gio").args(["open", "/usr/share/doc/soleos/LICENSE"]).spawn();
        });
        carte.append(&lire);
        c.append(&carte);
        let accepte = gtk::CheckButton::with_label(&tr("J'ai lu et j'accepte ces conditions"));
        accepte.set_halign(gtk::Align::Center);
        accepte.add_css_class("case-conditions");
        c.append(&accepte);
        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        boutons.set_halign(gtk::Align::Center);
        let arreter = gtk::Button::with_label(&tr("Arrêter l'installation"));
        arreter.add_css_class("secondaire");
        let continuer = gtk::Button::with_label(&tr("Continuer"));
        continuer.add_css_class("principal");
        continuer.set_sensitive(false);
        boutons.append(&arreter);
        boutons.append(&continuer);
        c.append(&boutons);
        {
            let co = continuer.clone();
            accepte.connect_toggled(move |a| co.set_sensitive(a.is_active()));
        }
        let f = self.fenetre.clone();
        arreter.connect_clicked(move |_| f.close());
        let moi = Rc::downgrade(self);
        continuer.connect_clicked(move |_| {
            // Trace de l'acceptation (version et date), gardée sur l'ordinateur installé
            let version = std::fs::read_to_string("/etc/soleos-version").unwrap_or_default();
            let date = glib::DateTime::now_local().ok().and_then(|d| d.format("%Y-%m-%d %H:%M").ok()).map(|s| s.to_string()).unwrap_or_default();
            let _ = std::fs::write("/tmp/soleos-acceptation", format!("SoleOS {} : conditions d'utilisation acceptées le {}\n", version.trim(), date));
            if let Some(m) = moi.upgrade() {
                m.page_confirmation(d.clone());
            }
        });
        self.afficher(&c, "conditions");
    }

    fn page_confirmation(self: &Rc<Self>, d: Disque) {
        let c = colonne(26);
        c.append(&logo(300));
        let version = std::fs::read_to_string("/etc/soleos-version").map(|v| format!(" {}", v.trim())).unwrap_or_default();
        c.append(&etiquette(&trv("Installer SoleOS{version} sur cet ordinateur", &[("version", &version)]), "titre"));

        let carte = gtk::Box::new(gtk::Orientation::Vertical, 12);
        carte.add_css_class("carte");
        carte.set_size_request(560, -1);
        carte.append(&etiquette(&tr("SoleOS va s'installer sur ce disque :"), "sous-titre"));
        carte.append(&etiquette(&format!("💽  {}  —  {}", d.modele, texte_taille(d.octets)), "disque"));
        if d.nombre > 1 {
            carte.append(&etiquette(
                &trv("(le plus grand des {n} disques de cet ordinateur)", &[("n", &d.nombre.to_string())]),
                "discret",
            ));
        }
        let alerte = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        alerte.add_css_class("alerte");
        let texte_alerte = etiquette(
            "⚠  Toutes les données de ce disque seront définitivement effacées, y compris Windows et vos fichiers.",
            "alerte-texte",
        );
        texte_alerte.set_justify(gtk::Justification::Left);
        texte_alerte.set_xalign(0.0);
        texte_alerte.set_hexpand(true);
        alerte.append(&texte_alerte);
        carte.append(&alerte);
        c.append(&carte);

        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        boutons.set_halign(gtk::Align::Center);
        let annuler = gtk::Button::with_label(&tr("Annuler"));
        annuler.add_css_class("secondaire");
        let installer = gtk::Button::with_label(&tr("Effacer et installer SoleOS"));
        installer.add_css_class("principal");
        installer.add_css_class("danger");
        boutons.append(&annuler);
        boutons.append(&installer);
        c.append(&boutons);
        c.append(&etiquette(
            "Pensez à sauvegarder vos fichiers importants avant de continuer.",
            "discret",
        ));

        let f = self.fenetre.clone();
        annuler.connect_clicked(move |_| f.close());
        let moi = Rc::downgrade(self);
        installer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.page_installation(d.clone());
            }
        });
        self.afficher(&c, "confirmation");
    }

    fn page_installation(self: &Rc<Self>, d: Disque) {
        self.page_travail(vec!["--installer".into(), d.chemin], "Démarrage de l'installation…",
            "N'éteignez pas l'ordinateur et ne retirez pas la clé USB.", false, None, None);
    }

    /// Réparer avec des données chiffrées : le code (ou la clé de secours) est demandé
    fn page_code(self: &Rc<Self>, d: Disque, erreur: Option<&str>) {
        let c = colonne(22);
        c.append(&logo(300));
        c.append(&etiquette(&tr("Réparer SoleOS"), "titre"));
        c.append(&etiquette(
            "Vos fichiers sont protégés. Pour les garder pendant la réparation,\ntapez votre code SoleOS (ou votre clé de secours).",
            "sous-titre",
        ));
        let saisie = gtk::Entry::new();
        saisie.set_visibility(false);
        saisie.set_input_purpose(gtk::InputPurpose::Password);
        saisie.add_css_class("code");
        saisie.set_size_request(460, -1);
        saisie.set_halign(gtk::Align::Center);
        c.append(&saisie);
        // Bouton bien visible pour vérifier ce qu'on tape
        let afficher = gtk::CheckButton::with_label(&tr("Afficher ce que je tape"));
        afficher.set_halign(gtk::Align::Center);
        afficher.add_css_class("afficher-code");
        {
            let saisie = saisie.clone();
            afficher.connect_toggled(move |b| saisie.set_visibility(b.is_active()));
        }
        c.append(&afficher);
        let message = etiquette(erreur.unwrap_or(""), "erreur-code");
        message.set_visible(erreur.is_some());
        c.append(&message);
        // Rangée du haut AZERTY tapée sans Maj : on prévient tout de suite
        let conseil = etiquette(&tr("Pour taper les chiffres : touche Maj (⇧) ou pavé numérique."), "erreur-code");
        conseil.set_visible(false);
        c.append(&conseil);
        {
            let conseil = conseil.clone();
            saisie.connect_changed(move |e| {
                conseil.set_visible(e.text().chars().any(|c| "&é\"'(è_çà".contains(c)));
            });
        }
        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        boutons.set_halign(gtk::Align::Center);
        let retour = gtk::Button::with_label(&tr("Retour"));
        retour.add_css_class("secondaire");
        let reparer = gtk::Button::with_label(&tr("Réparer"));
        reparer.add_css_class("principal");
        reparer.add_css_class("orange");
        reparer.set_sensitive(false);
        boutons.append(&retour);
        boutons.append(&reparer);
        c.append(&boutons);
        c.append(&etiquette(
            "Code oublié et clé de secours perdue : seul « Tout effacer » reste possible.",
            "discret",
        ));
        let r2 = reparer.clone();
        saisie.connect_changed(move |e| r2.set_sensitive(!e.text().trim().is_empty()));
        let lancer = {
            let moi = Rc::downgrade(self);
            let (saisie, d) = (saisie.clone(), d.clone());
            move || {
                let Some(m) = moi.upgrade() else { return };
                let secret = saisie.text().trim().to_string();
                if secret.is_empty() {
                    return;
                }
                saisie.set_text("");
                m.page_travail(vec!["--reparer".into(), "systeme".into(), d.chemin.clone()],
                    "Démarrage de la réparation…", "N'éteignez pas l'ordinateur.", true, Some(secret), Some(d.clone()));
            }
        };
        let l2 = lancer.clone();
        reparer.connect_clicked(move |_| l2());
        saisie.connect_activate(move |_| lancer());
        let moi = Rc::downgrade(self);
        retour.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.pages.set_visible_child_name("choix-reparation");
            }
        });
        // Page recréée à chaque fois (message d'erreur à jour)
        if let Some(ancienne) = self.pages.child_by_name("code") {
            self.pages.remove(&ancienne);
        }
        self.afficher(&c, "code");
        saisie.grab_focus();
    }

    fn page_travail(self: &Rc<Self>, arguments: Vec<String>, premier_texte: &str, consigne: &str, reparation: bool,
                    secret: Option<String>, disque_code: Option<Disque>) {
        // Empêche de fermer la fenêtre pendant l'installation
        self.fenetre.connect_close_request(|_| glib::Propagation::Stop);

        let diapos = gtk::Stack::new();
        diapos.set_transition_type(gtk::StackTransitionType::Crossfade);
        diapos.set_transition_duration(900);
        diapos.set_vexpand(true);
        for (i, (image, titre, texte)) in DIAPOS.iter().enumerate() {
            let c = colonne(22);
            if *image == "logo" {
                c.append(&logo(520));
            } else {
                let img = icones::image(&format!("tuile-{image}"), 168);
                c.append(&img);
            }
            c.append(&etiquette(titre, "diapo-titre"));
            c.append(&etiquette(texte, "diapo-texte"));
            diapos.add_named(&c, Some(&format!("d{i}")));
        }
        let index = Rc::new(RefCell::new(0usize));
        let d2 = diapos.clone();
        glib::timeout_add_seconds_local(7, move || {
            let mut i = index.borrow_mut();
            *i = (*i + 1) % DIAPOS.len();
            d2.set_visible_child_name(&format!("d{}", *i));
            glib::ControlFlow::Continue
        });

        let etape = etiquette(premier_texte, "etape");
        let barre = gtk::ProgressBar::new();
        barre.set_size_request(640, -1);
        barre.set_halign(gtk::Align::Center);
        let bas = gtk::Box::new(gtk::Orientation::Vertical, 12);
        bas.set_margin_bottom(46);
        bas.append(&etape);
        bas.append(&barre);
        bas.append(&etiquette(consigne, "discret"));

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.append(&diapos);
        page.append(&bas);
        if let Some(ancienne) = self.pages.child_by_name("installation") {
            self.pages.remove(&ancienne);
        }
        self.afficher(&page, "installation");

        // Lancement du moteur dans un fil à part
        let (envoi, reception) = async_channel::unbounded::<Message>();
        std::thread::spawn(move || {
            let args: Vec<&str> = arguments.iter().map(|a| a.as_str()).collect();
            let enfant = commande_moteur(&args)
                .stdin(if secret.is_some() { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let mut enfant = match enfant {
                Ok(e) => e,
                Err(e) => {
                    let _ = envoi.send_blocking(Message::Erreur(trv("Lancement impossible : {e}", &[("e", &e.to_string())])));
                    return;
                }
            };
            // Code de l'utilisateur : transmis au moteur par l'entrée standard (jamais en argument)
            if let (Some(s), Some(mut entree)) = (secret, enfant.stdin.take()) {
                use std::io::Write;
                let _ = writeln!(entree, "{s}");
            }
            let mut fini = false;
            if let Some(sortie) = enfant.stdout.take() {
                for ligne in BufReader::new(sortie).lines().map_while(Result::ok) {
                    let mut c = ligne.splitn(2, '|');
                    let msg = match (c.next(), c.next()) {
                        (Some("ETAPE"), Some(t)) => Message::Etape(t.to_string()),
                        (Some("PROGRES"), Some(p)) => Message::Progres(p.trim().parse::<f64>().unwrap_or(0.0) / 100.0),
                        (Some("ERREUR"), Some(t)) => { fini = true; Message::Erreur(t.to_string()) }
                        (Some("TERMINE"), _) => { fini = true; Message::Termine }
                        _ => continue,
                    };
                    let _ = envoi.send_blocking(msg);
                }
            }
            let statut = enfant.wait();
            if !fini {
                let code = statut.map(|s| s.to_string()).unwrap_or_default();
                let _ = envoi.send_blocking(Message::Erreur(trv("Le moteur s'est arrêté de façon inattendue ({code}).", &[("code", &code)])));
            }
        });

        let moi = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(msg) = reception.recv().await {
                let Some(m) = moi.upgrade() else { break };
                match msg {
                    Message::Etape(t) => etape.set_label(&t),
                    Message::Progres(p) => barre.set_fraction(p.clamp(0.0, 1.0)),
                    Message::Termine => { m.page_fin(reparation); break; }
                    // Code refusé : retour à la saisie (rien n'a été modifié sur le disque)
                    Message::Erreur(t) if t == "CODE" || t == "BLOQUE" => {
                        let texte = if t == "CODE" {
                            tr("Code incorrect. Réessayez, ou tapez votre clé de secours.")
                        } else {
                            tr("Trop d'essais : la puce de sécurité patiente quelques minutes. Vous pouvez aussi taper votre clé de secours.")
                        };
                        match disque_code.clone() {
                            Some(d) => m.page_code(d, Some(&texte)),
                            None => m.page_erreur(&texte),
                        }
                        break;
                    }
                    Message::Erreur(t) => { m.page_erreur(&t); break; }
                }
            }
        });
    }

    fn page_fin(self: &Rc<Self>, reparation: bool) {
        let c = colonne(24);
        c.append(&logo(420));
        if reparation {
            c.append(&etiquette(&tr("SoleOS est remis à neuf !"), "titre"));
            c.append(&etiquette(&tr("Redémarrez l'ordinateur pour l'utiliser."), "sous-titre"));
        } else {
            c.append(&etiquette(&tr("SoleOS est installé !"), "titre"));
            c.append(&etiquette(&tr("Retirez la clé USB, puis redémarrez l'ordinateur."), "sous-titre"));
        }
        let redemarrer = gtk::Button::with_label(&tr("Redémarrer maintenant"));
        redemarrer.add_css_class("principal");
        redemarrer.set_halign(gtk::Align::Center);
        redemarrer.connect_clicked(|_| {
            let _ = Command::new("systemctl").arg("reboot").spawn();
        });
        c.append(&redemarrer);
        self.afficher(&c, "fin");
    }

    /// Démarré sur l'outil de réparation : deux choix
    fn page_choix_reparation(self: &Rc<Self>, d: Disque) {
        let c = colonne(22);
        c.append(&logo(300));
        c.append(&etiquette(&tr("Réparer ou réinitialiser SoleOS"), "titre"));

        let choix = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        choix.set_halign(gtk::Align::Center);

        // Réparer
        let orange = gtk::Box::new(gtk::Orientation::Vertical, 12);
        orange.add_css_class("carte-choix");
        orange.add_css_class("carte-orange");
        orange.set_size_request(380, -1);
        orange.append(&etiquette(&tr("⚙  Réparer SoleOS"), "titre-choix"));
        orange.append(&etiquette(
            &format!(
                "{}{}",
                tr("Le système est remis à neuf. Vos fichiers, vos applications, vos réglages et vos réseaux Wi-Fi sont conservés.\n\nLe navigateur est remis à zéro (extensions et réglages) ; vos favoris restent."),
                if d.code { format!("\n\n{}", tr("Votre code (ou votre clé de secours) vous sera demandé.")) } else { String::new() }
            ),
            "texte-choix",
        ));
        let reparer = gtk::Button::with_label(&tr("Réparer"));
        reparer.add_css_class("principal");
        reparer.add_css_class("orange");
        reparer.set_valign(gtk::Align::End);
        reparer.set_vexpand(true);
        orange.append(&reparer);

        // Tout effacer
        let rouge = gtk::Box::new(gtk::Orientation::Vertical, 12);
        rouge.add_css_class("carte-choix");
        rouge.add_css_class("carte-rouge");
        rouge.set_size_request(380, -1);
        rouge.append(&etiquette(&tr("⚠  Tout effacer"), "titre-choix"));
        rouge.append(&etiquette(
            "Retour au premier jour : tous vos fichiers, photos, applications, réglages et comptes seront supprimés définitivement.\n\nAucun code n'est demandé : à utiliser aussi si vous avez oublié votre code et perdu votre clé de secours.",
            "texte-choix",
        ));
        let saisie = gtk::Entry::builder().placeholder_text(tr("Tapez EFFACER pour confirmer")).build();
        saisie.add_css_class("confirmation");
        let effacer = gtk::Button::with_label(&tr("Tout effacer"));
        effacer.add_css_class("principal");
        effacer.add_css_class("danger");
        effacer.set_sensitive(false);
        let e2 = effacer.clone();
        // Mot de confirmation dans la langue choisie (EFFACER, ERASE…) ; « EFFACER » marche toujours
        let mot = tr("EFFACER").to_uppercase();
        saisie.connect_changed(move |s| {
            let t = s.text().trim().to_uppercase();
            e2.set_sensitive(t == mot || t == "EFFACER");
        });
        rouge.append(&saisie);
        rouge.append(&effacer);

        choix.append(&orange);
        choix.append(&rouge);
        c.append(&choix);
        c.append(&etiquette(&trv("Disque : {modele}  —  {taille}", &[("modele", &d.modele), ("taille", &texte_taille(d.octets))]), "discret"));

        let annuler = gtk::Button::with_label(&tr("Annuler et redémarrer normalement"));
        annuler.add_css_class("secondaire");
        annuler.set_halign(gtk::Align::Center);
        annuler.connect_clicked(|_| { let _ = Command::new("systemctl").arg("reboot").spawn(); });
        c.append(&annuler);

        let moi = Rc::downgrade(self);
        let disque = d.chemin.clone();
        let dc = d.clone();
        reparer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                if dc.code {
                    m.page_code(dc.clone(), None);
                } else {
                    m.page_travail(vec!["--reparer".into(), "systeme".into(), disque.clone()],
                        "Démarrage de la réparation…", "N'éteignez pas l'ordinateur.", true, None, None);
                }
            }
        });
        let moi = Rc::downgrade(self);
        let disque = d.chemin.clone();
        effacer.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                m.page_travail(vec!["--reparer".into(), "tout".into(), disque.clone()],
                    "Démarrage de la réinitialisation…", "N'éteignez pas l'ordinateur.", true, None, None);
            }
        });
        self.afficher(&c, "choix-reparation");
    }

    /// Depuis SoleOS installé : redémarrer sur l'outil de réparation
    fn page_reinitialiser(self: &Rc<Self>) {
        let c = colonne(24);
        c.append(&logo(300));
        c.append(&etiquette(&tr("Réparer ou réinitialiser SoleOS"), "titre"));
        let carte = gtk::Box::new(gtk::Orientation::Vertical, 12);
        carte.add_css_class("carte");
        carte.set_size_request(600, -1);
        carte.append(&etiquette(
            "L'ordinateur va redémarrer sur l'outil de réparation de SoleOS.\nVous pourrez y choisir :",
            "sous-titre",
        ));
        carte.append(&etiquette(&tr("⚙  Réparer — le système est remis à neuf, vos fichiers et vos applications sont conservés"), "texte-choix"));
        carte.append(&etiquette(&tr("⚠  Tout effacer — retour au premier jour"), "texte-choix"));
        carte.append(&etiquette(&tr("Pensez à enregistrer vos documents ouverts."), "discret"));
        c.append(&carte);
        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        boutons.set_halign(gtk::Align::Center);
        let annuler = gtk::Button::with_label(&tr("Annuler"));
        annuler.add_css_class("secondaire");
        let continuer = gtk::Button::with_label(&tr("Redémarrer vers la réparation"));
        continuer.add_css_class("principal");
        continuer.add_css_class("orange");
        boutons.append(&annuler);
        boutons.append(&continuer);
        c.append(&boutons);
        let f = self.fenetre.clone();
        annuler.connect_clicked(move |_| f.close());
        let moi = Rc::downgrade(self);
        continuer.connect_clicked(move |_| {
            let Some(m) = moi.upgrade() else { return };
            let commande = std::env::var("SOLEOS_SYSTEME_TEST").unwrap_or_default();
            let resultat = if commande.is_empty() {
                Command::new("sudo").args(["-n", SYSTEME, "preparer-reparation"]).status()
            } else {
                Command::new(commande).arg("preparer-reparation").status()
            };
            match resultat {
                Ok(s) if s.success() => { let _ = Command::new("systemctl").arg("reboot").spawn(); }
                _ => m.page_erreur(&tr("Impossible de préparer l'outil de réparation.")),
            }
        });
        self.afficher(&c, "reinitialiser");
    }

    fn page_erreur(self: &Rc<Self>, texte: &str) {
        let c = colonne(22);
        c.append(&logo(300));
        c.append(&etiquette(&tr("L'opération n'a pas pu se terminer"), "titre"));
        c.append(&etiquette(texte, "sous-titre"));
        c.append(&etiquette(
            "Détails : /var/log/soleos-installation.log",
            "discret",
        ));
        let fermer = gtk::Button::with_label(&tr("Fermer"));
        fermer.add_css_class("secondaire");
        fermer.set_halign(gtk::Align::Center);
        let f = self.fenetre.clone();
        fermer.connect_clicked(move |_| {
            // Après une erreur, la fermeture est de nouveau permise
            f.destroy();
        });
        c.append(&fermer);
        self.afficher(&c, "erreur");
    }
}

fn construire(app: &gtk::Application) {
    let fenetre = gtk::ApplicationWindow::builder()
        .application(app)
        .title(tr("Installer SoleOS"))
        .decorated(false)
        .default_width(800)
        .default_height(600)
        .build();
    fenetre.add_css_class("installateur");
    theme::adapter(&fenetre);
    let pages = gtk::Stack::new();
    pages.set_transition_type(gtk::StackTransitionType::Crossfade);
    pages.set_transition_duration(400);
    fenetre.set_child(Some(&pages));

    let inst = Rc::new(Installateur { fenetre: fenetre.clone(), pages });
    // L'installateur vit aussi longtemps que sa fenêtre
    let garde = RefCell::new(Some(inst.clone()));
    fenetre.connect_destroy(move |_| { garde.borrow_mut().take(); });
    let mode = mode_demande();
    // Plein écran, centré, quelle que soit la taille de l'écran (demandé une fois la fenêtre affichée :
    // avant, certains gestionnaires d'affichage l'ignorent et la fenêtre reste petite dans un coin)
    // Taille de l'écran dès le départ, puis « agrandie » une fois affichée
    // (avant l'affichage, certains gestionnaires d'affichage ignorent la demande).
    // Plein écran quel que soit l'écran, redemandé tant qu'il n'est pas obtenu
    theme::garder_plein_ecran(&fenetre, true);
    fenetre.present();
    theme::verifier_si_demande(&fenetre);
    if mode == Mode::Reinitialiser {
        fenetre.set_title(Some(&tr("Réparer SoleOS")));
        inst.page_reinitialiser();
        return;
    }
    inst.page_recherche();

    // Recherche du disque (petit délai pour laisser l'écran s'afficher)
    let (envoi, reception) = async_channel::bounded::<Result<Disque, String>>(1);
    glib::timeout_add_local_once(Duration::from_millis(600), move || {
        std::thread::spawn(move || {
            let _ = envoi.send_blocking(detecter(mode));
        });
    });
    let moi = Rc::downgrade(&inst);
    glib::spawn_future_local(async move {
        if let (Ok(resultat), Some(m)) = (reception.recv().await, moi.upgrade()) {
            match (resultat, mode) {
                (Ok(d), Mode::Reparation) => {
                    let test_code = std::env::var_os("SOLEOS_TEST_PAGE_CODE").is_some();
                    m.page_choix_reparation(d.clone());
                    // Banc de captures : page du code directement (avec message d'erreur)
                    if test_code {
                        m.page_code(d, Some(&tr("Code incorrect. Réessayez, ou tapez votre clé de secours.")));
                    }
                }
                (Ok(d), _) => m.page_conditions(d),
                (Err(raison), _) => m.page_aucun(&raison),
            }
        }
    });
}

fn main() -> glib::ExitCode {
    tr::initialiser();
    let app = gtk::Application::builder().application_id("fr.soleos.Installateur").build();
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
        } else {
            construire(app);
        }
    });
    // Nos options (--reinitialiser) sont lues par mode_demande() : GTK ne doit pas les voir,
    // sinon il refuse l'option inconnue et le programme se ferme sans rien afficher.
    let nom = std::env::args().next().unwrap_or_else(|| "soleos-installateur".into());
    app.run_with_args(&[nom])
}
