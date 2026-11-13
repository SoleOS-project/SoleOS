//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Réglages → Imprimantes et scanner → « Ajouter une imprimante manuellement » (v13).
//!
//! Pour l'imprimante que la recherche automatique ne trouve pas (autre réseau, ancienne
//! imprimante…), comme sous Windows : adresse IP ou nom, protocole (automatique, IPP, port 9100
//! « JetDirect », LPD), puis pilote : sans pilote (IPP Everywhere) si l'imprimante le permet,
//! sinon la marque et le modèle dans les pilotes installés (lpinfo -m), ou le fichier PPD du
//! fabricant. Outils standards de CUPS : lpadmin, lpinfo.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk::prelude::*;

use crate::tr::{tr, trv};
use crate::wifi::en_fond;

fn outil(prog: &str) -> String {
    for d in ["/usr/sbin", "/usr/bin", "/sbin", "/bin"] {
        let c = format!("{d}/{prog}");
        if std::path::Path::new(&c).exists() {
            return c;
        }
    }
    prog.to_string()
}

fn lancer(prog: &str, args: &[&str], delai: u64) -> Result<String, String> {
    let s = Command::new("timeout")
        .arg(delai.to_string())
        .arg(outil(prog))
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if s.status.success() {
        Ok(String::from_utf8_lossy(&s.stdout).to_string())
    } else {
        let e = String::from_utf8_lossy(&s.stderr).trim().to_string();
        Err(if e.is_empty() { tr("l'imprimante ne répond pas") } else { e })
    }
}

/// Adresse tapée : IP (v4/v6) ou nom de machine, sans espace ni caractère spécial
pub fn adresse_valide(a: &str) -> bool {
    !a.is_empty() && a.len() < 128 && a.chars().all(|c| c.is_ascii_alphanumeric() || ".-:_[]".contains(c))
}

/// Nom de file d'impression à partir d'un texte (lettres, chiffres, _ et -)
fn nom_file(texte: &str) -> String {
    let n: String = texte.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let n = n.trim_matches('_').to_string();
    if n.is_empty() { "Imprimante".into() } else { n.chars().take(40).collect() }
}

fn nom_libre(base: &str) -> String {
    let existants = lancer("lpstat", &["-v"], 10).unwrap_or_default();
    let pris = |n: &str| existants.lines().any(|l| l.starts_with(&format!("device for {n}:")));
    let mut nom = nom_file(base);
    let mut i = 2;
    while pris(&nom) {
        nom = format!("{}_{i}", nom_file(base));
        i += 1;
    }
    nom
}

/// Adresses à essayer selon le protocole choisi
pub fn adresses(adresse: &str, protocole: u32) -> Vec<String> {
    match protocole {
        1 => vec![format!("ipp://{adresse}/ipp/print"), format!("ipp://{adresse}/ipp"), format!("ipps://{adresse}/ipp/print")],
        2 => vec![format!("socket://{adresse}:9100")],
        3 => vec![format!("lpd://{adresse}/queue")],
        _ => vec![format!("ipp://{adresse}/ipp/print"), format!("ipp://{adresse}/ipp"), format!("ipps://{adresse}/ipp/print")],
    }
}

fn installer(nom: &str, uri: &str, pilote: &[&str], description: &str) -> Result<String, String> {
    let mut args = vec!["-p", nom, "-E", "-v", uri];
    args.extend_from_slice(pilote);
    args.extend_from_slice(&["-D", description]);
    lancer("lpadmin", &args, 60)?;
    let _ = lancer("cupsenable", &[nom], 10);
    let _ = lancer("cupsaccept", &[nom], 10);
    // Première imprimante : par défaut
    if lancer("lpstat", &["-d"], 10).map(|d| d.contains("no system default")).unwrap_or(false) {
        let _ = lancer("lpadmin", &["-d", nom], 10);
    }
    Ok(nom.to_string())
}

/// Sans pilote : l'imprimante décrit elle-même ce qu'elle sait faire (IPP Everywhere)
pub fn ajouter_sans_pilote(adresse: &str, protocole: u32) -> Result<String, String> {
    let nom = nom_libre(&format!("Imprimante_{adresse}"));
    let mut derniere = tr("l'imprimante ne répond pas");
    for uri in adresses(adresse, protocole).into_iter().filter(|u| u.starts_with("ipp")) {
        match installer(&nom, &uri, &["-m", "everywhere"], &trv("Imprimante ({a})", &[("a", adresse)])) {
            Ok(n) => return Ok(n),
            Err(e) => derniere = e,
        }
    }
    Err(derniere)
}

