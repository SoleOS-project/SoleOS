//! Panneau « Appareils » : imprimantes (et scanner), Bluetooth, écran externe.
//! S'appuie sur les outils standards : CUPS (lpstat, lpinfo, lpadmin, lp),
//! BlueZ (bluetoothctl) et wlr-randr. Les commandes tournent dans un fil à part.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use gtk::prelude::*;

use crate::icones;
use crate::wifi::en_fond;
use crate::tr::{n_, tr, trv};

// =====================================================================
//  Outils communs
// =====================================================================

/// Lance une commande et attend sa fin, au plus `delai` secondes (un outil bloqué ne fige jamais le panneau).
fn sortie_delai(prog: &str, args: &[&str], delai: u64) -> Result<String, String> {
    use std::io::Read;
    use std::process::Stdio;
    // Les outils d'administration (lpinfo, lpadmin…) sont dans /usr/sbin, absent du PATH des utilisateurs
    let chemin = chemin_outil(prog).unwrap_or_else(|| prog.to_string());
    let mut enfant = Command::new(&chemin)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Lecture des sorties dans des fils à part (sinon une sortie volumineuse bloquerait la commande)
    let mut o = enfant.stdout.take().unwrap();
    let mut e = enfant.stderr.take().unwrap();
    let lo = std::thread::spawn(move || { let mut t = String::new(); let _ = o.read_to_string(&mut t); t });
    let le = std::thread::spawn(move || { let mut t = String::new(); let _ = e.read_to_string(&mut t); t });
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(delai);
    let statut = loop {
        match enfant.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if std::time::Instant::now() < fin => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                break None;
            }
        }
    };
    let (texte, erreur) = (lo.join().unwrap_or_default(), le.join().unwrap_or_default());
    match statut {
        Some(st) if st.success() => Ok(texte),
        Some(_) => Err(if erreur.trim().is_empty() { texte.trim().to_string() } else { erreur.trim().to_string() }),
        None => Err(trv("{prog} ne répond pas.", &[("prog", prog)])),
    }
}

fn sortie(prog: &str, args: &[&str]) -> Result<String, String> {
    sortie_delai(prog, args, 20)
}

fn chemin_outil(prog: &str) -> Option<String> {
    if prog.contains('/') {
        return Some(prog.to_string());
    }
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .chain(["/usr/local/sbin", "/usr/sbin", "/sbin", "/usr/bin", "/bin"])
        .map(|d| std::path::Path::new(d).join(prog))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
}

fn existe(prog: &str) -> bool {
    chemin_outil(prog).is_some()
}

fn vider(boite: &gtk::Box) {
    while let Some(enfant) = boite.first_child() {
        boite.remove(&enfant);
    }
}

/// (le texte est traduit ici : on peut passer directement le texte français)
fn etiquette(texte: &str, classe: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&tr(texte)));
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_max_width_chars(40);
    l.set_width_chars(40);
    if !classe.is_empty() {
        l.add_css_class(classe);
    }
    l
}

fn petit_bouton(texte: &str, classe: &str) -> gtk::Button {
    let b = gtk::Button::with_label(&tr(texte));
    b.add_css_class("petit");
    if !classe.is_empty() {
        b.add_css_class(classe);
    }
    b.set_valign(gtk::Align::Center);
    b
}

/// Ligne : icône, nom, détail en dessous, et des boutons à droite.
fn ligne(icone: &str, nom: &str, detail: &str, actif: bool, boutons: &[gtk::Button]) -> gtk::Box {
    let image = icones::image(icone, 20);
    let titre = gtk::Label::new(Some(&tr(nom)));
    titre.set_xalign(0.0);
    titre.set_ellipsize(gtk::pango::EllipsizeMode::End);
    titre.add_css_class("nom-appareil");
    let textes = gtk::Box::new(gtk::Orientation::Vertical, 1);
    textes.set_hexpand(true);
    textes.set_valign(gtk::Align::Center);
    textes.append(&titre);
    if !detail.is_empty() {
        let d = gtk::Label::new(Some(&tr(detail)));
        d.set_xalign(0.0);
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        d.add_css_class("texte-info");
        textes.append(&d);
    }
    let l = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    l.add_css_class("appareil-ligne");
    if actif {
        l.add_css_class("appareil-actif");
    }
    l.append(&image);
    l.append(&textes);
    for b in boutons {
        l.append(b);
    }
    l
}

