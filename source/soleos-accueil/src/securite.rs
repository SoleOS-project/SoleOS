//! SoleOS – code, clé de secours et verrouillage de l'écran.
//!
//! Le travail sur les données chiffrées est fait par /usr/local/sbin/soleos-securite
//! (lancé en administrateur via sudo, règles limitées à ses seules actions).
//! Les codes passent toujours par l'entrée standard, jamais en argument.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

pub const OUTIL: &str = "/usr/local/sbin/soleos-securite";
pub const LONGUEUR_CODE: usize = 6;
pub const LONGUEUR_MDP: usize = 8;

/// État de la protection (sortie de « soleos-securite etat »)
#[derive(Default, Clone, Debug)]
pub struct Etat {
    pub chiffre: bool,
    /// « provisoire » (premier démarrage), « code », « motdepasse » ou « aucune »
    pub methode: String,
    pub tpm: bool,
    pub secours_utilisee: bool,
    pub verrouillable: bool,
}

impl Etat {
    /// Code à 6 chiffres (puce TPM) plutôt que mot de passe
    pub fn par_code(&self) -> bool {
        match self.methode.as_str() {
            "code" => true,
            "motdepasse" => false,
            _ => self.tpm,
        }
    }
    /// Texte à afficher selon la protection : version « code » ou version « mot de passe »
    /// (phrases entières, pour que chaque langue puisse les traduire correctement)
    pub fn selon(&self, code: &str, mot_de_passe: &str) -> String {
        if self.par_code() { crate::tr::tr(code) } else { crate::tr::tr(mot_de_passe) }
    }
}

fn commande(args: &[&str]) -> Command {
    if let Some(autre) = std::env::var_os("SOLEOS_SECURITE_TEST") {
        let mut c = Command::new(autre);
        c.args(args);
        return c;
    }
    let mut c = Command::new("sudo");
    c.arg("-n").arg(OUTIL).args(args);
    c
}

/// Lance une action ; `entree` est écrite sur l'entrée standard.
/// Renvoie les lignes de sortie, ou le message d'erreur (« ERREUR|… »).
pub fn appeler(args: &[&str], entree: &str) -> Result<Vec<String>, String> {
    let mut enfant = commande(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::tr::trv("Outil de sécurité introuvable ({e}).", &[("e", &e.to_string())]))?;
    if let Some(mut e) = enfant.stdin.take() {
        let _ = e.write_all(entree.as_bytes());
    }
    let mut sortie = String::new();
    if let Some(mut s) = enfant.stdout.take() {
        let _ = s.read_to_string(&mut sortie);
    }
    let mut erreurs = String::new();
    if let Some(mut s) = enfant.stderr.take() {
        let _ = s.read_to_string(&mut erreurs);
    }
    let statut = enfant.wait().map_err(|e| e.to_string())?;
    let lignes: Vec<String> = sortie.lines().map(|l| l.trim().to_string()).collect();
    if let Some(e) = lignes.iter().find_map(|l| l.strip_prefix("ERREUR|")) {
        return Err(e.to_string());
    }
    if !statut.success() {
        let detail = erreurs.lines().last().unwrap_or("").trim().to_string();
        return Err(if detail.is_empty() { crate::tr::tr("L'opération a échoué.") } else { detail });
    }
    Ok(lignes)
}

pub fn analyser_etat(texte: &str) -> Etat {
    let mut e = Etat::default();
    for l in texte.lines() {
        let Some((k, v)) = l.trim().split_once('=') else { continue };
        match k {
            "CHIFFRE" => e.chiffre = v == "1",
            "METHODE" => e.methode = v.to_string(),
            "TPM" => e.tpm = v == "1",
            "SECOURS_UTILISEE" => e.secours_utilisee = v == "1",
            "VERROUILLABLE" => e.verrouillable = v == "1",
            _ => {}
        }
    }
    e
}

pub fn etat() -> Etat {
    match appeler(&["etat"], "") {
        Ok(l) => analyser_etat(&l.join("\n")),
        Err(_) => Etat::default(),
    }
}

/// Clé de secours dans la sortie (« CLE=… »)
pub fn cle_dans(lignes: &[String]) -> Option<String> {
    lignes.iter().find_map(|l| l.strip_prefix("CLE=")).map(str::to_string)
}

/// Chiffre tapé (seuls les vrais chiffres comptent : rangée du haut avec Maj, ou pavé numérique)
pub fn chiffre(c: char) -> Option<char> {
    c.is_ascii_digit().then_some(c)
}

/// Signe de la rangée du haut d'un clavier AZERTY tapé sans Maj (&é"'(-è_çà)
pub fn signe_azerty(c: char) -> bool {
    "&é\"'(-è_çà".contains(c)
}

pub fn conseil_maj_texte() -> String {
    crate::tr::tr("Pour taper les chiffres : touche Maj (⇧) ou pavé numérique.")
}