/// Avec un pilote : modèle choisi dans la liste (`ppd` = nom donné par lpinfo -m) ou fichier PPD
pub fn ajouter_avec_pilote(adresse: &str, protocole: u32, ppd: Option<&str>, fichier: Option<&str>, modele: &str) -> Result<String, String> {
    let nom = nom_libre(if modele.is_empty() { adresse } else { modele });
    // Automatique avec un pilote : port 9100 d'abord (imprimantes laser), puis IPP, puis LPD
    let essais = if protocole == 0 {
        let mut v = adresses(adresse, 2);
        v.extend(adresses(adresse, 1).into_iter().take(1));
        v.extend(adresses(adresse, 3));
        v
    } else {
        adresses(adresse, protocole)
    };
    let pilote: Vec<&str> = match (ppd, fichier) {
        (_, Some(f)) => vec!["-P", f],
        (Some(p), None) => vec!["-m", p],
        _ => return Err(tr("Choisissez d'abord un pilote.")),
    };
    let description = if modele.is_empty() { adresse.to_string() } else { modele.to_string() };
    let mut derniere = String::new();
    for uri in essais {
        match installer(&nom, &uri, &pilote, &description) {
            Ok(n) => return Ok(n),
            Err(e) => derniere = e,
        }
    }
    Err(derniere)
}

/// Pilotes installés : (nom pour lpadmin, marque et modèle). Lu une fois (plusieurs milliers).
fn tous_les_pilotes() -> &'static Vec<(String, String)> {
    static PILOTES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    PILOTES.get_or_init(|| {
        lancer("lpinfo", &["-m"], 90)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once(' '))
            .filter(|(p, _)| *p != "everywhere" && !p.starts_with("driverless:"))
            .map(|(p, n)| (p.to_string(), n.trim().to_string()))
            .collect()
    })
}

/// Recherche dans les pilotes : tous les mots tapés doivent se trouver dans le nom (40 au plus)
pub fn chercher_pilotes(texte: &str) -> Vec<(String, String)> {
    let mots: Vec<String> = texte.to_lowercase().split_whitespace().map(str::to_string).collect();
    if mots.is_empty() {
        return Vec::new();
    }
    let mut r: Vec<(String, String)> = tous_les_pilotes()
        .iter()
        .filter(|(_, n)| {
            let n = n.to_lowercase();
            mots.iter().all(|m| n.contains(m.as_str()))
        })
        .take(400)
        .cloned()
        .collect();
    // Les pilotes conseillés (« recommended ») en premier, puis les noms les plus courts
    r.sort_by_key(|(_, n)| (!n.to_lowercase().contains("recommended"), n.len()));
    r.truncate(40);
    r
}

pub struct AjoutManuel {
    pub racine: gtk::Box,
    zone: gtk::Box,
    adresse: gtk::Entry,
    protocole: gtk::DropDown,
    recherche: gtk::SearchEntry,
    resultats: gtk::ListBox,
    info: gtk::Label,
    choisi: RefCell<Option<(String, String)>>,
    fichier: RefCell<Option<String>>,
    /// Après un ajout réussi : la liste des imprimantes est relue
    pub apres: RefCell<Option<Box<dyn Fn(String)>>>,
}

impl AjoutManuel {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let ouvrir = gtk::Button::with_label(&tr("＋  Ajouter une imprimante manuellement…"));
        ouvrir.add_css_class("menu");
        ouvrir.set_halign(gtk::Align::Start);
        racine.append(&ouvrir);

        let zone = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone.add_css_class("zone-mdp");
        zone.set_visible(false);
        let explication = gtk::Label::new(Some(&tr(
            "L'adresse de l'imprimante est dans son menu réseau, ou sur la page de configuration qu'elle imprime.",
        )));
        explication.set_wrap(true);
        explication.set_xalign(0.0);
        explication.add_css_class("texte-info");
        zone.append(&explication);
        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let adresse = gtk::Entry::builder().placeholder_text(tr("Adresse IP ou nom (ex. 192.168.1.20)")).hexpand(true).build();
        adresse.add_css_class("champ");
        let choix = [tr("Automatique"), tr("IPP"), tr("Port 9100 (JetDirect)"), tr("LPD")];
        let choix: Vec<&str> = choix.iter().map(|s| s.as_str()).collect();
        let protocole = gtk::DropDown::from_strings(&choix);
        ligne.append(&adresse);
        ligne.append(&protocole);
        zone.append(&ligne);