// =====================================================================
//  Imprimantes (CUPS)
// =====================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct Imprimante {
    pub nom: String,
    pub uri: String,
    pub etat: String,
    pub par_defaut: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trouvee {
    pub uri: String,
    pub info: String,
    pub modele: String,
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct EtatImpression {
    pub outil: bool,
    pub service: bool,
    pub installees: Vec<Imprimante>,
    pub trouvees: Vec<Trouvee>,
}

/// « device for Bureau: ipp://… » → [(Bureau, ipp://…)]
pub fn analyser_lpstat_v(texte: &str) -> Vec<(String, String)> {
    texte
        .lines()
        .filter_map(|l| {
            let reste = l.strip_prefix("device for ")?;
            let (nom, uri) = reste.split_once(": ")?;
            Some((nom.trim().to_string(), uri.trim().to_string()))
        })
        .collect()
}

/// Sortie de « lpinfo -l -v » : un bloc par appareil.
pub fn analyser_lpinfo(texte: &str) -> Vec<Trouvee> {
    let mut liste: Vec<Trouvee> = Vec::new();
    for l in texte.lines() {
        let l = l.trim();
        if let Some(uri) = l.strip_prefix("Device: uri = ") {
            liste.push(Trouvee { uri: uri.trim().to_string(), info: String::new(), modele: String::new(), id: String::new() });
        } else if let Some(courant) = liste.last_mut() {
            if let Some(v) = l.strip_prefix("info = ") {
                courant.info = v.trim().to_string();
            } else if let Some(v) = l.strip_prefix("make-and-model = ") {
                courant.modele = v.trim().to_string();
            } else if let Some(v) = l.strip_prefix("device-id = ") {
                courant.id = v.trim().to_string();
            }
        }
    }
    // Seules les vraies imprimantes : réseau (annoncées sur le réseau local) ou USB, avec un nom
    let reseau_local = |u: &str| u.starts_with("dnssd://") || ((u.starts_with("ipp://") || u.starts_with("ipps://")) && u.contains(".local"));
    liste.retain(|t| {
        (reseau_local(&t.uri) || t.uri.starts_with("usb://"))
            && !(t.info.is_empty() && t.modele.is_empty())
            && !t.info.to_lowercase().contains("fax")
            && !t.uri.contains("_fax")
            // Serveurs d'impression des box (« bbox Ippos Printer »…) : rien de connu derrière
            && !(t.modele == "Unknown" && t.id.is_empty())
    });
    // Une même imprimante peut apparaître plusieurs fois (dnssd, ipps, USB via ipp-usb) :
    // on garde la version réseau (sans pilote) en priorité.
    let rang = |u: &str| if u.starts_with("dnssd://") { 0 } else if u.starts_with("ipps://") { 1 } else if u.starts_with("ipp://") { 2 } else { 3 };
    liste.sort_by_key(|t| rang(&t.uri));
    let mut gardees: Vec<Trouvee> = Vec::new();
    for t in liste {
        let n = nom_lisible(&t).to_lowercase();
        let doublon = gardees.iter().any(|g| {
            let m = nom_lisible(g).to_lowercase();
            g.uri == t.uri || m == n || (t.uri.starts_with("usb://") && (m.contains(&n) || n.contains(&m)))
        });
        if !doublon {
            gardees.push(t);
        }
    }
    gardees
}

pub fn nom_lisible(t: &Trouvee) -> String {
    let base = if !t.info.is_empty() && t.info != "Unknown" { &t.info } else { &t.modele };
    // « HP LaserJet @ maison » → « HP LaserJet » ; « … (driverless) » → « … »
    let n = base.split(" @ ").next().unwrap_or(base).trim();
    n.trim_end_matches("(driverless)").trim().to_string()
}

/// Nom de file d'impression accepté par CUPS (lettres, chiffres, _ et -).
pub fn nom_file(texte: &str) -> String {
    let sans_accents = |c: char| match c {
        'à' | 'â' | 'ä' | 'á' => 'a', 'À' | 'Â' | 'Ä' | 'Á' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e', 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'î' | 'ï' | 'í' => 'i', 'Î' | 'Ï' | 'Í' => 'I',
        'ô' | 'ö' | 'ó' => 'o', 'Ô' | 'Ö' | 'Ó' => 'O',
        'ù' | 'û' | 'ü' | 'ú' => 'u', 'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
        'ç' => 'c', 'Ç' => 'C',
        c => c,
    };
    let mut n: String = texte
        .chars()
        .map(sans_accents)
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    while n.contains("__") {
        n = n.replace("__", "_");
    }
    let n = n.trim_matches('_').chars().take(40).collect::<String>();
    if n.is_empty() { tr("Imprimante") } else { n }
}

fn etat_lisible(ligne_lpstat_p: &str) -> String {
    if ligne_lpstat_p.contains("disabled") {
        tr("Arrêtée")
    } else if ligne_lpstat_p.contains("now printing") {
        tr("Impression en cours…")
    } else {
        tr("Prête")
    }
}

pub fn lire_imprimantes(chercher: bool) -> EtatImpression {
    let mut etat = EtatImpression { outil: existe("lpstat"), ..Default::default() };
    if !etat.outil {
        return etat;
    }
    let liste = sortie("lpstat", &["-v"]).unwrap_or_default();
    etat.service = sortie("lpstat", &["-r"]).map(|s| s.contains("is running")).unwrap_or(false);
    let defaut = sortie("lpstat", &["-d"]).unwrap_or_default();
    let defaut = defaut.split(": ").nth(1).unwrap_or("").trim().to_string();
    let etats = sortie("lpstat", &["-p"]).unwrap_or_default();
    for (nom, uri) in analyser_lpstat_v(&liste) {
        let ligne_etat = etats
            .lines()
            .find(|l| l.starts_with(&format!("printer {nom} ")))
            .unwrap_or("");
        etat.installees.push(Imprimante {
            par_defaut: nom == defaut,
            etat: etat_lisible(ligne_etat),
            nom,
            uri,
        });
    }
    if chercher && etat.service {
        let trouvees = sortie("lpinfo", &["--timeout", "8", "--include-schemes", "dnssd,usb,driverless", "-l", "-v"]).unwrap_or_default();
        etat.trouvees = analyser_lpinfo(&trouvees)
            .into_iter()
            .filter(|t| !etat.installees.iter().any(|i| i.uri == t.uri))
            .collect();
    }
    etat
}

/// Ajoute une imprimante trouvée. Sans pilote si elle le permet, sinon pilote installé correspondant.
pub fn ajouter_imprimante(t: &Trouvee, deja: &[Imprimante]) -> Result<String, String> {
    let mut nom = nom_file(&nom_lisible(t));
    let mut i = 2;
    while deja.iter().any(|d| d.nom.eq_ignore_ascii_case(&nom)) {
        nom = format!("{}_{i}", nom_file(&nom_lisible(t)));
        i += 1;
    }
    let description = nom_lisible(t);
    let mut essais: Vec<String> = Vec::new();
    if t.uri.starts_with("dnssd://") {
        essais.push("everywhere".into());
    }
    if !t.id.is_empty() {
        if let Ok(pilotes) = sortie("lpinfo", &["--device-id", &t.id, "-m"]) {
            if let Some(p) = pilotes.lines().next().and_then(|l| l.split_whitespace().next()) {
                essais.push(p.to_string());
            }
        }
    }
    if !essais.iter().any(|e| e == "everywhere") {
        essais.push("everywhere".into());
    }
    let mut derniere = tr("Aucun pilote ne convient à cette imprimante.");
    for pilote in essais {
        match sortie("lpadmin", &["-p", &nom, "-E", "-v", &t.uri, "-m", &pilote, "-D", &description]) {
            Ok(_) => {
                let _ = sortie("cupsenable", &[&nom]);
                let _ = sortie("cupsaccept", &[&nom]);
                if deja.is_empty() {
                    let _ = sortie("lpadmin", &["-d", &nom]);
                }
                return Ok(nom);
            }
            Err(e) => derniere = e,
        }
    }
    Err(derniere)
}

pub fn imprimante_par_defaut(nom: &str) -> Result<String, String> {
    sortie("lpadmin", &["-d", nom])
}

pub fn supprimer_imprimante(nom: &str) -> Result<String, String> {
    sortie("lpadmin", &["-x", nom])
}

pub fn page_de_test(nom: &str) -> Result<String, String> {
    for f in ["/usr/share/cups/data/testprint", "/usr/share/cups/data/default-testpage.pdf"] {
        if std::path::Path::new(f).exists() {
            return sortie("lp", &["-d", nom, "-t", "Page de test SoleOS", f]);
        }
    }
    Err(tr("Page de test introuvable."))
}

// =====================================================================
//  Bluetooth (BlueZ)
// =====================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct AppareilBt {
    pub adresse: String,
    pub nom: String,
    pub associe: bool,
    pub connecte: bool,
    pub icone: String,
}

#[derive(Clone, Debug, Default)]
pub struct EtatBluetooth {
    pub outil: bool,
    pub adaptateur: bool,
    pub allume: bool,
    pub appareils: Vec<AppareilBt>,
}

/// « Device AA:BB:… Nom » → [(AA:BB:…, Nom)]
pub fn analyser_devices(texte: &str) -> Vec<(String, String)> {
    texte
        .lines()
        .filter_map(|l| {
            let reste = l.trim().strip_prefix("Device ")?;
            let (adresse, nom) = reste.split_once(' ').unwrap_or((reste, ""));
            Some((adresse.to_string(), nom.trim().to_string()))
        })
        .collect()
}

fn bt(args: &[&str]) -> Result<String, String> {
    // --timeout N : l'outil s'arrête de lui-même ; on lui laisse quelques secondes de plus
    let delai = args.windows(2).find(|w| w[0] == "--timeout").and_then(|w| w[1].parse::<u64>().ok()).unwrap_or(5) + 5;
    sortie_delai("bluetoothctl", args, delai)
}

/// Une carte Bluetooth est-elle présente ? (sans interroger le service, qui peut être arrêté)
pub fn carte_bluetooth() -> bool {
    std::fs::read_dir("/sys/class/bluetooth")
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("hci")))
        .unwrap_or(false)
}

pub fn lire_bluetooth(chercher: bool) -> EtatBluetooth {
    let mut etat = EtatBluetooth { outil: existe("bluetoothctl"), ..Default::default() };
    if !etat.outil {
        return etat;
    }
    if !carte_bluetooth() {
        return etat;
    }
    let liste = bt(&["list"]).unwrap_or_default();
    etat.adaptateur = liste.lines().any(|l| l.starts_with("Controller "));
    if !etat.adaptateur {
        return etat;
    }
    etat.allume = bt(&["show"]).map(|s| s.contains("Powered: yes")).unwrap_or(false);
    if !etat.allume {
        return etat;
    }
    if chercher {
        let _ = bt(&["--timeout", "8", "scan", "on"]);
    }
    let associes: Vec<String> = analyser_devices(&bt(&["devices", "Paired"]).unwrap_or_default())
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    let connectes: Vec<String> = analyser_devices(&bt(&["devices", "Connected"]).unwrap_or_default())
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    for (adresse, nom) in analyser_devices(&bt(&["devices"]).unwrap_or_default()) {
        // Appareils sans nom (adresse seule) : inutilisables pour un humain
        if nom.is_empty() || nom.replace('-', ":") == adresse {
            continue;
        }
        let info = bt(&["info", &adresse]).unwrap_or_default();
        let icone = info
            .lines()
            .find_map(|l| l.trim().strip_prefix("Icon: "))
            .unwrap_or("")
            .to_string();
        etat.appareils.push(AppareilBt {
            associe: associes.contains(&adresse) || info.contains("Paired: yes"),
            connecte: connectes.contains(&adresse) || info.contains("Connected: yes"),
            adresse,
            nom,
            icone,
        });
    }
    // Associés d'abord, puis par nom
    etat.appareils.sort_by(|a, b| b.associe.cmp(&a.associe).then(a.nom.cmp(&b.nom)));
    etat
}