/// Conseil à afficher si le texte contient des signes AZERTY tapés sans Maj
pub fn conseil_maj(texte: &str) -> Option<String> {
    // Une clé de secours (lettres et tirets) n'est pas concernée
    (!texte.chars().any(|c| c.is_ascii_alphabetic()) && texte.chars().any(signe_azerty)).then(conseil_maj_texte)
}

pub fn normaliser_code(texte: &str) -> String {
    texte.chars().filter(|c| !c.is_whitespace()).collect()
}

pub fn code_valide(texte: &str) -> bool {
    let c = normaliser_code(texte);
    c.len() == LONGUEUR_CODE && c.chars().all(|c| c.is_ascii_digit())
}

/// Clé de secours présentée en deux lignes de quatre groupes
pub fn cle_sur_deux_lignes(cle: &str) -> String {
    let groupes: Vec<&str> = cle.split('-').collect();
    if groupes.len() == 8 {
        format!("{}\n{}", groupes[..4].join("  "), groupes[4..].join("  "))
    } else {
        cle.to_string()
    }
}

// ---------------------------------------------------------------------
//  Verrouillage de l'écran

fn fichier_delai() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("soleos").join("verrouillage"))
}

/// Délai avant verrouillage, en minutes (0 = jamais ; 10 par défaut)
pub fn delai_verrouillage() -> u32 {
    fichier_delai()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(10)
}

pub fn choisir_delai(minutes: u32) {
    if crate::plasma::actif() {
        crate::plasma::delai_verrouillage(minutes);
    }
    if let Some(f) = fichier_delai() {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, minutes.to_string());
    }
    // La surveillance redémarre seule avec le nouveau délai
    let _ = Command::new("pkill").args(["-u", &utilisateur_id(), "-x", "swayidle"]).status();
}

fn utilisateur_id() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("Uid:")).map(|l| l.split_whitespace().nth(1).unwrap_or("").to_string()))
        .unwrap_or_default()
}

pub fn verrouiller() {
    if crate::plasma::actif() {
        crate::plasma::verrouiller();
        return;
    }
    crate::lancer(&["soleos-verrouiller"]);
}

// ---------------------------------------------------------------------
//  Pavé du code à 6 chiffres : points, touches à l'écran (écran tactile) et clavier

pub struct Pave {
    pub racine: gtk::Box,
    points: Vec<gtk::Box>,
    pub message: gtk::Label,
    valeur: RefCell<String>,
    rappel: RefCell<Option<Box<dyn Fn(String)>>>,
}

