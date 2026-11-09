//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Réglages → Son → « Le son ne marche pas ? » : essai du son.
//!
//! Certaines puces son sont câblées autrement que la norme (haut-parleurs alimentés par une
//! sortie GPIO, sorties non déclarées, prise casque mal détectée). SoleOS essaie les réglages
//! connus (« recettes » de /usr/local/sbin/flashos-son) un par un ; à chaque essai un son est
//! joué et l'utilisateur dit s'il l'entend. La recette qui marche est gardée pour ce modèle
//! d'ordinateur seulement, et appliquée à chaque démarrage.
//!
//! Pendant l'essai, PipeWire est arrêté : le son est joué directement sur la carte (ALSA), et
//! la puce peut être reconfigurée.

use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use gtk::prelude::*;

use crate::tr::{tr, trv};
use crate::wifi::en_fond;

const OUTIL: &str = "/usr/local/sbin/flashos-son";

/// Recettes essayées pour les haut-parleurs, dans l'ordre (vide = réglage d'origine)
const ESSAIS_HP: &[&[u8]] = &[&[], &[1], &[2], &[3]];

fn sudo(args: &[&str]) -> (bool, String) {
    match Command::new("sudo").arg("-n").arg(OUTIL).args(args).output() {
        Ok(s) => (s.status.success(), String::from_utf8_lossy(&s.stdout).to_string() + &String::from_utf8_lossy(&s.stderr)),
        Err(e) => (false, e.to_string()),
    }
}

fn utilisateur(args: &[&str]) -> bool {
    Command::new(args[0]).args(&args[1..]).status().map(|s| s.success()).unwrap_or(false)
}

/// Arrête / relance le son de la session (la puce ne se reconfigure que si personne ne l'utilise)
fn arreter_pipewire() {
    utilisateur(&["timeout", "15", "systemctl", "--user", "stop", "pipewire-pulse.socket", "pipewire.socket", "pipewire-pulse.service", "wireplumber.service", "pipewire.service"]);
    std::thread::sleep(std::time::Duration::from_millis(600));
}

fn relancer_pipewire() {
    // Sans attendre (une attente bloquait la fin de l'essai sur certains PC)
    utilisateur(&["systemctl", "--user", "--no-block", "start", "pipewire.socket", "pipewire-pulse.socket", "pipewire.service", "wireplumber.service", "pipewire-pulse.service"]);
}

fn liste(r: &[u8]) -> Vec<String> {
    r.iter().map(|n| n.to_string()).collect()
}