pub fn bt_allumer(oui: bool) -> Result<String, String> {
    if oui {
        let _ = sortie("rfkill", &["unblock", "bluetooth"]);
    }
    bt(&["power", if oui { "on" } else { "off" }])
}

/// Association : une session bluetoothctl avec un « agent » qui accepte la demande côté PC
/// (sans agent, les casques et écouteurs refusent l'association).
pub fn bt_associer(adresse: &str) -> Result<(), String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let echec = || Err(tr("L'association a échoué. Mettez l'appareil en mode « association » (souvent un appui long sur son bouton) et réessayez."));
    let Some(outil) = chemin_outil("bluetoothctl") else { return echec() };
    let Ok(mut enfant) = Command::new(outil)
        .args(["--agent", "NoInputNoOutput"])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return echec();
    };
    let mut entree = enfant.stdin.take().unwrap();
    let sortie = enfant.stdout.take().unwrap();
    let (envoi, reception) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        // bluetoothctl n'envoie pas toujours de fin de ligne après une question : lecture par morceaux
        let mut lecteur = BufReader::new(sortie);
        let mut tampon = Vec::new();
        while let Ok(n) = lecteur.read_until(b'\n', &mut tampon) {
            if n == 0 { break; }
            let ligne = String::from_utf8_lossy(&tampon).to_string();
            if std::env::var_os("SOLEOS_DEBUG").is_some() {
                eprint!("bt> {ligne}");
            }
            let _ = envoi.send(ligne);
            tampon.clear();
        }
    });
    let mut ecrire = |t: &str| { let _ = entree.write_all(format!("{t}\n").as_bytes()); let _ = entree.flush(); };
    std::thread::sleep(Duration::from_millis(800));
    // Note : l'agent de cette session sert à l'association qu'elle lance (pas besoin d'être l'agent par défaut)
    // L'appareil n'est gardé en mémoire que peu de temps après une recherche : on le recherche à nouveau
    while reception.try_recv().is_ok() {}
    ecrire("scan on");
    let attente = Instant::now() + Duration::from_secs(12);
    while Instant::now() < attente {
        if let Ok(l) = reception.recv_timeout(Duration::from_millis(300)) {
            if l.contains(&format!("Device {adresse}")) && (l.contains("NEW") || l.contains("CHG")) {
                break;
            }
        }
    }
    ecrire("scan off");
    std::thread::sleep(Duration::from_millis(300));
    ecrire(&format!("pair {adresse}"));
    let fin = Instant::now() + Duration::from_secs(40);
    let mut associe = false;
    let mut derniere_reponse = Instant::now() - Duration::from_secs(10);
    while Instant::now() < fin {
        match reception.recv_timeout(Duration::from_millis(500)) {
            Ok(ligne) => {
                if ligne.contains("Pairing successful") || ligne.contains("AlreadyExists") || ligne.contains("Paired: yes") {
                    associe = true;
                    break;
                }
                if ligne.contains("Failed to pair") || ligne.contains("not available") {
                    break;
                }
                // Questions de l'agent (« Confirm passkey … (yes/no) », « Authorize service … ») : on accepte
                if ligne.contains("(yes/no)") && derniere_reponse.elapsed() > Duration::from_millis(500) {
                    ecrire("yes");
                    derniere_reponse = Instant::now();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    if associe {
        ecrire(&format!("trust {adresse}"));
        std::thread::sleep(Duration::from_millis(500));
        ecrire(&format!("connect {adresse}"));
        // Laisser le temps à la connexion audio, en acceptant les autorisations demandées
        let fin = Instant::now() + Duration::from_secs(12);
        while Instant::now() < fin {
            if let Ok(l) = reception.recv_timeout(Duration::from_millis(500)) {
                if l.contains("(yes/no)") { ecrire("yes"); }
                if l.contains("Connection successful") { break; }
            }
        }
    }
    ecrire("quit");
    std::thread::sleep(Duration::from_millis(300));
    let _ = enfant.kill();
    let _ = enfant.wait();
    // Vérification finale, indépendante des messages
    let info = bt(&["info", adresse]).unwrap_or_default();
    if info.contains("Paired: yes") { Ok(()) } else { echec() }
}

pub fn bt_connecter(adresse: &str, oui: bool) -> Result<String, String> {
    if oui { bt(&["--timeout", "15", "connect", adresse]) } else { bt(&["disconnect", adresse]) }
}

pub fn bt_oublier(adresse: &str) -> Result<String, String> {
    bt(&["remove", adresse])
}

fn icone_bt(icone: &str) -> &'static str {
    match icone {
        i if i.starts_with("audio") => "son",
        i if i.starts_with("input") => "bluetooth",
        "computer" | "phone" => "ecran",
        _ => "bluetooth",
    }
}

fn type_bt(icone: &str) -> &'static str {
    match icone {
        "audio-headset" | "audio-headphones" => n_("Casque"),
        "audio-card" => n_("Enceinte"),
        "input-mouse" => n_("Souris"),
        "input-keyboard" => n_("Clavier"),
        "input-gaming" => n_("Manette"),
        "phone" => n_("Téléphone"),
        "computer" => n_("Ordinateur"),
        _ => "",
    }
}

// =====================================================================
//  Écrans (wlr-randr)
// =====================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct Sortie {
    pub nom: String,
    pub description: String,
    pub actif: bool,
    /// (largeur, hauteur, Hz) disponibles
    pub modes: Vec<(u32, u32, f32)>,
    pub prefere: Option<(u32, u32, f32)>,
    pub courant: Option<(u32, u32, f32)>,
    pub echelle: f32,
    /// Taille physique en millimètres (0 si inconnue : vidéoprojecteur…)
    pub mm: (u32, u32),
}

impl Sortie {
    pub fn interne(&self) -> bool {
        ["eDP", "LVDS", "DSI"].iter().any(|p| self.nom.starts_with(p))
    }
    fn mode_base(&self) -> Option<(u32, u32, f32)> {
        let valide = |m: &(u32, u32, f32)| m.0 > 0 && m.1 > 0;
        self.prefere.filter(valide).or(self.courant.filter(valide)).or_else(|| self.modes.iter().copied().find(valide))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModeEcran {
    Dupliquer,
    Etendre,
    ExterneSeul,
    InterneSeul,
}

impl ModeEcran {
    fn code(self) -> &'static str {
        match self {
            ModeEcran::Dupliquer => "dupliquer",
            ModeEcran::Etendre => "etendre",
            ModeEcran::ExterneSeul => "externe",
            ModeEcran::InterneSeul => "interne",
        }
    }
    fn depuis(code: &str) -> Option<Self> {
        Some(match code.trim() {
            "dupliquer" => ModeEcran::Dupliquer,
            "etendre" => ModeEcran::Etendre,
            "externe" => ModeEcran::ExterneSeul,
            "interne" => ModeEcran::InterneSeul,
            _ => return None,
        })
    }
}

/// Analyse la sortie texte de wlr-randr.
pub fn analyser_wlr_randr(texte: &str) -> Vec<Sortie> {
    let mut sorties: Vec<Sortie> = Vec::new();
    let mut dans_modes = false;
    for brut in texte.lines() {
        if brut.is_empty() {
            continue;
        }
        if !brut.starts_with(' ') {
            let (nom, desc) = brut.split_once(' ').unwrap_or((brut, ""));
            sorties.push(Sortie {
                nom: nom.to_string(),
                description: desc.trim().trim_matches('"').to_string(),
                actif: false,
                modes: Vec::new(),
                prefere: None,
                courant: None,
                echelle: 1.0,
                mm: (0, 0),
            });
            dans_modes = false;
            continue;
        }
        let Some(s) = sorties.last_mut() else { continue };
        let l = brut.trim();
        if l.starts_with("Modes:") {
            dans_modes = true;
        } else if dans_modes && l.contains(" px") {
            // « 1920x1080 px, 60.000000 Hz (preferred, current) » ou « 1280x720 px (current) »
            let (taille, reste) = l.split_once(" px").unwrap();
            let (w, h) = taille.split_once('x').unwrap_or(("0", "0"));
            let hz: f32 = reste
                .strip_prefix(", ")
                .and_then(|r| r.split_whitespace().next())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);
            let m = (w.parse().unwrap_or(0), h.parse().unwrap_or(0), hz);
            if reste.contains("preferred") {
                s.prefere = Some(m);
            }
            if reste.contains("current") {
                s.courant = Some(m);
            }
            s.modes.push(m);
        } else {
            dans_modes = false;
            if let Some(v) = l.strip_prefix("Enabled: ") {
                s.actif = v.trim() == "yes";
            } else if let Some(v) = l.strip_prefix("Scale: ") {
                s.echelle = v.trim().parse().unwrap_or(1.0);
            } else if let Some(v) = l.strip_prefix("Physical size: ") {
                // « 294x165 mm »
                let v = v.trim().trim_end_matches("mm").trim();
                if let Some((w, h)) = v.split_once('x') {
                    s.mm = (w.trim().parse().unwrap_or(0), h.trim().parse().unwrap_or(0));
                }
            }
        }
    }
    sorties
}

