//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! « Signaler un problème » (v13) : l'utilisateur décrit en une phrase ce qui ne marche pas ;
//! SoleOS prépare un message dans Mail (Thunderbird) pour l'assistance, avec le relevé
//! d'assistance en pièce jointe (aucun mot de passe ni contenu personnel). L'utilisateur le
//! relit et l'envoie de sa propre boîte. La correction arrive ensuite chez tout le monde par la
//! mise à jour automatique.

use std::process::Command;
use std::rc::Rc;

use gtk::prelude::*;

use crate::tr::{tr, trv};
use crate::wifi::en_fond;

/// Adresse de l'assistance SoleOS
pub const ASSISTANCE: &str = "gdarco7@gmail.com";

pub struct Signaler {
    pub racine: gtk::Box,
    texte: gtk::TextView,
    info: gtk::Label,
    bouton: gtk::Button,
}

/// Valeur pour « thunderbird -compose » : entre apostrophes, sans apostrophe à l'intérieur
fn valeur(t: &str) -> String {
    t.replace('\'', "’")
}

impl Signaler {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let titre = gtk::Label::new(Some(&tr("Signaler un problème")));
        titre.add_css_class("nom-appareil");
        titre.set_xalign(0.0);
        racine.append(&titre);
        let explication = gtk::Label::new(Some(&tr(
            "Décrivez en une phrase ce qui ne marche pas. SoleOS prépare un message pour l'assistance, avec un relevé technique (sans mot de passe ni contenu personnel). Vous le relisez avant de l'envoyer.",
        )));
        explication.set_wrap(true);
        explication.set_xalign(0.0);
        explication.set_max_width_chars(52);
        explication.add_css_class("texte-info");
        racine.append(&explication);
        let texte = gtk::TextView::new();
        texte.set_wrap_mode(gtk::WrapMode::WordChar);
        texte.add_css_class("champ-signaler");
        texte.set_top_margin(8);
        texte.set_bottom_margin(8);
        texte.set_left_margin(10);
        texte.set_right_margin(10);
        let cadre = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(110)
            .child(&texte)
            .build();
        cadre.add_css_class("cadre-signaler");
        racine.append(&cadre);
        let bouton = gtk::Button::with_label(&tr("Préparer le message"));
        bouton.add_css_class("petit");
        bouton.add_css_class("principal");
        bouton.set_halign(gtk::Align::Start);
        racine.append(&bouton);
        let info = gtk::Label::new(None);
        info.set_wrap(true);
        info.set_xalign(0.0);
        info.set_max_width_chars(52);
        info.add_css_class("texte-info");
        info.set_selectable(true);
        racine.append(&info);
        let s = Rc::new(Signaler { racine, texte, info, bouton: bouton.clone() });
        let moi = Rc::downgrade(&s);
        bouton.connect_clicked(move |_| {
            if let Some(s) = moi.upgrade() {
                s.preparer();
            }
        });
        s
    }

    /// Page ouverte : prête pour un nouveau signalement
    pub fn au_debut(&self) {
        if self.bouton.is_sensitive() {
            self.info.set_label("");
        }
        self.texte.grab_focus();
    }

    fn preparer(self: &Rc<Self>) {
        let b = self.texte.buffer();
        let description = b.text(&b.start_iter(), &b.end_iter(), false).trim().to_string();
        if description.is_empty() {
            self.info.set_label(&tr("Décrivez d'abord le problème en une phrase."));
            return;
        }
        self.bouton.set_sensitive(false);
        self.info.set_label(&tr("Préparation du relevé et du message…"));
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                // Relevé d'assistance (première ligne : « Diagnostic enregistré : <fichier> »)
                let sortie = Command::new("soleos-diagnostic").output().map(|s| String::from_utf8_lossy(&s.stdout).to_string()).unwrap_or_default();
                let fichier = sortie.lines().next().and_then(|l| l.split_once(": ")).map(|(_, f)| f.trim().to_string()).filter(|f| std::path::Path::new(f).exists());
                let version = std::fs::read_to_string("/etc/soleos-version").unwrap_or_default().trim().to_string();
                let machine = ["sys_vendor", "product_name"]
                    .iter()
                    .filter_map(|f| std::fs::read_to_string(format!("/sys/class/dmi/id/{f}")).ok())
                    .map(|t| t.trim().to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let sujet = format!("SoleOS {version} : {}", description.chars().take(60).collect::<String>());
                let corps = format!("{description}\n\n--\nSoleOS {version}\n{machine}");
                let mut options = format!("to='{ASSISTANCE}',subject='{}',body='{}'", valeur(&sujet), valeur(&corps));
                if let Some(f) = &fichier {
                    options.push_str(&format!(",attachment='file://{}'", valeur(f)));
                }
                let mail = Command::new("thunderbird").args(["-compose", &options]).spawn().is_ok();
                (mail, fichier)
            },
            move |(mail, fichier)| {
                let Some(s) = moi.upgrade() else { return };
                s.bouton.set_sensitive(true);
                let releve = fichier.clone().unwrap_or_default();
                s.info.set_label(&if mail {
                    s.texte.buffer().set_text("");
                    trv("Le message est prêt dans Mail : relisez-le puis touchez « Envoyer ». Relevé joint : {f}", &[("f", &releve)])
                } else {
                    trv("Mail n'a pas pu s'ouvrir. Écrivez à {a} en joignant le fichier {f}.", &[("a", ASSISTANCE), ("f", &releve)])
                });
            },
        );
    }
}