/// Applique des recettes (vide : réglage d'origine). Réessaie un peu si la carte est encore occupée.
fn appliquer(r: &[u8]) -> bool {
    for _ in 0..5 {
        let ok = if r.is_empty() {
            sudo(&["annuler"]).0
        } else {
            let l = liste(r);
            let mut a = vec!["essayer"];
            a.extend(l.iter().map(|s| s.as_str()));
            sudo(&a).0
        };
        if ok {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
    false
}

/// Joue un son d'essai (2 notes, gauche puis droite) sur la carte, haut-parleurs ou casque
fn jouer(carte: &str, casque: bool) {
    let c = carte.to_string();
    let reg = |nom: &str, val: &str| {
        let _ = Command::new("amixer").args(["-q", "-c", &c, "sset", nom, val]).status();
    };
    reg("Master", "80% unmute");
    reg("PCM", "80% unmute");
    if casque {
        reg("Headphone", "80% unmute");
        reg("Speaker", "mute");
    } else {
        reg("Speaker", "80% unmute");
    }
    let _ = Command::new("timeout")
        .args(["5", "speaker-test", "-D", &format!("plughw:{c},0"), "-c", "2", "-t", "sine", "-f", "660", "-l", "1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Prise casque détectée ? (None : la puce ne sait pas le dire)
fn casque_detecte(carte: &str) -> Option<bool> {
    let s = Command::new("amixer").args(["-c", carte, "cget", "iface=CARD,name=Headphone Jack"]).output().ok()?;
    let t = String::from_utf8_lossy(&s.stdout).to_string();
    let v = t.lines().find(|l| l.trim_start().starts_with(": values="))?;
    Some(v.contains("on"))
}

#[derive(Clone, Copy, PartialEq)]
enum Etape {
    Debut,
    HautParleurs(usize),
    BrancherCasque,
    Casque(u8), // 0 : réglage des haut-parleurs seul ; 5 ou 4 : avec cette recette
    Fin,
}

pub struct EssaiSon {
    pub racine: gtk::Box,
    message: gtk::Label,
    detail: gtk::Label,
    boutons: gtk::Box,
    carte: RefCell<String>,
    etape: Cell<Etape>,
    /// Recettes retenues pour les haut-parleurs (None : aucune n'a marché)
    hp: RefCell<Option<Vec<u8>>>,
    /// Recette retenue pour le casque (0 : aucune nécessaire ; None : rien n'a marché)
    casque: Cell<Option<u8>>,
    en_cours: Cell<bool>,
}

impl EssaiSon {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let titre = gtk::Label::new(Some(&tr("Essai du son")));
        titre.add_css_class("nom-appareil");
        titre.set_xalign(0.0);
        racine.append(&titre);
        let message = gtk::Label::new(None);
        message.set_wrap(true);
        message.set_xalign(0.0);
        message.set_max_width_chars(52);
        message.add_css_class("texte-essai-son");
        racine.append(&message);
        let detail = gtk::Label::new(None);
        detail.set_wrap(true);
        detail.set_xalign(0.0);
        detail.set_max_width_chars(52);
        detail.add_css_class("texte-info");
        racine.append(&detail);
        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        boutons.set_margin_top(6);
        racine.append(&boutons);
        let e = Rc::new(EssaiSon {
            racine,
            message,
            detail,
            boutons,
            carte: RefCell::new(String::new()),
            etape: Cell::new(Etape::Debut),
            hp: RefCell::new(None),
            casque: Cell::new(None),
            en_cours: Cell::new(false),
        });
        e.au_debut();
        // Page quittée en plein essai (retour, Réglages fermés) : le son est remis en route
        let moi = Rc::downgrade(&e);
        e.racine.connect_unmap(move |_| {
            if let Some(m) = moi.upgrade() {
                m.abandonner();
            }
        });
        e
    }

    /// Essai interrompu : réglage gardé (ou d'origine) remis, son relancé
    pub fn abandonner(self: &Rc<Self>) {
        if matches!(self.etape.get(), Etape::Debut | Etape::Fin) {
            return;
        }
        self.etape.set(Etape::Fin);
        self.en_cours.set(false);
        en_fond(
            || {
                appliquer(&[]);
                sudo(&["appliquer"]);
                relancer_pipewire();
            },
            |_| {},
        );
        self.au_debut();
    }

    fn bouton(self: &Rc<Self>, texte: &str, principal: bool, action: impl Fn(&Rc<Self>) + 'static) {
        let b = gtk::Button::with_label(texte);
        b.add_css_class("petit");
        if principal {
            b.add_css_class("principal");
        }
        let moi = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(m) = moi.upgrade() {
                if !m.en_cours.get() {
                    action(&m);
                }
            }
        });
        self.boutons.append(&b);
    }

    fn vider_boutons(&self) {
        while let Some(e) = self.boutons.first_child() {
            self.boutons.remove(&e);
        }
    }

    fn occupe(&self, texte: &str) {
        self.en_cours.set(true);
        self.vider_boutons();
        let roue = gtk::Spinner::new();
        roue.start();
        self.boutons.append(&roue);
        self.detail.set_label(texte);
    }

    /// Page ouverte : on repart du début (si aucun essai n'est en cours)
    pub fn au_debut(self: &Rc<Self>) {
        if self.en_cours.get() || !matches!(self.etape.get(), Etape::Debut | Etape::Fin) {
            return;
        }
        self.etape.set(Etape::Debut);
        self.message.set_label(&tr(
            "Si les haut-parleurs ou le casque restent muets, SoleOS peut essayer les réglages connus pour la puce son de cet ordinateur. À chaque essai, un son est joué : dites simplement si vous l'entendez. Rien n'est gardé sans votre accord.",
        ));
        self.detail.set_label(&tr("Durée : 2 à 5 minutes. Le son des applications est coupé pendant l'essai."));
        self.vider_boutons();
        self.bouton(&tr("Commencer l'essai"), true, |m| m.commencer());
        // Réglages déjà gardés : on peut revenir à l'origine
        let moi = Rc::downgrade(self);
        en_fond(
            || sudo(&["etat"]).1,
            move |etat| {
                let Some(m) = moi.upgrade() else { return };
                let gardees = etat.lines().find_map(|l| l.strip_prefix("gardees=")).unwrap_or("").trim().to_string();
                if !gardees.is_empty() && m.etape.get() == Etape::Debut {
                    m.detail.set_label(&trv("Un réglage du son est déjà appliqué sur cet ordinateur (recettes {r}).", &[("r", &gardees)]));
                    m.bouton(&tr("Revenir au réglage d'origine"), false, |m| m.revenir_origine());
                }
            },
        );
    }

    fn commencer(self: &Rc<Self>) {
        self.occupe(&tr("Préparation…"));
        let moi = Rc::downgrade(self);
        en_fond(
            || {
                let (ok, etat) = sudo(&["etat"]);
                let reglable = etat.lines().any(|l| l == "reglable=oui");
                let carte = etat.lines().find_map(|l| l.strip_prefix("carte=")).map(str::to_string).filter(|_| reglable);
                if ok && carte.is_some() {
                    arreter_pipewire();
                }
                (carte, etat.lines().find_map(|l| l.strip_prefix("puce=")).unwrap_or("").to_string())
            },
            move |(carte, puce)| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                match carte {
                    Some(c) => {
                        *m.carte.borrow_mut() = c;
                        m.message.set_label(&trv("Puce son : {p}.", &[("p", &puce)]));
                        *m.hp.borrow_mut() = None;
                        m.casque.set(None);
                        m.essai_hp(0);
                    }
                    None => {
                        m.etape.set(Etape::Fin);
                        m.message.set_label(&tr("Aucune puce son à régler n'a été trouvée sur cet ordinateur."));
                        m.detail.set_label("");
                        m.vider_boutons();
                        m.bouton(&tr("Créer un relevé pour l'assistance"), false, |m| m.releve());
                    }
                }
            },
        );
    }

    /// Haut-parleurs : essai n° i (réglage d'origine, puis les recettes)
    fn essai_hp(self: &Rc<Self>, i: usize) {
        self.etape.set(Etape::HautParleurs(i));
        self.message.set_label(&trv(
            "Haut-parleurs — essai {n} sur {t}. Débranchez le casque s'il y en a un. Un son va être joué (gauche puis droite).",
            &[("n", &(i + 1).to_string()), ("t", &ESSAIS_HP.len().to_string())],
        ));
        self.occupe(&tr("Lecture du son…"));
        let (carte, recettes) = (self.carte.borrow().clone(), ESSAIS_HP[i]);
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                let ok = appliquer(recettes);
                if ok {
                    jouer(&carte, false);
                }
                ok
            },
            move |ok| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                m.vider_boutons();
                m.detail.set_label(&if ok { tr("Avez-vous entendu le son dans les haut-parleurs ?") } else { tr("Ce réglage n'a pas pu être appliqué.") });
                m.bouton(&tr("Oui, je l'entends"), true, move |m| {
                    *m.hp.borrow_mut() = Some(ESSAIS_HP[i].to_vec());
                    m.brancher_casque();
                });
                m.bouton(&tr("Non"), false, move |m| {
                    if i + 1 < ESSAIS_HP.len() {
                        m.essai_hp(i + 1);
                    } else {
                        *m.hp.borrow_mut() = None;
                        m.brancher_casque();
                    }
                });
                m.bouton(&tr("Rejouer"), false, move |m| m.essai_hp(i));
            },
        );
    }

    fn base_hp(&self) -> Vec<u8> {
        self.hp.borrow().clone().unwrap_or_default()
    }

    fn brancher_casque(self: &Rc<Self>) {
        self.etape.set(Etape::BrancherCasque);
        self.message.set_label(&tr("Casque — branchez un casque ou des écouteurs dans la prise de l'ordinateur."));
        self.detail.set_label(&tr("Pas de casque sous la main ? Vous pouvez passer cette étape."));
        self.vider_boutons();
        self.bouton(&tr("C'est branché"), true, |m| m.essai_casque(0));
        self.bouton(&tr("Passer"), false, |m| m.terminer());
    }

    /// Casque : 0 = avec le réglage des haut-parleurs seul ; 5 = détection inversée ; 4 = sans détection
    fn essai_casque(self: &Rc<Self>, recette: u8) {
        self.etape.set(Etape::Casque(recette));
        self.message.set_label(&tr("Casque — un son va être joué dans le casque."));
        self.occupe(&tr("Lecture du son…"));
        let mut r = self.base_hp();
        if recette != 0 {
            r.push(recette);
        }
        let carte = self.carte.borrow().clone();
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                let ok = appliquer(&r);
                std::thread::sleep(std::time::Duration::from_millis(500));
                let detecte = casque_detecte(&carte);
                if ok {
                    jouer(&carte, true);
                }
                (ok, detecte)
            },
            move |(ok, detecte)| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                m.vider_boutons();
                let info = match detecte {
                    Some(true) => tr("SoleOS voit bien le casque branché."),
                    Some(false) => tr("SoleOS ne voit pas le casque branché."),
                    None => String::new(),
                };
                m.detail.set_label(&if ok { format!("{info} {}", tr("Avez-vous entendu le son dans le casque ?")).trim().to_string() } else { tr("Ce réglage n'a pas pu être appliqué.") });
                // Entendu ET détecté : c'est bon. Entendu sans détection : on essaie la détection inversée
                m.bouton(&tr("Oui, je l'entends"), true, move |m| {
                    if detecte == Some(false) && recette == 0 {
                        m.essai_casque(5);
                    } else {
                        m.casque.set(Some(recette));
                        m.terminer();
                    }
                });
                m.bouton(&tr("Non"), false, move |m| match recette {
                    0 => m.essai_casque(5),
                    5 => m.essai_casque(4),
                    _ => {
                        m.casque.set(None);
                        m.terminer();
                    }
                });
                m.bouton(&tr("Rejouer"), false, move |m| m.essai_casque(recette));
            },
        );
    }

    fn terminer(self: &Rc<Self>) {
        self.etape.set(Etape::Fin);
        let hp = self.hp.borrow().clone();
        let casque = self.casque.get();
        let mut r = hp.clone().unwrap_or_default();
        if let Some(c) = casque.filter(|c| *c != 0) {
            r.push(c);
        }
        let hp_ok = hp.is_some();
        let casque_ok = casque.is_some();
        let texte = match (hp_ok, casque_ok) {
            (true, true) => tr("Le son fonctionne : haut-parleurs et casque."),
            (true, false) => tr("Les haut-parleurs fonctionnent. Le casque n'a pas été réglé (passé, ou aucun réglage n'a marché)."),
            (false, true) => tr("Le casque fonctionne, mais aucun réglage connu ne fait marcher les haut-parleurs de cet ordinateur."),
            (false, false) => tr("Aucun réglage connu ne fait marcher le son de cet ordinateur."),
        };
        self.message.set_label(&texte);
        self.occupe(&tr("Remise en route du son…"));
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                if r.is_empty() {
                    sudo(&["oublier"]);
                    appliquer(&[]);
                } else {
                    let l = liste(&r);
                    let mut a = vec!["garder"];
                    a.extend(l.iter().map(|s| s.as_str()));
                    sudo(&a);
                    appliquer(&r);
                }
                relancer_pipewire();
                r
            },
            move |r| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                m.vider_boutons();
                m.detail.set_label(&if r.is_empty() {
                    tr("Rien n'a été changé. Un relevé pour l'assistance aidera à trouver le bon réglage pour ce modèle.")
                } else {
                    tr("Ce réglage est gardé pour cet ordinateur et sera appliqué à chaque démarrage.")
                });
                if !hp_ok || !casque_ok {
                    m.bouton(&tr("Créer un relevé pour l'assistance"), false, |m| m.releve());
                }
                m.bouton(&tr("Recommencer"), false, |m| m.au_debut());
            },
        );
    }

    fn revenir_origine(self: &Rc<Self>) {
        self.occupe(&tr("Retour au réglage d'origine…"));
        let moi = Rc::downgrade(self);
        en_fond(
            || {
                sudo(&["oublier"]);
                arreter_pipewire();
                appliquer(&[]);
                relancer_pipewire();
            },
            move |_| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                m.etape.set(Etape::Fin);
                m.au_debut();
                m.detail.set_label(&tr("Le son est revenu au réglage d'origine."));
            },
        );
    }

    fn releve(self: &Rc<Self>) {
        self.occupe(&tr("Création du relevé…"));
        let moi = Rc::downgrade(self);
        en_fond(
            || {
                Command::new("flashos-diagnostic").output().map(|s| String::from_utf8_lossy(&s.stdout).lines().next().unwrap_or("").to_string()).unwrap_or_default()
            },
            move |sortie| {
                let Some(m) = moi.upgrade() else { return };
                m.en_cours.set(false);
                m.vider_boutons();
                m.detail.set_label(&if sortie.is_empty() { tr("Le relevé n'a pas pu être créé.") } else { sortie });
                m.bouton(&tr("Recommencer"), false, |m| m.au_debut());
            },
        );
    }
}