pub fn lire_ecrans() -> Vec<Sortie> {
    sortie_delai("wlr-randr", &[], 5).map(|t| analyser_wlr_randr(&t)).unwrap_or_default()
}

/// Écran du PC (interne) et écran branché (externe).
fn repartir(sorties: &[Sortie]) -> Option<(Sortie, Sortie)> {
    if sorties.len() < 2 {
        return None;
    }
    let interne = sorties.iter().find(|s| s.interne()).unwrap_or(&sorties[0]).clone();
    let externe = sorties.iter().find(|s| s.nom != interne.nom)?.clone();
    Some((interne, externe))
}

fn mode_txt(m: (u32, u32, f32)) -> String {
    if m.2 > 0.0 { format!("{}x{}@{:.3}Hz", m.0, m.1, m.2) } else { format!("{}x{}", m.0, m.1) }
}

/// Commandes wlr-randr pour un mode d'affichage (testées sans écran).
// ---------------------------------------------------------------------
//  Taille d'affichage (agrandissement de tout l'écran)
// ---------------------------------------------------------------------

/// Choix de l'utilisateur : None = automatique (selon la finesse de l'écran)
pub fn taille_choisie() -> Option<f32> {
    let f = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config/soleos/taille"))?;
    std::fs::read_to_string(f).ok()?.trim().parse::<f32>().ok().filter(|v| (0.5..=3.0).contains(v))
}

pub fn choisir_taille(valeur: Option<f32>) {
    if let Some(f) = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config/soleos/taille")) {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, valeur.map(|v| v.to_string()).unwrap_or_else(|| "auto".into()));
    }
}

/// Agrandissement automatique : selon le nombre de points par pouce de l'écran
pub fn echelle_auto(s: &Sortie) -> f32 {
    let Some(m) = s.prefere.or(s.courant) else { return 1.0 };
    if s.mm.0 < 100 {
        return 1.0; // taille inconnue (vidéoprojecteur, machine virtuelle…)
    }
    let ppp = m.0 as f32 / (s.mm.0 as f32 / 25.4);
    // L'interface SoleOS est déjà grande (style tablette) : on agrandit moins que Windows.
    // 13-14 pouces en 1920 → 125 % ; 15,6 pouces en 1920 → 100 % ; 13 pouces en 2560 → 150 % ; 4K → 200 %
    let e: f32 = match ppp {
        p if p >= 250.0 => 2.0,
        p if p >= 190.0 => 1.5,
        p if p >= 150.0 => 1.25,
        _ => 1.0,
    };
    // Jamais moins de 720 points de haut une fois agrandi (sinon les pages seraient à l'étroit)
    let max = (m.1 as f32 / 720.0).max(1.0);
    let e = if e > max { ((max * 4.0).floor() / 4.0).max(1.0) } else { e };
    e
}

/// Agrandissement d'un écran : celui choisi pour l'écran du PC, automatique pour les autres
pub fn echelle_pour(s: &Sortie, interne: bool) -> f32 {
    if interne {
        if let Some(v) = taille_choisie() {
            return v;
        }
    }
    echelle_auto(s)
}

/// Applique l'agrandissement à tous les écrans allumés (sans deuxième écran à arranger)
pub fn appliquer_taille() {
    let sorties = lire_ecrans();
    if sorties.len() >= 2 {
        let _ = appliquer_ecran(mode_memorise());
        return;
    }
    for s in sorties.iter().filter(|s| s.actif) {
        // Toujours la résolution d'origine de l'écran (la plus nette)
        if let (Some(p), Some(c)) = (s.prefere, s.courant) {
            if (p.0, p.1) != (c.0, c.1) {
                let _ = sortie("wlr-randr", &["--output", &s.nom, "--mode", &mode_txt(p)]);
            }
        }
        let e = echelle_pour(s, true);
        if (s.echelle - e).abs() > 0.01 {
            let _ = sortie("wlr-randr", &["--output", &s.nom, "--scale", &format!("{e:.4}")]);
        }
    }
}

fn txt_echelle(e: f32) -> String {
    if (e - 1.0).abs() < 0.001 { "1".into() } else { format!("{e:.4}") }
}

pub fn commandes_ecran(sorties: &[Sortie], mode: ModeEcran) -> Vec<Vec<String>> {
    let (si, se) = match repartir(sorties) {
        Some((i, e)) => (echelle_pour(&i, true), echelle_pour(&e, false)),
        None => (1.0, 1.0),
    };
    commandes_ecran_echelles(sorties, mode, si, se)
}

/// si : agrandissement de l'écran du PC ; se : celui du deuxième écran (mode « étendre » ou seul)
pub fn commandes_ecran_echelles(sorties: &[Sortie], mode: ModeEcran, si: f32, se: f32) -> Vec<Vec<String>> {
    let Some((int, ext)) = repartir(sorties) else { return Vec::new() };
    let me = ext.mode_base();
    // Écran du PC éteint (mode inconnu) : on se cale sur l'autre écran
    let mi_connu = int.mode_base();
    let Some(mi) = mi_connu.or(me) else { return Vec::new() };
    let o = |s: &str| s.to_string();
    // Allumer un écran, avec une résolution précise si on la connaît
    let allumer = |s: &Sortie, m: Option<(u32, u32, f32)>, pos: String, echelle: f32| {
        let mut c = vec![o("--output"), s.nom.clone(), o("--on")];
        if let Some(m) = m {
            c.extend([o("--mode"), mode_txt(m)]);
        }
        c.extend([o("--pos"), pos, o("--scale"), txt_echelle(echelle)]);
        c
    };
    match mode {
        ModeEcran::Dupliquer => {
            // Même image : même position, même taille « logique ».
            // Si l'écran externe connaît la résolution du PC, on la prend ; sinon on met à l'échelle.
            // Largeur « logique » de l'écran du PC (après agrandissement)
            let largeur = mi.0 as f32 / si;
            let (mode_e, echelle) = match ext.modes.iter().filter(|m| m.0 == mi.0 && m.1 == mi.1).max_by(|a, b| a.2.total_cmp(&b.2)) {
                Some(m) => (Some(*m), si),
                None => match me {
                    Some(me) => (Some(me), me.0 as f32 / largeur),
                    None => (None, si),
                },
            };
            vec![allumer(&int, mi_connu, o("0,0"), si), allumer(&ext, mode_e, o("0,0"), echelle)]
        }
        ModeEcran::Etendre => vec![
            allumer(&int, mi_connu, o("0,0"), si),
            allumer(&ext, me, format!("{},0", (mi.0 as f32 / si).round() as u32), se),
        ],
        ModeEcran::ExterneSeul => vec![allumer(&ext, me, o("0,0"), se), vec![o("--output"), int.nom.clone(), o("--off")]],
        ModeEcran::InterneSeul => vec![
            vec![o("--output"), int.nom.clone(), o("--on"), o("--pos"), o("0,0"), o("--scale"), txt_echelle(si)],
            vec![o("--output"), ext.nom.clone(), o("--off")],
        ],
    }
}

