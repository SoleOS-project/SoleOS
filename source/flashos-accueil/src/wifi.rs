//! Panneau Wi-Fi : liste des réseaux, connexion avec mot de passe,
//! activation / désactivation. S'appuie sur NetworkManager (outil nmcli).
//! Les commandes tournent dans un fil à part pour ne jamais figer l'écran.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::icones;
use crate::tr::{tr, trv};

#[derive(Clone, Debug)]
pub struct ReseauWifi {
    pub ssid: String,
    pub signal: u8,
    pub securise: bool,
    pub actif: bool,
    pub connu: bool,
}

#[derive(Clone, Debug)]
pub struct EtatWifi {
    pub outil: bool,
    pub carte: bool,
    pub active: bool,
    pub reseaux: Vec<ReseauWifi>,
}

fn nmcli(args: &[&str]) -> Result<String, String> {
    // Sortie de nmcli en anglais quelle que soit la langue choisie (elle est analysée ici)
    let sortie = Command::new("nmcli").args(args).env("LC_ALL", "C.UTF-8").output().map_err(|e| e.to_string())?;
    if sortie.status.success() {
        Ok(String::from_utf8_lossy(&sortie.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&sortie.stderr).trim().to_string())
    }
}

/// Découpe une ligne « nmcli -t » (les « : » du contenu sont échappés en « \: »).
fn champs(ligne: &str) -> Vec<String> {
    let mut resultat = vec![String::new()];
    let mut echappe = false;
    for c in ligne.chars() {
        if echappe {
            resultat.last_mut().unwrap().push(c);
            echappe = false;
        } else if c == '\\' {
            echappe = true;
        } else if c == ':' {
            resultat.push(String::new());
        } else {
            resultat.last_mut().unwrap().push(c);
        }
    }
    resultat
}

/// Lit l'état complet du Wi-Fi (bloquant : à appeler hors de l'interface).
pub fn lire_etat() -> EtatWifi {
    let Ok(appareils) = nmcli(&["-t", "-f", "TYPE", "device"]) else {
        return EtatWifi { outil: false, carte: false, active: false, reseaux: vec![] };
    };
    let carte = appareils.lines().any(|l| l.trim() == "wifi");
    let active = nmcli(&["radio", "wifi"]).map(|s| s.trim() == "enabled").unwrap_or(false);
    if !carte || !active {
        return EtatWifi { outil: true, carte, active, reseaux: vec![] };
    }

    let connus: Vec<String> = nmcli(&["-t", "-f", "NAME,TYPE", "connection", "show"])
        .unwrap_or_default()
        .lines()
        .map(champs)
        .filter(|c| c.len() >= 2 && c[1].contains("wireless"))
        .map(|c| c[0].clone())
        .collect();

    let mut reseaux: Vec<ReseauWifi> = Vec::new();
    let liste = nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "auto"])
        .unwrap_or_default();
    for ligne in liste.lines() {
        let c = champs(ligne);
        if c.len() < 4 || c[1].is_empty() {
            continue; // réseau masqué
        }
        let r = ReseauWifi {
            ssid: c[1].clone(),
            signal: c[2].parse().unwrap_or(0),
            securise: !c[3].trim().is_empty() && c[3].trim() != "--",
            actif: c[0].trim() == "*",
            connu: connus.contains(&c[1]),
        };
        // Un même réseau peut apparaître plusieurs fois (plusieurs bornes) : on garde le meilleur.
        match reseaux.iter_mut().find(|x| x.ssid == r.ssid) {
            Some(x) => {
                x.actif |= r.actif;
                x.signal = x.signal.max(r.signal);
            }
            None => reseaux.push(r),
        }
    }
    reseaux.sort_by(|a, b| b.actif.cmp(&a.actif).then(b.signal.cmp(&a.signal)));
    EtatWifi { outil: true, carte, active, reseaux }
}

/// Échec d'une connexion Wi-Fi.
pub enum Echec {
    /// Mot de passe refusé : le réseau mémorisé a été oublié, il faut le redemander.
    MotDePasse,
    Autre(String),
}