pub const STYLE_PAVE: &str = r#"
.point-code { min-width: 18px; min-height: 18px; border-radius: 9px; border: 2px solid alpha(white, 0.55); background: none; }
.point-code.plein { background: #FFC23D; border-color: #FFC23D; }
button.touche {
  min-width: 76px; min-height: 76px; border-radius: 38px; padding: 0;
  background: #141A2E; color: #E9ECF5; border: 1px solid #2A3352; box-shadow: none; outline: none;
  font-size: 28px; font-weight: 500; font-family: "Space Grotesk", "Inter", sans-serif;
}
.compact button.touche { min-width: 64px; min-height: 64px; border-radius: 32px; font-size: 24px; }
.mini button.touche { min-width: 54px; min-height: 54px; border-radius: 27px; font-size: 21px; }
button.touche:hover { background: #1A2138; border-color: #FFC23D; }
button.touche:active { background: #232B47; }
button.touche label { color: #f2f4f8; }
button.touche.vide { background: none; border: none; }
.message-code { color: #FF9B6B; font-size: 16px; min-height: 22px; }
"#;

impl Pave {
    pub fn nouveau() -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 18);
        racine.set_halign(gtk::Align::Center);
        let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        rangee.set_halign(gtk::Align::Center);
        let mut points = Vec::new();
        for _ in 0..LONGUEUR_CODE {
            let p = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            p.add_css_class("point-code");
            p.set_valign(gtk::Align::Center);
            rangee.append(&p);
            points.push(p);
        }
        racine.append(&rangee);
        let message = gtk::Label::new(None);
        message.add_css_class("message-code");
        racine.append(&message);

        let grille = gtk::Grid::new();
        grille.set_row_spacing(14);
        grille.set_column_spacing(22);
        grille.set_halign(gtk::Align::Center);
        racine.append(&grille);

        let pave = Rc::new(Pave { racine, points, message, valeur: RefCell::new(String::new()), rappel: RefCell::new(None) });
        let touches = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "", "0", "⌫"];
        for (i, t) in touches.iter().enumerate() {
            let b = gtk::Button::with_label(t);
            b.add_css_class("touche");
            b.set_focusable(false);
            if t.is_empty() {
                b.add_css_class("vide");
                b.set_sensitive(false);
            }
            let moi = Rc::downgrade(&pave);
            let t = t.to_string();
            b.connect_clicked(move |_| {
                let Some(p) = moi.upgrade() else { return };
                if t == "⌫" {
                    p.effacer();
                } else if let Some(c) = t.chars().next() {
                    p.ajouter(c);
                }
            });
            grille.attach(&b, (i % 3) as i32, (i / 3) as i32, 1, 1);
        }
        pave
    }

    /// Appelé quand les 6 chiffres sont tapés
    pub fn quand_complet(&self, f: impl Fn(String) + 'static) {
        *self.rappel.borrow_mut() = Some(Box::new(f));
    }

    #[allow(dead_code)]
    pub fn vider(&self) {
        self.valeur.borrow_mut().clear();
        self.dessiner();
    }

    pub fn dire(&self, texte: &str) {
        self.message.set_label(texte);
    }

    fn dessiner(&self) {
        let n = self.valeur.borrow().chars().count();
        for (i, p) in self.points.iter().enumerate() {
            if i < n { p.add_css_class("plein") } else { p.remove_css_class("plein") }
        }
    }

    fn ajouter(self: &Rc<Self>, c: char) {
        let Some(c) = chiffre(c) else { return };
        if self.valeur.borrow().len() >= LONGUEUR_CODE {
            return;
        }
        self.valeur.borrow_mut().push(c);
        self.message.set_label("");
        self.dessiner();
        if self.valeur.borrow().len() == LONGUEUR_CODE {
            let code = self.valeur.borrow().clone();
            // Petit temps pour voir le dernier point se remplir
            let moi = Rc::downgrade(self);
            glib::timeout_add_local_once(std::time::Duration::from_millis(180), move || {
                if let Some(p) = moi.upgrade() {
                    if let Some(f) = p.rappel.borrow().as_ref() {
                        f(code);
                    }
                }
            });
        }
    }

    fn effacer(&self) {
        self.valeur.borrow_mut().pop();
        self.dessiner();
    }

    /// Le clavier physique tape aussi dans le pavé (chiffres avec Maj, pavé numérique)
    pub fn ecouter_clavier(self: &Rc<Self>, cible: &impl IsA<gtk::Widget>) -> gtk::EventControllerKey {
        let touches = gtk::EventControllerKey::new();
        touches.set_propagation_phase(gtk::PropagationPhase::Capture);
        // Référence forte : le pavé vit aussi longtemps que l'écoute du clavier
        let p = self.clone();
        touches.connect_key_pressed(move |_, cle, _, _| {
            if std::env::var_os("SOLEOS_DEBUG").is_some() {
                eprintln!("touche {:?} {:?} affiché={}", cle.name(), cle.to_unicode(), p.racine.is_mapped());
            }
            if !p.racine.is_mapped() {
                return glib::Propagation::Proceed;
            }
            if cle == gdk::Key::BackSpace {
                p.effacer();
                return glib::Propagation::Stop;
            }
            let pave_num = [
                gdk::Key::KP_0, gdk::Key::KP_1, gdk::Key::KP_2, gdk::Key::KP_3, gdk::Key::KP_4,
                gdk::Key::KP_5, gdk::Key::KP_6, gdk::Key::KP_7, gdk::Key::KP_8, gdk::Key::KP_9,
            ];
            // Pavé numérique avec Verr Num éteint : les touches donnent Fin, Bas… → on prend leur chiffre
            let pave_sans_verr = [
                gdk::Key::KP_Insert, gdk::Key::KP_End, gdk::Key::KP_Down, gdk::Key::KP_Page_Down, gdk::Key::KP_Left,
                gdk::Key::KP_Begin, gdk::Key::KP_Right, gdk::Key::KP_Home, gdk::Key::KP_Up, gdk::Key::KP_Page_Up,
            ];
            let c = match pave_num.iter().position(|k| *k == cle).or_else(|| pave_sans_verr.iter().position(|k| *k == cle)) {
                Some(i) => char::from_digit(i as u32, 10),
                None => cle.to_unicode(),
            };
            match c {
                Some(ch) if ch.is_ascii_digit() => {
                    p.ajouter(ch);
                    glib::Propagation::Stop
                }
                Some(ch) if signe_azerty(ch) => {
                    p.dire(&conseil_maj_texte());
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        cible.add_controller(touches.clone());
        touches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seuls_les_vrais_chiffres() {
        assert_eq!(chiffre('5'), Some('5'));
        assert_eq!(chiffre('('), None);
        assert!(code_valide("582739"));
        assert!(!code_valide("(_éè\"ç"));
        assert!(conseil_maj("(_éè\"ç").is_some());
        assert!(conseil_maj("dlgeuvtd-cgnejtgg").is_none());
        assert!(conseil_maj("582739").is_none());
    }
}