pub fn appliquer_ecran(mode: ModeEcran) -> Result<(), String> {
    let sorties = lire_ecrans();
    let cmds = commandes_ecran(&sorties, mode);
    if cmds.is_empty() {
        return Err(tr("Aucun deuxième écran branché."));
    }
    // Allumer d'abord, éteindre ensuite : on ne se retrouve jamais sans écran.
    for c in cmds.iter() {
        let args: Vec<&str> = c.iter().map(|s| s.as_str()).collect();
        sortie("wlr-randr", &args)?;
    }
    Ok(())
}

fn fichier_mode() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config/soleos/ecran"))
}

pub fn mode_memorise() -> ModeEcran {
    fichier_mode()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| ModeEcran::depuis(&t))
        .unwrap_or(ModeEcran::Dupliquer)
}

fn memoriser(mode: ModeEcran) {
    if let Some(f) = fichier_mode() {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, mode.code());
    }
}

/// Appelé quand un écran est branché ou débranché : applique le choix mémorisé.
pub fn ecran_change() {
    let sorties = lire_ecrans();
    if sorties.len() >= 2 {
        let mode = mode_memorise();
        en_fond(move || appliquer_ecran(mode), |_| {});
    } else if let Some(seul) = sorties.first() {
        // Écran externe débranché alors que l'écran du PC était éteint : on le rallume.
        if !seul.actif {
            let nom = seul.nom.clone();
            en_fond(move || sortie("wlr-randr", &["--output", &nom, "--on"]), |_| {});
        }
        en_fond(appliquer_taille, |_| {});
    }
}

// =====================================================================
//  Panneau
// =====================================================================

pub struct Panneau {
    pub racine: gtk::Box,
    pile: gtk::Stack,
    // Imprimantes
    info_impr: gtk::Label,
    liste_impr: gtk::Box,
    etat_impr: RefCell<EtatImpression>,
    // Bluetooth
    info_bt: gtk::Label,
    liste_bt: gtk::Box,
    inter_bt: gtk::Switch,
    maj_bt: std::cell::Cell<bool>,
    // Écran
    info_ecran: gtk::Label,
    choix_ecran: gtk::Box,
    /// Ajout d'une imprimante à la main (adresse, protocole, pilote)
    manuel: Rc<crate::imprimante_manuelle::AjoutManuel>,
}

fn titre_section(texte: &str) -> gtk::Label {
    let l = etiquette(texte, "titre-menu");
    l.set_margin_top(6);
    l
}

impl Panneau {
    /// Panneau d'un seul type d'appareil : « imprimantes », « bluetooth » ou « ecran »
    pub fn nouveau(seule: &str) -> Rc<Self> {
        let racine = gtk::Box::new(gtk::Orientation::Vertical, 8);
        racine.set_size_request(460, -1);

        let pile = gtk::Stack::new();
        pile.set_transition_type(gtk::StackTransitionType::Crossfade);
        pile.set_vhomogeneous(false);
        pile.set_interpolate_size(true);
        let onglets = gtk::StackSwitcher::new();
        onglets.set_stack(Some(&pile));
        onglets.add_css_class("onglets");
        onglets.set_halign(gtk::Align::Center);
        onglets.set_visible(false);
        racine.append(&onglets);
        racine.append(&pile);

        // --- Imprimantes ---
        let page_impr = gtk::Box::new(gtk::Orientation::Vertical, 6);
        page_impr.append(&etiquette(&tr("Imprimantes et scanner"), "nom-appareil"));
        let info_impr = etiquette("", "texte-info");
        let liste_impr = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let defil = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(360)
            .child(&liste_impr)
            .build();
        let bas = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let actualiser = gtk::Button::with_label(&tr("↻ Rechercher"));
        actualiser.add_css_class("menu");
        let numeriser = gtk::Button::with_label(&tr("Numériser un document"));
        numeriser.add_css_class("menu");
        numeriser.set_hexpand(true);
        numeriser.set_halign(gtk::Align::End);
        bas.append(&actualiser);
        bas.append(&numeriser);
        page_impr.append(&info_impr);
        page_impr.append(&defil);
        page_impr.append(&bas);
        let manuel = crate::imprimante_manuelle::AjoutManuel::nouveau();
        page_impr.append(&manuel.racine);
        if seule == "imprimantes" {
            pile.add_titled(&page_impr, Some("imprimantes"), &tr("Imprimantes"));
        }

        // --- Bluetooth ---
        let page_bt = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let entete_bt = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let t = etiquette(&tr("Bluetooth"), "nom-appareil");
        t.set_hexpand(true);
        let inter_bt = gtk::Switch::new();
        inter_bt.set_valign(gtk::Align::Center);
        entete_bt.append(&t);
        entete_bt.append(&inter_bt);
        let info_bt = etiquette("", "texte-info");
        let liste_bt = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let defil_bt = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(360)
            .child(&liste_bt)
            .build();
        let chercher_bt = gtk::Button::with_label(&tr("↻ Rechercher des appareils"));
        chercher_bt.add_css_class("menu");
        page_bt.append(&entete_bt);
        page_bt.append(&info_bt);
        page_bt.append(&defil_bt);
        page_bt.append(&chercher_bt);
        if seule == "bluetooth" {
            pile.add_titled(&page_bt, Some("bluetooth"), &tr("Bluetooth"));
        }

        // --- Écran ---
        let page_ecran = gtk::Box::new(gtk::Orientation::Vertical, 6);
        page_ecran.append(&etiquette(&tr("Deuxième écran"), "nom-appareil"));
        let info_ecran = etiquette("", "texte-info");
        let choix_ecran = gtk::Box::new(gtk::Orientation::Vertical, 4);
        page_ecran.append(&info_ecran);
        page_ecran.append(&choix_ecran);
        if seule == "ecran" {
            pile.add_titled(&page_ecran, Some("ecran"), &tr("Écran"));
        }

        let p = Rc::new(Panneau {
            racine,
            pile: pile.clone(),
            info_impr,
            liste_impr,
            etat_impr: RefCell::new(EtatImpression::default()),
            info_bt,
            liste_bt,
            inter_bt: inter_bt.clone(),
            maj_bt: std::cell::Cell::new(false),
            info_ecran,
            choix_ecran,
            manuel: manuel.clone(),
        });
        let moi = Rc::downgrade(&p);
        *manuel.apres.borrow_mut() = Some(Box::new(move |nom| {
            if let Some(p) = moi.upgrade() {
                p.action_impr(move || Ok(trv("« {nom} » est prête.", &[("nom", &nom.replace('_', " "))])));
            }
        }));

        let moi = Rc::downgrade(&p);
        actualiser.connect_clicked(move |_| {
            if let Some(p) = moi.upgrade() {
                p.actualiser_imprimantes(true);
            }
        });
        // NAPS2 (complet), sinon Simple Scan
        numeriser.connect_clicked(|_| {
            if ["/usr/bin/naps2", "/usr/lib/naps2/naps2"].iter().any(|c| std::path::Path::new(c).exists()) {
                crate::lancer(&["naps2"]);
            } else {
                crate::lancer(&["simple-scan"]);
            }
        });
        let moi = Rc::downgrade(&p);
        chercher_bt.connect_clicked(move |_| {
            if let Some(p) = moi.upgrade() {
                p.actualiser_bluetooth(true);
            }
        });
        let moi = Rc::downgrade(&p);
        inter_bt.connect_active_notify(move |s| {
            let Some(p) = moi.upgrade() else { return };
            if p.maj_bt.get() {
                return;
            }
            let oui = s.is_active();
            p.info_bt.set_label(&if oui { tr("Activation…") } else { tr("Désactivation…") });
            let moi = Rc::downgrade(&p);
            en_fond(
                move || {
                    let _ = bt_allumer(oui);
                    std::thread::sleep(std::time::Duration::from_millis(800));
                },
                move |_| {
                    if let Some(p) = moi.upgrade() {
                        p.actualiser_bluetooth(oui);
                    }
                },
            );
        });
        let moi = Rc::downgrade(&p);
        pile.connect_visible_child_name_notify(move |_| {
            if let Some(p) = moi.upgrade() {
                p.actualiser_page();
            }
        });
        p
    }