/// Supprime les réglages mémorisés d'un réseau (mot de passe compris).
pub fn oublier(ssid: &str) {
    // Un même nom peut avoir plusieurs profils : on les supprime tous.
    for _ in 0..5 {
        if nmcli(&["connection", "delete", "id", ssid]).is_err() {
            break;
        }
    }
}

/// Se connecte à un réseau (bloquant).
pub fn connecter(r: &ReseauWifi, mot_de_passe: Option<&str>) -> Result<(), Echec> {
    let resultat = if r.connu && mot_de_passe.is_none() {
        nmcli(&["--wait", "30", "connection", "up", "id", &r.ssid])
    } else {
        // Nouveau mot de passe : on repart de zéro pour ce réseau.
        if mot_de_passe.is_some() {
            oublier(&r.ssid);
        }
        let mut args = vec!["--wait", "30", "device", "wifi", "connect", r.ssid.as_str()];
        if let Some(mdp) = mot_de_passe {
            args.push("password");
            args.push(mdp);
        }
        nmcli(&args)
    };
    resultat.map(|_| ()).map_err(|e| {
        let e_min = e.to_lowercase();
        if e_min.contains("secret") || e_min.contains("password") || e_min.contains("802-1x")
            || e_min.contains("psk") || e_min.contains("mot de passe")
        {
            // Sinon le mauvais mot de passe resterait mémorisé et bloquerait tout nouvel essai.
            oublier(&r.ssid);
            Echec::MotDePasse
        } else if e_min.contains("timeout") || e_min.contains("délai") {
            Echec::Autre(tr("Le réseau ne répond pas. Réessayez plus près de la box."))
        } else {
            Echec::Autre(trv("Connexion impossible : {e}", &[("e", &e)]))
        }
    })
}

/// Réseau masqué (son nom n'est pas diffusé) : connexion en donnant son nom (bloquant)
/// `securite` : 0 automatique, 1 WPA/WPA2 personnel, 2 WPA3 personnel, 3 aucun (réseau ouvert)
pub fn connecter_masque(ssid: &str, mot_de_passe: &str, securite: u32) -> Result<(), Echec> {
    oublier(ssid);
    let resultat = if securite == 0 {
        // Automatique : le Wi-Fi reconnaît lui-même le chiffrement du réseau
        let mut args = vec!["--wait", "40", "device", "wifi", "connect", ssid];
        if !mot_de_passe.is_empty() {
            args.push("password");
            args.push(mot_de_passe);
        }
        args.push("hidden");
        args.push("yes");
        nmcli(&args).map(|_| ())
    } else {
        // Chiffrement choisi : profil créé à la main, puis connexion
        let mut args = vec!["connection", "add", "type", "wifi", "con-name", ssid, "ifname", "*", "ssid", ssid, "wifi.hidden", "yes"];
        match securite {
            1 => args.extend(["wifi-sec.key-mgmt", "wpa-psk", "wifi-sec.psk", mot_de_passe]),
            2 => args.extend(["wifi-sec.key-mgmt", "sae", "wifi-sec.psk", mot_de_passe]),
            _ => {}
        }
        nmcli(&args).and_then(|_| nmcli(&["--wait", "40", "connection", "up", "id", ssid])).map(|_| ())
    };
    resultat.map_err(|e| {
        let e_min = e.to_lowercase();
        oublier(ssid);
        if e_min.contains("secret") || e_min.contains("password") || e_min.contains("psk") {
            Echec::MotDePasse
        } else if e_min.contains("no network with ssid") || e_min.contains("not found") {
            Echec::Autre(tr("Aucun réseau de ce nom n'a répondu. Vérifiez le nom (majuscules comprises)."))
        } else {
            Echec::Autre(trv("Connexion impossible : {e}", &[("e", &e)]))
        }
    })
}

pub fn activer(oui: bool) {
    let _ = nmcli(&["radio", "wifi", if oui { "on" } else { "off" }]);
}

/// Exécute `travail` dans un fil à part, puis `ensuite` dans l'interface.
pub(crate) fn en_fond<T: Send + 'static>(travail: impl FnOnce() -> T + Send + 'static, ensuite: impl FnOnce(T) + 'static) {
    let (envoi, reception) = async_channel::bounded::<T>(1);
    std::thread::spawn(move || {
        let _ = envoi.send_blocking(travail());
    });
    glib::spawn_future_local(async move {
        if let Ok(resultat) = reception.recv().await {
            ensuite(resultat);
        }
    });
}