        let titre_pilote = gtk::Label::new(Some(&tr("Pilote : sans pilote si l'imprimante le permet. Sinon, tapez sa marque et son modèle :")));
        titre_pilote.set_wrap(true);
        titre_pilote.set_xalign(0.0);
        titre_pilote.add_css_class("texte-info");
        zone.append(&titre_pilote);
        let recherche = gtk::SearchEntry::builder().placeholder_text(tr("Marque et modèle (ex. Brother HL-2130)")).build();
        zone.append(&recherche);
        let resultats = gtk::ListBox::new();
        resultats.set_selection_mode(gtk::SelectionMode::Single);
        resultats.add_css_class("liste-pilotes");
        let defil = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(180)
            .child(&resultats)
            .build();
        zone.append(&defil);

        let info = gtk::Label::new(None);
        info.set_wrap(true);
        info.set_xalign(0.0);
        info.add_css_class("texte-info");
        zone.append(&info);

        let boutons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let ppd = gtk::Button::with_label(&tr("Fichier pilote (PPD)…"));
        ppd.add_css_class("menu");
        ppd.set_hexpand(true);
        ppd.set_halign(gtk::Align::Start);
        let annuler = gtk::Button::with_label(&tr("Annuler"));
        annuler.add_css_class("menu");
        let ajouter = gtk::Button::with_label(&tr("Ajouter"));
        ajouter.add_css_class("principal");
        boutons.append(&ppd);
        boutons.append(&annuler);
        boutons.append(&ajouter);
        zone.append(&boutons);
        racine.append(&zone);

        let a = Rc::new(AjoutManuel {
            racine,
            zone: zone.clone(),
            adresse: adresse.clone(),
            protocole,
            recherche: recherche.clone(),
            resultats: resultats.clone(),
            info,
            choisi: RefCell::new(None),
            fichier: RefCell::new(None),
            apres: RefCell::new(None),
        });