    /// À l'ouverture de la bulle : on rafraîchit la page visible.
    pub fn actualiser(self: &Rc<Self>) {
        self.actualiser_page();
    }

    fn actualiser_page(self: &Rc<Self>) {
        match self.pile.visible_child_name().as_deref() {
            Some("bluetooth") => self.actualiser_bluetooth(true),
            Some("ecran") => self.afficher_ecran(),
            _ => self.actualiser_imprimantes(true),
        }
    }

    // ----- Imprimantes -----
    fn actualiser_imprimantes(self: &Rc<Self>, chercher: bool) {
        // D'abord les imprimantes déjà ajoutées (immédiat), puis la recherche de nouvelles (quelques secondes)
        let moi = Rc::downgrade(self);
        en_fond(|| lire_imprimantes(false), move |etat| {
            let Some(p) = moi.upgrade() else { return };
            let service = etat.service;
            p.afficher_imprimantes(etat);
            if chercher && service {
                p.info_impr.set_visible(true);
                p.info_impr.set_label(&tr("Recherche d'autres imprimantes à proximité…"));
                let moi = Rc::downgrade(&p);
                en_fond(|| lire_imprimantes(true), move |etat| {
                    if let Some(p) = moi.upgrade() {
                        p.afficher_imprimantes(etat);
                    }
                });
            }
        });
    }

    fn afficher_imprimantes(self: &Rc<Self>, etat: EtatImpression) {
        vider(&self.liste_impr);
        let message = if !etat.outil {
            tr("L'impression n'est pas installée sur ce système.")
        } else if !etat.service {
            tr("Le service d'impression ne répond pas.")
        } else if etat.installees.is_empty() && etat.trouvees.is_empty() {
            tr("Aucune imprimante trouvée. Allumez-la et branchez-la (USB) ou reliez-la au même réseau, puis « Rechercher ».")
        } else {
            String::new()
        };
        self.info_impr.set_label(&message);
        self.info_impr.set_visible(!message.is_empty());

        if !etat.installees.is_empty() {
            self.liste_impr.append(&titre_section("Mes imprimantes"));
        }
        for i in &etat.installees {
            let detail = if i.par_defaut { format!("{} · {}", i.etat, tr("par défaut")) } else { i.etat.clone() };
            let test = petit_bouton("Test", "");
            test.set_tooltip_text(Some(&tr("Imprimer une page de test")));
            let mut boutons = vec![test.clone()];
            if !i.par_defaut {
                let defaut = petit_bouton("Par défaut", "");
                let nom = i.nom.clone();
                let moi = Rc::downgrade(self);
                defaut.connect_clicked(move |_| {
                    let nom = nom.clone();
                    if let Some(p) = moi.upgrade() {
                        p.action_impr(move || imprimante_par_defaut(&nom).map(|_| String::new()));
                    }
                });
                boutons.push(defaut);
            }
            let retirer = petit_bouton("✕", "danger");
            retirer.set_tooltip_text(Some(&tr("Retirer cette imprimante")));
            boutons.push(retirer.clone());
            let nom = i.nom.clone();
            let moi = Rc::downgrade(self);
            test.connect_clicked(move |_| {
                let nom = nom.clone();
                if let Some(p) = moi.upgrade() {
                    p.action_impr(move || page_de_test(&nom).map(|_| tr("Page de test envoyée.")));
                }
            });
            let nom = i.nom.clone();
            let moi = Rc::downgrade(self);
            retirer.connect_clicked(move |_| {
                let nom = nom.clone();
                if let Some(p) = moi.upgrade() {
                    p.action_impr(move || supprimer_imprimante(&nom).map(|_| String::new()));
                }
            });
            self.liste_impr.append(&ligne("appareils", &i.nom.replace('_', " "), &detail, i.par_defaut, &boutons));
        }

        if !etat.trouvees.is_empty() {
            self.liste_impr.append(&titre_section("Trouvées à proximité"));
        }
        for t in &etat.trouvees {
            let ajouter = petit_bouton("Ajouter", "principal");
            let detail = if t.uri.starts_with("usb://") { n_("Branchée en USB") } else { n_("Sur le réseau") };
            let t2 = t.clone();
            let moi = Rc::downgrade(self);
            ajouter.connect_clicked(move |b| {
                b.set_sensitive(false);
                b.set_label(&tr("…"));
                let Some(p) = moi.upgrade() else { return };
                let t = t2.clone();
                let deja = p.etat_impr.borrow().installees.clone();
                p.info_impr.set_visible(true);
                p.info_impr.set_label(&trv("Ajout de « {nom} »…", &[("nom", &nom_lisible(&t))]));
                // Échec (aucun pilote ne convient) : l'ajout manuel s'ouvre avec son adresse
                let (uri, modele) = (t.uri.clone(), t.modele.clone());
                let manuel = Rc::downgrade(&p.manuel);
                p.action_impr(move || ajouter_imprimante(&t, &deja).map(|n| trv("« {nom} » est prête.", &[("nom", &n.replace('_', " "))])));
                let pause = gtk::glib::timeout_add_local(std::time::Duration::from_millis(500), {
                    let moi = Rc::downgrade(&p);
                    let mut tours = 0;
                    move || {
                        tours += 1;
                        let Some(p) = moi.upgrade() else { return gtk::glib::ControlFlow::Break };
                        if tours > 240 {
                            return gtk::glib::ControlFlow::Break;
                        }
                        let texte = p.info_impr.label().to_string();
                        if texte.starts_with(&tr("Échec : {e}").replace("{e}", "")) {
                            if let Some(m) = manuel.upgrade() {
                                m.preparer(&uri, &modele);
                            }
                            return gtk::glib::ControlFlow::Break;
                        }
                        if !texte.ends_with('…') {
                            return gtk::glib::ControlFlow::Break;
                        }
                        gtk::glib::ControlFlow::Continue
                    }
                });
                let _ = pause;
            });
            self.liste_impr.append(&ligne("appareils", &nom_lisible(t), detail, false, &[ajouter]));
        }
        *self.etat_impr.borrow_mut() = etat;
    }

    /// Lance une action d'impression en fond, puis rafraîchit en affichant le résultat.
    fn action_impr(self: &Rc<Self>, travail: impl FnOnce() -> Result<String, String> + Send + 'static) {
        let moi = Rc::downgrade(self);
        en_fond(
            move || {
                let r = travail();
                (r, lire_imprimantes(false))
            },
            move |(r, etat)| {
                let Some(p) = moi.upgrade() else { return };
                let mut etat = etat;
                // On garde la liste des imprimantes trouvées (sans relancer une recherche)
                let anciennes = p.etat_impr.borrow().trouvees.clone();
                etat.trouvees = anciennes.into_iter().filter(|t| !etat.installees.iter().any(|i| i.uri == t.uri)).collect();
                p.afficher_imprimantes(etat);
                let message = match r {
                    Ok(m) => m,
                    Err(e) => trv("Échec : {e}", &[("e", &if e.is_empty() { tr("erreur inconnue") } else { e })]),
                };
                if !message.is_empty() {
                    p.info_impr.set_visible(true);
                    p.info_impr.set_label(&message);
                }
            },
        );
    }