/// Le panneau affiché dans la bulle du bouton réseau.
pub struct Panneau {
    pub racine: gtk::Box,
    interrupteur: gtk::Switch,
    info: gtk::Label,
    liste: gtk::Box,
    zone_mdp: gtk::Box,
    titre_mdp: gtk::Label,
    champ_mdp: gtk::PasswordEntry,
    choisi: RefCell<Option<ReseauWifi>>,
    maj_interrupteur: std::cell::Cell<bool>,
    /// Appelé après une connexion (mise à jour de l'icône de la barre).
    pub apres_changement: RefCell<Option<Box<dyn Fn()>>>,
}

impl Panneau {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 8);
        racine.set_size_request(400, -1);

        let titre = gtk::Label::new(Some(&tr("Wi-Fi")));
        titre.add_css_class("titre-panneau");
        titre.set_xalign(0.0);
        titre.set_hexpand(true);
        let interrupteur = gtk::Switch::new();
        interrupteur.set_valign(gtk::Align::Center);
        let entete = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        entete.set_margin_start(8);
        entete.set_margin_end(4);
        entete.append(&titre);
        entete.append(&interrupteur);

        let info = gtk::Label::new(None);
        info.add_css_class("texte-info");
        info.set_xalign(0.0);
        info.set_wrap(true);
        info.set_margin_start(8);

        let liste = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let defilement = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(330)
            .child(&liste)
            .build();

        let titre_mdp = gtk::Label::new(None);
        titre_mdp.set_xalign(0.0);
        titre_mdp.set_wrap(true);
        let champ_mdp = gtk::PasswordEntry::new();
        champ_mdp.set_show_peek_icon(true);
        champ_mdp.add_css_class("champ");
        let annuler = gtk::Button::with_label(&tr("Annuler"));
        annuler.add_css_class("menu");
        let valider = gtk::Button::with_label(&tr("Se connecter"));
        valider.add_css_class("principal");
        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        boutons.set_halign(gtk::Align::End);
        boutons.append(&annuler);
        boutons.append(&valider);
        let zone_mdp = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone_mdp.add_css_class("zone-mdp");
        zone_mdp.append(&titre_mdp);
        zone_mdp.append(&champ_mdp);
        zone_mdp.append(&boutons);
        zone_mdp.set_visible(false);

        // Réseau masqué : nom (SSID) et mot de passe tapés à la main
        let masque = gtk::Button::with_label(&tr("Réseau masqué…"));
        masque.add_css_class("menu");
        masque.set_halign(gtk::Align::Start);
        let champ_ssid = gtk::Entry::builder().placeholder_text(tr("Nom du réseau (SSID)")).build();
        champ_ssid.add_css_class("champ");
        let champ_mdp_masque = gtk::PasswordEntry::new();
        champ_mdp_masque.set_show_peek_icon(true);
        champ_mdp_masque.add_css_class("champ");
        champ_mdp_masque.set_property("placeholder-text", tr("Mot de passe (vide si réseau ouvert)"));
        // Chiffrement du réseau masqué (Automatique convient presque toujours)
        let choix_securite = gtk::DropDown::from_strings(&[
            &tr("Chiffrement : automatique (conseillé)"),
            &tr("WPA / WPA2 personnel"),
            &tr("WPA3 personnel"),
            &tr("Aucun (réseau ouvert)"),
        ]);
        choix_securite.set_hexpand(true);
        {
            let m = champ_mdp_masque.clone();
            choix_securite.connect_selected_notify(move |d| {
                let ouvert = d.selected() == 3;
                m.set_sensitive(!ouvert);
                if ouvert {
                    m.set_text("");
                }
            });
        }
        let annuler_m = gtk::Button::with_label(&tr("Annuler"));
        annuler_m.add_css_class("menu");
        let valider_m = gtk::Button::with_label(&tr("Se connecter"));
        valider_m.add_css_class("principal");
        let boutons_m = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        boutons_m.set_halign(gtk::Align::End);
        boutons_m.append(&annuler_m);
        boutons_m.append(&valider_m);
        let zone_masque = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone_masque.add_css_class("zone-mdp");
        let t_m = gtk::Label::new(Some(&tr("Réseau masqué : tapez son nom exact et son mot de passe.")));
        t_m.set_xalign(0.0);
        t_m.set_wrap(true);
        zone_masque.append(&t_m);
        zone_masque.append(&champ_ssid);
        zone_masque.append(&choix_securite);
        zone_masque.append(&champ_mdp_masque);
        zone_masque.append(&boutons_m);
        zone_masque.set_visible(false);

        racine.append(&entete);
        racine.append(&info);
        racine.append(&defilement);
        racine.append(&zone_mdp);
        racine.append(&masque);
        racine.append(&zone_masque);
        {
            let (z, s) = (zone_masque.clone(), champ_ssid.clone());
            masque.connect_clicked(move |_| {
                z.set_visible(!z.is_visible());
                s.grab_focus();
            });
            let (z, s, m) = (zone_masque.clone(), champ_ssid.clone(), champ_mdp_masque.clone());
            annuler_m.connect_clicked(move |_| {
                z.set_visible(false);
                s.set_text("");
                m.set_text("");
            });
        }

        let panneau = Rc::new(Panneau {
            racine,
            interrupteur: interrupteur.clone(),
            info,
            liste,
            zone_mdp: zone_mdp.clone(),
            titre_mdp,
            champ_mdp: champ_mdp.clone(),
            choisi: RefCell::new(None),
            maj_interrupteur: std::cell::Cell::new(false),
            apres_changement: RefCell::new(None),
        });

        let moi = Rc::downgrade(&panneau);
        interrupteur.connect_active_notify(move |s| {
            let Some(p) = moi.upgrade() else { return };
            if p.maj_interrupteur.get() {
                return;
            }
            let oui = s.is_active();
            p.info.set_label(&if oui { tr("Activation du Wi-Fi…") } else { tr("Désactivation du Wi-Fi…") });
            let moi = Rc::downgrade(&p);
            en_fond(
                move || {
                    activer(oui);
                    std::thread::sleep(std::time::Duration::from_secs(if oui { 3 } else { 1 }));
                },
                move |_| {
                    if let Some(p) = moi.upgrade() {
                        p.actualiser();
                    }
                },
            );
        });

        let moi = Rc::downgrade(&panneau);
        annuler.connect_clicked(move |_| {
            if let Some(p) = moi.upgrade() {
                p.zone_mdp.set_visible(false);
                p.champ_mdp.set_text("");
            }
        });

        let moi = Rc::downgrade(&panneau);
        let connecter_avec_mdp = move || {
            let Some(p) = moi.upgrade() else { return };
            let mdp = p.champ_mdp.text().to_string();
            let reseau = p.choisi.borrow().clone();
            if let (Some(r), false) = (reseau, mdp.is_empty()) {
                p.zone_mdp.set_visible(false);
                p.champ_mdp.set_text("");
                p.lancer_connexion(r, Some(mdp));
            }
        };
        let c = connecter_avec_mdp.clone();
        valider.connect_clicked(move |_| c());
        champ_mdp.connect_activate(move |_| connecter_avec_mdp());

        let moi = Rc::downgrade(&panneau);
        let (z, s, m, cs) = (zone_masque.clone(), champ_ssid.clone(), champ_mdp_masque.clone(), choix_securite.clone());
        let connecter_masque_ui = move || {
            let Some(p) = moi.upgrade() else { return };
            let ssid = s.text().trim().to_string();
            if ssid.is_empty() {
                return;
            }
            let mdp = m.text().to_string();
            let securite = cs.selected();
            if (securite == 1 || securite == 2) && mdp.chars().count() < 8 {
                p.info.set_label(&tr("Ce chiffrement demande un mot de passe d'au moins 8 caractères."));
                return;
            }
            z.set_visible(false);
            s.set_text("");
            m.set_text("");
            p.info.set_label(&trv("Connexion à « {ssid} »…", &[("ssid", &ssid)]));
            let moi = Rc::downgrade(&p);
            en_fond(
                move || connecter_masque(&ssid, &mdp, securite),
                move |r| {
                    let Some(p) = moi.upgrade() else { return };
                    match r {
                        Ok(()) => {
                            p.actualiser();
                            if let Some(f) = p.apres_changement.borrow().as_ref() {
                                f();
                            }
                        }
                        Err(Echec::MotDePasse) => p.info.set_label(&tr("Mot de passe refusé. Réessayez.")),
                        Err(Echec::Autre(t)) => p.info.set_label(&t),
                    }
                },
            );
        };
        let c = connecter_masque_ui.clone();
        valider_m.connect_clicked(move |_| c());
        champ_mdp_masque.connect_activate(move |_| connecter_masque_ui());

        panneau
    }

    /// Relit l'état du Wi-Fi et reconstruit la liste.
    pub fn actualiser(self: &Rc<Self>) {
        self.info.set_label(&tr("Recherche des réseaux…"));
        let moi = Rc::downgrade(self);
        en_fond(lire_etat, move |etat| {
            if let Some(p) = moi.upgrade() {
                p.afficher(etat);
            }
        });
    }

    fn afficher(self: &Rc<Self>, etat: EtatWifi) {
        while let Some(enfant) = self.liste.first_child() {
            self.liste.remove(&enfant);
        }
        self.maj_interrupteur.set(true);
        self.interrupteur.set_active(etat.active);
        self.interrupteur.set_sensitive(etat.carte);
        self.maj_interrupteur.set(false);

        let message = if !etat.outil {
            tr("Le gestionnaire de réseau n'est pas installé.")
        } else if !etat.carte {
            tr("Aucune carte Wi-Fi détectée sur cet ordinateur.")
        } else if !etat.active {
            tr("Le Wi-Fi est désactivé.")
        } else if etat.reseaux.is_empty() {
            tr("Aucun réseau trouvé à proximité.")
        } else {
            String::new()
        };
        self.info.set_label(&message);
        self.info.set_visible(!message.is_empty());

        for r in etat.reseaux {
            self.liste.append(&self.ligne(r));
        }
    }

    fn ligne(self: &Rc<Self>, r: ReseauWifi) -> gtk::Button {
        let icone = icones::image("wifi", 18);
        icone.set_opacity(match r.signal {
            67.. => 1.0,
            34..=66 => 0.7,
            _ => 0.4,
        });
        let nom = gtk::Label::new(Some(&r.ssid));
        nom.set_xalign(0.0);
        nom.set_hexpand(true);
        nom.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let contenu = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        contenu.append(&icone);
        contenu.append(&nom);
        if r.actif {
            let etat = gtk::Label::new(Some(&tr("Connecté")));
            etat.add_css_class("texte-info");
            contenu.append(&etat);
        }
        if r.securise {
            contenu.append(&gtk::Label::new(Some(&tr("🔒"))));
        }
        let bouton = gtk::Button::builder().child(&contenu).build();
        bouton.add_css_class("reseau-ligne");
        if r.actif {
            bouton.add_css_class("reseau-actif");
        }
        bouton.set_tooltip_text(Some(&trv("Signal : {n}", &[("n", &crate::tr::pourcent(r.signal))])));

        let moi = Rc::downgrade(self);
        bouton.connect_clicked(move |_| {
            let Some(p) = moi.upgrade() else { return };
            if r.actif {
                return;
            }
            if r.securise && !r.connu {
                p.titre_mdp.set_label(&trv("Mot de passe pour « {nom} »", &[("nom", &r.ssid)]));
                *p.choisi.borrow_mut() = Some(r.clone());
                p.zone_mdp.set_visible(true);
                p.champ_mdp.grab_focus();
            } else {
                p.lancer_connexion(r.clone(), None);
            }
        });
        bouton
    }

    fn lancer_connexion(self: &Rc<Self>, r: ReseauWifi, mdp: Option<String>) {
        self.info.set_visible(true);
        self.info.set_label(&trv("Connexion à « {nom} »…", &[("nom", &r.ssid)]));
        let moi = Rc::downgrade(self);
        let nom = r.ssid.clone();
        let r_copie = r.clone();
        en_fond(
            move || connecter(&r, mdp.as_deref()),
            move |resultat| {
                let Some(p) = moi.upgrade() else { return };
                match resultat {
                    Ok(()) => {
                        p.info.set_label(&trv("Connecté à « {nom} ».", &[("nom", &nom)]));
                        let moi = Rc::downgrade(&p);
                        en_fond(lire_etat, move |etat| {
                            if let Some(p) = moi.upgrade() {
                                p.afficher(etat);
                                p.info.set_visible(true);
                                p.info.set_label(&trv("Connecté à « {nom} ».", &[("nom", &nom)]));
                            }
                        });
                    }
                    Err(Echec::Autre(message)) => p.info.set_label(&message),
                    Err(Echec::MotDePasse) => {
                        // On redemande le mot de passe, sur place.
                        p.info.set_label(&tr("Mot de passe incorrect. Réessayez."));
                        let mut r2 = r_copie.clone();
                        r2.connu = false;
                        p.titre_mdp.set_label(&trv("Mot de passe pour « {nom} »", &[("nom", &r2.ssid)]));
                        *p.choisi.borrow_mut() = Some(r2);
                        p.zone_mdp.set_visible(true);
                        p.champ_mdp.grab_focus();
                    }
                }
                if let Some(f) = p.apres_changement.borrow().as_ref() {
                    f();
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    //! Tests avec un faux NetworkManager (FLASHOS_FAUX_NMCLI = dossier contenant un faux « nmcli »).
    use super::*;
    use std::fs;

    const PROFILS: &str = "/tmp/claude-0/fakenm_profils";
    const ACTIF: &str = "/tmp/claude-0/fakenm_actif";

    fn preparer(profils: &str) {
        let dossier = std::env::var("FLASHOS_FAUX_NMCLI").expect("FLASHOS_FAUX_NMCLI");
        let chemin = format!("{dossier}:{}", std::env::var("PATH").unwrap());
        unsafe { std::env::set_var("PATH", chemin) };
        fs::write(PROFILS, profils).unwrap();
        fs::write(ACTIF, "").unwrap();
    }
    fn reseau(ssid: &str, connu: bool) -> ReseauWifi {
        ReseauWifi { ssid: ssid.into(), signal: 50, securise: true, actif: false, connu }
    }

    #[test]
    fn scenario_complet() {
        // 1. Ancien mauvais mot de passe mémorisé (la panne constatée)
        preparer("Voisin:5G|ancienfaux\n");
        let etat = lire_etat();
        let v = etat.reseaux.iter().find(|r| r.ssid == "Voisin:5G").expect("réseau listé");
        assert!(v.connu, "le réseau doit être vu comme mémorisé");
        assert!(matches!(connecter(v, None), Err(Echec::MotDePasse)));
        assert!(!fs::read_to_string(PROFILS).unwrap().contains("Voisin:5G"), "profil fautif oublié");

        // 2. Nouveau mot de passe faux : rien ne doit rester mémorisé
        assert!(matches!(connecter(&reseau("Freebox-Salon", false), Some("faux")), Err(Echec::MotDePasse)));
        assert!(!fs::read_to_string(PROFILS).unwrap().contains("Freebox-Salon"));

        // 3. Bon mot de passe : connexion réussie et mémorisée
        assert!(connecter(&reseau("Freebox-Salon", false), Some("bonmotdepasse")).is_ok());
        assert_eq!(fs::read_to_string(ACTIF).unwrap().trim(), "Freebox-Salon");
        let n = fs::read_to_string(PROFILS).unwrap().matches("Freebox-Salon|").count();
        assert_eq!(n, 1, "un seul profil pour ce réseau");

        // 4. Réseau mémorisé avec le bon mot de passe : reconnexion sans rien demander
        fs::write(ACTIF, "").unwrap();
        let etat = lire_etat();
        let f = etat.reseaux.iter().find(|r| r.ssid == "Freebox-Salon").unwrap();
        assert!(f.connu);
        assert!(connecter(f, None).is_ok());
    }
}