        let moi = Rc::downgrade(&a);
        ouvrir.connect_clicked(move |_| {
            if let Some(a) = moi.upgrade() {
                let v = !a.zone.is_visible();
                a.zone.set_visible(v);
                if v {
                    a.adresse.grab_focus();
                }
            }
        });
        let moi = Rc::downgrade(&a);
        annuler.connect_clicked(move |_| {
            if let Some(a) = moi.upgrade() {
                a.remettre();
            }
        });
        let moi = Rc::downgrade(&a);
        recherche.connect_search_changed(move |e| {
            if let Some(a) = moi.upgrade() {
                a.chercher(e.text().to_string());
            }
        });
        let moi = Rc::downgrade(&a);
        resultats.connect_row_selected(move |_, ligne| {
            let Some(a) = moi.upgrade() else { return };
            if let Some(l) = ligne {
                let ppd = l.widget_name().to_string();
                let nom = l.tooltip_text().map(|t| t.to_string()).unwrap_or_default();
                *a.fichier.borrow_mut() = None;
                a.info.set_label(&trv("Pilote choisi : {p}", &[("p", &nom)]));
                *a.choisi.borrow_mut() = Some((ppd, nom));
            }
        });
        let moi = Rc::downgrade(&a);
        ppd.connect_clicked(move |b| {
            let Some(a) = moi.upgrade() else { return };
            let dialogue = gtk::FileChooserNative::new(
                Some(&tr("Fichier pilote (PPD)")),
                b.root().and_downcast::<gtk::Window>().as_ref(),
                gtk::FileChooserAction::Open,
                Some(&tr("Ouvrir")),
                Some(&tr("Annuler")),
            );
            let filtre = gtk::FileFilter::new();
            filtre.add_pattern("*.ppd");
            filtre.add_pattern("*.PPD");
            filtre.add_pattern("*.ppd.gz");
            filtre.set_name(Some(&tr("Pilote d'imprimante (PPD)")));
            dialogue.add_filter(&filtre);
            let moi = Rc::downgrade(&a);
            dialogue.connect_response(move |d, r| {
                if r == gtk::ResponseType::Accept {
                    if let (Some(a), Some(chemin)) = (moi.upgrade(), d.file().and_then(|f| f.path())) {
                        let c = chemin.to_string_lossy().to_string();
                        a.resultats.unselect_all();
                        *a.choisi.borrow_mut() = None;
                        a.info.set_label(&trv("Pilote choisi : {p}", &[("p", &c)]));
                        *a.fichier.borrow_mut() = Some(c);
                    }
                }
                d.destroy();
            });
            dialogue.show();
            std::mem::forget(dialogue);
        });
        let moi = Rc::downgrade(&a);
        ajouter.connect_clicked(move |_| {
            if let Some(a) = moi.upgrade() {
                a.ajouter();
            }
        });
        let moi = Rc::downgrade(&a);
        adresse.connect_activate(move |_| {
            if let Some(a) = moi.upgrade() {
                a.ajouter();
            }
        });
        a
    }

    fn remettre(&self) {
        self.zone.set_visible(false);
        self.adresse.set_text("");
        self.recherche.set_text("");
        self.info.set_label("");
        *self.choisi.borrow_mut() = None;
        *self.fichier.borrow_mut() = None;
    }

    /// Imprimante trouvée mais impossible à installer sans pilote : on ouvre l'ajout manuel
    /// avec son adresse, et on propose de choisir le pilote
    pub fn preparer(&self, uri: &str, modele: &str) {
        let hote = uri.split("://").nth(1).unwrap_or("").split(['/', '?']).next().unwrap_or("");
        let hote = hote.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map(|(h, _)| h).unwrap_or(hote);
        if adresse_valide(hote) {
            self.adresse.set_text(hote);
        }
        self.zone.set_visible(true);
        self.recherche.set_text(modele);
        self.info.set_label(&tr("Cette imprimante demande un pilote : choisissez son modèle dans la liste, puis « Ajouter »."));
    }

    fn chercher(self: &Rc<Self>, texte: String) {
        let moi = Rc::downgrade(self);
        let t = texte.clone();
        en_fond(
            move || chercher_pilotes(&t),
            move |liste| {
                let Some(a) = moi.upgrade() else { return };
                if a.recherche.text() != texte {
                    return; // tapé autre chose entre-temps
                }
                while let Some(e) = a.resultats.first_child() {
                    a.resultats.remove(&e);
                }
                for (ppd, nom) in liste {
                    let l = gtk::Label::new(Some(&nom));
                    l.set_xalign(0.0);
                    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    let ligne = gtk::ListBoxRow::new();
                    ligne.set_child(Some(&l));
                    ligne.set_widget_name(&ppd);
                    ligne.set_tooltip_text(Some(&nom));
                    a.resultats.append(&ligne);
                }
            },
        );
    }

    fn ajouter(self: &Rc<Self>) {
        let adresse = self.adresse.text().trim().to_string();
        if !adresse_valide(&adresse) {
            self.info.set_label(&tr("Tapez l'adresse IP ou le nom de l'imprimante."));
            return;
        }
        let protocole = self.protocole.selected();
        let choisi = self.choisi.borrow().clone();
        let fichier = self.fichier.borrow().clone();
        self.info.set_label(&tr("Ajout de l'imprimante…"));
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                if choisi.is_none() && fichier.is_none() {
                    if protocole == 2 || protocole == 3 {
                        return Err(tr("Avec ce protocole, choisissez le pilote de l'imprimante (marque et modèle)."));
                    }
                    return ajouter_sans_pilote(&adresse, protocole).map_err(|_| {
                        tr("Cette imprimante ne fonctionne pas sans pilote, ou ne répond pas. Vérifiez l'adresse, puis choisissez son modèle dans la liste.")
                    });
                }
                let (ppd, modele) = choisi.map(|(p, n)| (Some(p), n)).unwrap_or((None, String::new()));
                ajouter_avec_pilote(&adresse, protocole, ppd.as_deref(), fichier.as_deref(), &modele)
            },
            move |r| {
                let Some(a) = moi.upgrade() else { return };
                match r {
                    Ok(nom) => {
                        a.remettre();
                        if let Some(f) = a.apres.borrow().as_ref() {
                            f(nom);
                        }
                    }
                    Err(e) => a.info.set_label(&e),
                }
            },
        );
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn adresses_et_noms() {
        assert!(adresse_valide("192.168.1.20"));
        assert!(adresse_valide("imprimante-bureau.local"));
        assert!(!adresse_valide("1.2.3.4; reboot"));
        assert!(!adresse_valide(""));
        assert_eq!(adresses("10.0.0.5", 2), vec!["socket://10.0.0.5:9100".to_string()]);
        assert_eq!(nom_file("Brother HL-2130 series"), "Brother_HL-2130_series");
    }
}