    // ----- Bluetooth -----
    fn actualiser_bluetooth(self: &Rc<Self>, chercher: bool) {
        self.info_bt.set_visible(true);
        self.info_bt.set_label(&tr("Lecture…"));
        let moi = Rc::downgrade(self);
        // D'abord les appareils connus (immédiat), puis la recherche des appareils à proximité
        en_fond(|| lire_bluetooth(false), move |etat| {
            let Some(p) = moi.upgrade() else { return };
            let allume = etat.allume;
            p.afficher_bluetooth(etat);
            if chercher && allume {
                p.info_bt.set_visible(true);
                p.info_bt.set_label(&tr("Recherche des appareils à proximité…"));
                let moi = Rc::downgrade(&p);
                en_fond(|| lire_bluetooth(true), move |etat| {
                    if let Some(p) = moi.upgrade() {
                        p.afficher_bluetooth(etat);
                    }
                });
            }
        });
    }

    fn afficher_bluetooth(self: &Rc<Self>, etat: EtatBluetooth) {
        vider(&self.liste_bt);
        self.maj_bt.set(true);
        self.inter_bt.set_active(etat.allume);
        self.inter_bt.set_sensitive(etat.adaptateur);
        self.maj_bt.set(false);
        let message = if !etat.outil {
            tr("Le Bluetooth n'est pas installé sur ce système.")
        } else if !etat.adaptateur {
            tr("Pas de Bluetooth sur cet ordinateur.")
        } else if !etat.allume {
            tr("Le Bluetooth est désactivé.")
        } else if etat.appareils.is_empty() {
            tr("Aucun appareil. Mettez votre casque, souris… en mode « association », puis « Rechercher ».")
        } else {
            String::new()
        };
        self.info_bt.set_label(&message);
        self.info_bt.set_visible(!message.is_empty());

        let mut section = None;
        for a in etat.appareils {
            let s = if a.associe { n_("Mes appareils") } else { n_("À proximité") };
            if section != Some(s) {
                self.liste_bt.append(&titre_section(s));
                section = Some(s);
            }
            let type_ = type_bt(&a.icone);
            let detail = match (a.associe, a.connecte) {
                (true, true) => format!("{}{}{}", tr(type_), if type_.is_empty() { "" } else { " · " }, tr("Connecté")),
                (true, false) => format!("{}{}{}", tr(type_), if type_.is_empty() { "" } else { " · " }, tr("Non connecté")),
                _ => tr(type_),
            };
            let mut boutons = Vec::new();
            let adresse = a.adresse.clone();
            if a.associe {
                let b = petit_bouton(if a.connecte { n_("Déconnecter") } else { n_("Connecter") }, "");
                let oui = !a.connecte;
                let (ad, moi) = (adresse.clone(), Rc::downgrade(self));
                b.connect_clicked(move |b| {
                    b.set_sensitive(false);
                    let ad = ad.clone();
                    if let Some(p) = moi.upgrade() {
                        p.action_bt(move || bt_connecter(&ad, oui).map(|_| ()).map_err(|_| tr("Connexion impossible : l'appareil est-il allumé et à proximité ?")));
                    }
                });
                boutons.push(b);
                let oublier = petit_bouton("✕", "danger");
                oublier.set_tooltip_text(Some(&tr("Oublier cet appareil")));
                let (ad, moi) = (adresse.clone(), Rc::downgrade(self));
                oublier.connect_clicked(move |_| {
                    let ad = ad.clone();
                    if let Some(p) = moi.upgrade() {
                        p.action_bt(move || bt_oublier(&ad).map(|_| ()));
                    }
                });
                boutons.push(oublier);
            } else {
                let b = petit_bouton("Associer", "principal");
                let (ad, moi, nom) = (adresse.clone(), Rc::downgrade(self), a.nom.clone());
                b.connect_clicked(move |b| {
                    b.set_sensitive(false);
                    b.set_label(&tr("…"));
                    let ad = ad.clone();
                    if let Some(p) = moi.upgrade() {
                        p.info_bt.set_visible(true);
                        p.info_bt.set_label(&trv("Association avec « {nom} »…", &[("nom", &nom)]));
                        p.action_bt(move || bt_associer(&ad));
                    }
                });
                boutons.push(b);
            }
            self.liste_bt.append(&ligne(icone_bt(&a.icone), &a.nom, &detail, a.connecte, &boutons));
        }
    }

    fn action_bt(self: &Rc<Self>, travail: impl FnOnce() -> Result<(), String> + Send + 'static) {
        let moi = Rc::downgrade(self);
        en_fond(
            move || (travail(), lire_bluetooth(false)),
            move |(r, etat)| {
                let Some(p) = moi.upgrade() else { return };
                p.afficher_bluetooth(etat);
                if let Err(e) = r {
                    p.info_bt.set_visible(true);
                    p.info_bt.set_label(&e);
                }
            },
        );
    }

    // ----- Écran -----
    fn afficher_ecran(self: &Rc<Self>) {
        vider(&self.choix_ecran);
        let sorties = lire_ecrans();
        if sorties.len() < 2 {
            self.info_ecran.set_label(&tr("Un seul écran. Branchez un écran ou un vidéoprojecteur : l'image y sera dupliquée automatiquement."));
            return;
        }
        let ext = repartir(&sorties).map(|(_, e)| e);
        let nom_ext = ext
            .as_ref()
            .map(|e| if e.description.is_empty() { e.nom.clone() } else { e.description.clone() })
            .unwrap_or_default();
        self.info_ecran.set_label(&trv("Deuxième écran : {nom}", &[("nom", &nom_ext)]));
        let actuel = mode_memorise();
        for (mode, titre, detail) in [
            (ModeEcran::Dupliquer, n_("Dupliquer"), n_("La même image sur les deux écrans (présentations)")),
            (ModeEcran::Etendre, n_("Étendre"), n_("Le deuxième écran prolonge celui-ci, à droite")),
            (ModeEcran::ExterneSeul, n_("Deuxième écran seulement"), n_("L'écran du PC s'éteint")),
            (ModeEcran::InterneSeul, n_("Cet écran seulement"), n_("Le deuxième écran reste éteint")),
        ] {
            let b = petit_bouton(if mode == actuel { "✓" } else { n_("Choisir") }, if mode == actuel { "" } else { "principal" });
            b.set_sensitive(mode != actuel);
            let moi = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                memoriser(mode);
                let moi = moi.clone();
                en_fond(move || appliquer_ecran(mode), move |r| {
                    if let Some(p) = moi.upgrade() {
                        p.afficher_ecran();
                        if let Err(e) = r {
                            p.info_ecran.set_label(&e);
                        }
                    }
                });
            });
            self.choix_ecran.append(&ligne("ecran", titre, detail, mode == actuel, &[b]));
        }
    }
}

/// « soleos-accueil --appareils [ajouter-imprimante N | associer ADRESSE | ecran MODE] »
/// État des appareils en texte (assistance) et mêmes actions que le panneau.
pub fn diagnostic(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("ajouter-imprimante") => {
            let n: usize = args.get(1).and_then(|n| n.parse().ok()).unwrap_or(0);
            let etat = lire_imprimantes(true);
            let Some(t) = etat.trouvees.get(n) else { eprintln!("Aucune imprimante n°{n}"); return 1 };
            match ajouter_imprimante(t, &etat.installees) {
                Ok(nom) => { println!("Ajoutée : {nom}"); 0 }
                Err(e) => { eprintln!("Échec : {e}"); 1 }
            }
        }
        Some("associer") => match bt_associer(args.get(1).map(String::as_str).unwrap_or("")) {
            Ok(()) => { println!("Associé"); 0 }
            Err(e) => { eprintln!("{e}"); 1 }
        },
        Some("ecran") => {
            let mode = ModeEcran::depuis(args.get(1).map(String::as_str).unwrap_or("")).unwrap_or(ModeEcran::Dupliquer);
            memoriser(mode);
            match appliquer_ecran(mode) {
                Ok(()) => { println!("Écran : {}", mode.code()); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        _ => {
            let i = lire_imprimantes(true);
            println!("Impression : outil={} service={}", i.outil, i.service);
            for p in &i.installees { println!("  installée : {} [{}]{} {}", p.nom, p.etat, if p.par_defaut { " (par défaut)" } else { "" }, p.uri); }
            for (n, t) in i.trouvees.iter().enumerate() { println!("  trouvée n°{n} : {} — {}", nom_lisible(t), t.uri); }
            let b = lire_bluetooth(true);
            println!("Bluetooth : outil={} adaptateur={} allumé={}", b.outil, b.adaptateur, b.allume);
            for a in &b.appareils { println!("  {} {} associé={} connecté={} [{}]", a.adresse, a.nom, a.associe, a.connecte, a.icone); }
            let e = lire_ecrans();
            println!("Écrans : {} (mode mémorisé : {})", e.len(), mode_memorise().code());
            for s in &e { println!("  {} « {} » actif={} échelle={} mode={:?}", s.nom, s.description, s.actif, s.echelle, s.courant); }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LPINFO: &str = "Device: uri = usb://HP/LaserJet%20M110w?serial=ABC
        class = direct
        info = HP LaserJet M110w
        make-and-model = HP LaserJet M110w
        device-id = MFG:HP;MDL:LaserJet M110w;CMD:PCLM,PWGRaster;
        location =
Device: uri = dnssd://HP%20LaserJet%20M110w%20(USB)._ipp._tcp.local/?uuid=1234
        class = network
        info = HP LaserJet M110w (USB)
        make-and-model = HP LaserJet M110w
        device-id = MFG:HP;MDL:LaserJet M110w;
        location =
Device: uri = dnssd://Brother%20MFC-L2710DW._ipp._tcp.local/?uuid=5678
        class = network
        info = Brother MFC-L2710DW @ bureau
        make-and-model = Brother MFC-L2710DW
        device-id =
        location =
Device: uri = dnssd://Brother%20FAX._fax-ipp._tcp.local/
        class = network
        info = Brother MFC-L2710DW Fax
        make-and-model = Unknown
        device-id =
        location =
Device: uri = ipps://Brother%20MFC-L2710DW._ipps._tcp.local/
        class = network
        info = Brother MFC-L2710DW (driverless)
        make-and-model = Brother MFC-L2710DW
        device-id =
        location =
Device: uri = dnssd://bbox%20Ippos%20Printer._ipp._tcp.local/
        class = network
        info = bbox Ippos Printer
        make-and-model = Unknown
        device-id =
        location =
Device: uri = usb://Canon/MG2500?serial=X
        class = direct
        info = Canon MG2500 series
        make-and-model = Canon MG2500 series
        device-id = MFG:Canon;MDL:MG2500 series;
        location =
";

    #[test]
    fn lpinfo_dedoublonne_et_filtre() {
        let l = analyser_lpinfo(LPINFO);
        let noms: Vec<String> = l.iter().map(nom_lisible).collect();
        assert_eq!(noms, vec!["HP LaserJet M110w (USB)", "Brother MFC-L2710DW", "Canon MG2500 series"]);
        assert!(l[0].uri.starts_with("dnssd://"));
        assert!(l[2].uri.starts_with("usb://"));
    }

    #[test]
    fn noms_de_file() {
        assert_eq!(nom_file("HP LaserJet M110w (USB)"), "HP_LaserJet_M110w_USB");
        assert_eq!(nom_file("Épson ÉcoTank"), "Epson_EcoTank");
        assert_eq!(nom_file("@@@"), "Imprimante");
    }

    #[test]
    fn lpstat() {
        let v = analyser_lpstat_v("device for Bureau: ipp://10.0.0.5/ipp/print\ndevice for HP_M110w: implicitclass://HP/\n");
        assert_eq!(v[0], ("Bureau".to_string(), "ipp://10.0.0.5/ipp/print".to_string()));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn bluetooth_devices() {
        let d = analyser_devices("Device 00:1A:7D:DA:71:13 WH-1000XM4\nDevice 4C:87:5D:11:22:33 4C-87-5D-11-22-33\n");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].1, "WH-1000XM4");
    }

    const RANDR: &str = "eDP-1 \"BOE 0x0A1C (eDP-1)\"
  Enabled: yes
  Modes:
    1920x1080 px, 60.020000 Hz (preferred, current)
    1680x1050 px, 60.020000 Hz
  Position: 0,0
  Transform: normal
  Scale: 1.000000
HDMI-A-1 \"Epson Projector\"
  Enabled: no
  Modes:
    1280x800 px, 60.000000 Hz (preferred)
    1024x768 px, 60.000000 Hz
  Position: 0,0
  Scale: 1.000000
";

    #[test]
    fn wlr_randr_et_modes() {
        let s = analyser_wlr_randr(RANDR);
        assert_eq!(s.len(), 2);
        assert!(s[0].interne() && s[0].actif && !s[1].actif);
        assert_eq!(s[1].prefere, Some((1280, 800, 60.0)));
        // Dupliquer sans résolution commune : mise à l'échelle 1280/1920
        let c = commandes_ecran_echelles(&s, ModeEcran::Dupliquer, 1.0, 1.0);
        assert_eq!(c[1].join(" "), "--output HDMI-A-1 --on --mode 1280x800@60.000Hz --pos 0,0 --scale 0.6667");
        // Écran du PC agrandi à 125 % : le projecteur montre la même image (même largeur logique 1536)
        let c = commandes_ecran_echelles(&s, ModeEcran::Dupliquer, 1.25, 1.0);
        assert_eq!(c[0].join(" "), "--output eDP-1 --on --mode 1920x1080@60.020Hz --pos 0,0 --scale 1.2500");
        assert_eq!(c[1].join(" "), "--output HDMI-A-1 --on --mode 1280x800@60.000Hz --pos 0,0 --scale 0.8333");
        let c = commandes_ecran_echelles(&s, ModeEcran::Etendre, 1.25, 1.0);
        assert_eq!(c[1].join(" "), "--output HDMI-A-1 --on --mode 1280x800@60.000Hz --pos 1536,0 --scale 1");
        let c = commandes_ecran_echelles(&s, ModeEcran::Etendre, 1.0, 1.0);
        assert_eq!(c[1].join(" "), "--output HDMI-A-1 --on --mode 1280x800@60.000Hz --pos 1920,0 --scale 1");
        let h = analyser_wlr_randr("HEADLESS-1 \"Headless output 1\"\n  Enabled: yes\n  Modes:\n    1280x720 px (current)\n  Position: 1280,0\n");
        assert_eq!(h[0].courant, Some((1280, 720, 0.0)));
        assert_eq!(mode_txt(h[0].courant.unwrap()), "1280x720");
        let c = commandes_ecran_echelles(&s, ModeEcran::ExterneSeul, 1.0, 1.0);
        assert_eq!(c[1].join(" "), "--output eDP-1 --off");
        assert!(commandes_ecran_echelles(&s[..1], ModeEcran::Dupliquer, 1.0, 1.0).is_empty());
        // Finesse de l'écran : 13,3 pouces en 1920 → 150 % ; 15,6 pouces → 125 % ; 24 pouces → 100 %
        let mut dell = s[0].clone();
        dell.mm = (294, 165);
        assert_eq!(echelle_auto(&dell), 1.25);
        dell.mm = (309, 174);
        assert_eq!(echelle_auto(&dell), 1.25);
        dell.mm = (344, 194);
        assert_eq!(echelle_auto(&dell), 1.0);
        let mut k = dell.clone();
        k.prefere = Some((2560, 1600, 60.0));
        k.mm = (286, 179);
        assert_eq!(echelle_auto(&k), 1.5);
        k.prefere = Some((3840, 2160, 60.0));
        k.mm = (344, 194);
        assert_eq!(echelle_auto(&k), 2.0);
        k.prefere = Some((1366, 768, 60.0));
        k.mm = (160, 90);
        assert_eq!(echelle_auto(&k), 1.0);
        dell.mm = (531, 299);
        assert_eq!(echelle_auto(&dell), 1.0);
    }
}
