//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! SoleOS – clavier et souris Bluetooth dès le démarrage.
//!
//!   soleos-bluetooth etat              CLAVIER=0|1 et SOURIS=0|1 (périphériques présents)
//!   soleos-bluetooth associer [s]      service : associe tout seul les claviers et souris
//!                                       Bluetooth mis en mode association, tant qu'il manque
//!                                       un clavier ou une souris (s secondes au plus, 900 par défaut)
//!   soleos-bluetooth attendre [s]      attend un clavier et une souris (s secondes au plus)
//!
//! Sans clavier ni souris, on ne peut pas associer un appareil Bluetooth depuis les Réglages :
//! ce service le fait à leur place, avant même la demande du code. Les messages (code
//! d'association à taper…) s'affichent sur l'écran de démarrage (Plymouth).

#[path = "../tr.rs"]
mod tr;
use tr::{tr, trv};

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

const BLUEZ: &str = "org.bluez";
const CHEMIN_AGENT: &str = "/fr/soleos/agent";
const FICHIER_MESSAGE: &str = "/run/soleos/bluetooth-message";

// =====================================================================
//  Périphériques présents (/proc/bus/input/devices)
// =====================================================================

/// (clavier, souris) : un vrai clavier (lettres et chiffres), un pointeur (souris, pavé tactile, écran tactile)
pub fn peripheriques() -> (bool, bool) {
    let texte = std::fs::read_to_string(std::env::var("SOLEOS_INPUT_DEVICES").unwrap_or("/proc/bus/input/devices".into()))
        .unwrap_or_default();
    analyser(&texte)
}

fn analyser(texte: &str) -> (bool, bool) {
    let (mut clavier, mut souris) = (false, false);
    for bloc in texte.split("\n\n") {
        let gest = bloc.lines().find_map(|l| l.strip_prefix("H: Handlers=")).unwrap_or("");
        let touches = bloc.lines().find_map(|l| l.strip_prefix("B: KEY=")).unwrap_or("");
        let gest: Vec<&str> = gest.split_whitespace().collect();
        if gest.contains(&"kbd") && a_touches(touches, &[16, 30, 44, 28]) {
            // Q, A, Z, Entrée : écarte le bouton d'alimentation, les touches multimédia…
            clavier = true;
        }
        if gest.iter().any(|g| g.starts_with("mouse")) {
            souris = true;
        }
    }
    (clavier, souris)
}

/// Bits du masque « KEY= » (mots hexadécimaux de 64 bits, du plus haut au plus bas)
fn a_touches(masque: &str, codes: &[usize]) -> bool {
    let mots: Vec<u64> = masque.split_whitespace().rev().map(|m| u64::from_str_radix(m, 16).unwrap_or(0)).collect();
    codes.iter().all(|&c| mots.get(c / 64).map(|m| m & (1u64 << (c % 64)) != 0).unwrap_or(false))
}

// =====================================================================
//  Messages à l'écran
// =====================================================================

fn dire(texte: &str) {
    println!("{texte}");
    let _ = std::fs::create_dir_all("/run/soleos");
    let _ = std::fs::write(FICHIER_MESSAGE, texte);
    if Command::new("plymouth").arg("--ping").status().map(|s| s.success()).unwrap_or(false) {
        let _ = Command::new("plymouth").args(["display-message", &format!("--text={texte}")]).status();
    }
}

// =====================================================================
//  BlueZ par D-Bus
// =====================================================================

type Proprietes = HashMap<String, glib::Variant>;

fn appeler(c: &gio::DBusConnection, chemin: &str, interface: &str, methode: &str, args: Option<&glib::Variant>) -> Result<glib::Variant, glib::Error> {
    c.call_sync(Some(BLUEZ), chemin, interface, methode, args, None, gio::DBusCallFlags::NONE, 10_000, gio::Cancellable::NONE)
}

fn regler(c: &gio::DBusConnection, chemin: &str, interface: &str, nom: &str, valeur: glib::Variant) -> Result<glib::Variant, glib::Error> {
    let args = (interface, nom, glib::Variant::from_variant(&valeur)).to_variant();
    appeler(c, chemin, "org.freedesktop.DBus.Properties", "Set", Some(&args))
}

/// Objets de BlueZ : chemin → interfaces → propriétés
fn objets(c: &gio::DBusConnection) -> Vec<(String, HashMap<String, Proprietes>)> {
    let Ok(r) = appeler(c, "/", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects", None) else { return vec![] };
    let Some(dico) = r.child_value(0).get::<HashMap<glib::variant::ObjectPath, HashMap<String, Proprietes>>>() else {
        return vec![];
    };
    dico.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect()
}

fn texte(p: &Proprietes, cle: &str) -> String {
    p.get(cle).and_then(|v| v.get::<String>()).unwrap_or_default()
}
fn booleen(p: &Proprietes, cle: &str) -> bool {
    p.get(cle).and_then(|v| v.get::<bool>()).unwrap_or(false)
}

/// Appareil d'entrée : clavier, souris, tablette graphique (icône ou classe « périphérique »)
fn est_entree(p: &Proprietes) -> bool {
    let icone = texte(p, "Icon");
    let classe = p.get("Class").and_then(|v| v.get::<u32>()).unwrap_or(0);
    icone.starts_with("input-keyboard") || icone.starts_with("input-mouse") || icone.starts_with("input-tablet")
        || (classe >> 8) & 0x1f == 0x05
        || p.get("Appearance").and_then(|v| v.get::<u16>()).map(|a| (0x03c0..=0x03c4).contains(&a)).unwrap_or(false)
}

fn nom_appareil(p: &Proprietes) -> String {
    let n = texte(p, "Alias");
    if n.is_empty() { "Bluetooth".into() } else { n }
}

// =====================================================================
//  Agent : affiche les codes d'association, accepte le reste
// =====================================================================

const AGENT_XML: &str = r#"<node><interface name="org.bluez.Agent1">
  <method name="Release"/>
  <method name="RequestPinCode"><arg type="o" direction="in"/><arg type="s" direction="out"/></method>
  <method name="DisplayPinCode"><arg type="o" direction="in"/><arg type="s" direction="in"/></method>
  <method name="RequestPasskey"><arg type="o" direction="in"/><arg type="u" direction="out"/></method>
  <method name="DisplayPasskey"><arg type="o" direction="in"/><arg type="u" direction="in"/><arg type="q" direction="in"/></method>
  <method name="RequestConfirmation"><arg type="o" direction="in"/><arg type="u" direction="in"/></method>
  <method name="RequestAuthorization"><arg type="o" direction="in"/></method>
  <method name="AuthorizeService"><arg type="o" direction="in"/><arg type="s" direction="in"/></method>
  <method name="Cancel"/>
</interface></node>"#;

fn nom_par_chemin(c: &gio::DBusConnection, chemin: &str) -> String {
    let args = ("org.bluez.Device1", "Alias").to_variant();
    appeler(c, chemin, "org.freedesktop.DBus.Properties", "Get", Some(&args))
        .ok()
        .and_then(|v| v.child_value(0).as_variant())
        .and_then(|v| v.get::<String>())
        .unwrap_or_else(|| "Bluetooth".into())
}

fn code_au_hasard() -> u32 {
    let mut o = [0u8; 4];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut o);
    }
    u32::from_le_bytes(o) % 1_000_000
}

fn enregistrer_agent(c: &gio::DBusConnection) -> Result<(), String> {
    let noeud = gio::DBusNodeInfo::for_xml(AGENT_XML).map_err(|e| e.to_string())?;
    let interface = noeud.lookup_interface("org.bluez.Agent1").ok_or("interface")?;
    c.register_object(CHEMIN_AGENT, &interface)
        .method_call(|c, _, _, _, methode, params, invocation| {
            let appareil = || params.try_child_value(0).and_then(|v| v.str().map(str::to_string)).unwrap_or_default();
            match methode {
                "RequestPinCode" => {
                    // Clavier ancien : on choisit le code, la personne le tape sur le clavier
                    let code = format!("{:06}", code_au_hasard());
                    dire(&trv("Tapez {code} sur le clavier « {nom} », puis Entrée.", &[("code", &code), ("nom", &nom_par_chemin(&c, &appareil()))]));
                    invocation.return_value(Some(&(code,).to_variant()));
                }
                "RequestPasskey" => {
                    let code = code_au_hasard();
                    dire(&trv("Tapez {code} sur le clavier « {nom} », puis Entrée.", &[("code", &format!("{code:06}")), ("nom", &nom_par_chemin(&c, &appareil()))]));
                    invocation.return_value(Some(&(code,).to_variant()));
                }
                "DisplayPasskey" => {
                    let code = params.try_child_value(1).and_then(|v| v.get::<u32>()).unwrap_or(0);
                    let tapes = params.try_child_value(2).and_then(|v| v.get::<u16>()).unwrap_or(0);
                    if tapes == 0 {
                        dire(&trv("Tapez {code} sur le clavier « {nom} », puis Entrée.", &[("code", &format!("{code:06}")), ("nom", &nom_par_chemin(&c, &appareil()))]));
                    }
                    invocation.return_value(None);
                }
                "DisplayPinCode" => {
                    let code = params.try_child_value(1).and_then(|v| v.str().map(str::to_string)).unwrap_or_default();
                    dire(&trv("Tapez {code} sur le clavier « {nom} », puis Entrée.", &[("code", &code), ("nom", &nom_par_chemin(&c, &appareil()))]));
                    invocation.return_value(None);
                }
                // Confirmation, autorisations : acceptées (seuls des claviers et souris sont associés)
                _ => invocation.return_value(None),
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let args = (glib::variant::ObjectPath::try_from(CHEMIN_AGENT).unwrap(), "KeyboardDisplay").to_variant();
    appeler(c, "/org/bluez", "org.bluez.AgentManager1", "RegisterAgent", Some(&args)).map_err(|e| e.to_string())?;
    let args = (glib::variant::ObjectPath::try_from(CHEMIN_AGENT).unwrap(),).to_variant();
    let _ = appeler(c, "/org/bluez", "org.bluez.AgentManager1", "RequestDefaultAgent", Some(&args));
    Ok(())
}

// =====================================================================
//  Association automatique
// =====================================================================

struct Etat {
    c: gio::DBusConnection,
    adaptateur: RefCell<Option<String>>,
    agent: Cell<bool>,
    /// Appareils : nombre d'essais et moment du dernier
    essais: RefCell<HashMap<String, (u32, Instant)>>,
    en_cours: RefCell<Option<String>>,
    debut: Instant,
    duree: Duration,
    dernier_message: RefCell<String>,
    /// Message important affiché (code, appareil associé) : pas remplacé avant ce moment
    garder_message: Cell<Option<Instant>>,
}

impl Etat {
    fn message(&self, t: &str) {
        if *self.dernier_message.borrow() != t {
            *self.dernier_message.borrow_mut() = t.to_string();
            dire(t);
        }
    }

    /// Un tour : vérifie les périphériques, prépare l'adaptateur, associe ce qui se présente.
    /// Renvoie faux quand le service peut s'arrêter.
    fn tour(self: &Rc<Self>) -> bool {
        let (clavier, souris) = peripheriques();
        if clavier && souris && self.en_cours.borrow().is_none() {
            self.terminer();
            println!("Clavier et souris présents.");
            return false;
        }
        if self.debut.elapsed() > self.duree {
            self.terminer();
            println!("Délai écoulé.");
            return false;
        }
        let objs = objets(&self.c);
        // Adaptateur Bluetooth : allumé, associable, recherche des appareils
        if self.adaptateur.borrow().is_none() {
            let Some((chemin, _)) = objs.iter().find(|(_, i)| i.contains_key("org.bluez.Adapter1")) else {
                return true; // pas encore d'adaptateur (pilote en cours de chargement), ou aucun
            };
            let chemin = chemin.clone();
            let _ = regler(&self.c, &chemin, "org.bluez.Adapter1", "Powered", true.to_variant());
            let _ = regler(&self.c, &chemin, "org.bluez.Adapter1", "Pairable", true.to_variant());
            if !self.agent.get() {
                match enregistrer_agent(&self.c) {
                    Ok(()) => self.agent.set(true),
                    Err(e) => eprintln!("agent : {e}"),
                }
            }
            let _ = appeler(&self.c, &chemin, "org.bluez.Adapter1", "StartDiscovery", None);
            *self.adaptateur.borrow_mut() = Some(chemin);
        }
        if self.en_cours.borrow().is_some() {
            return true;
        }
        let manque = match (clavier, souris) {
            (false, false) => tr("Il manque un clavier ou une souris : si c'est un appareil Bluetooth, mettez-le en mode association (souvent un appui long sur son bouton)."),
            (false, true) => tr("Il manque un clavier : si c'est un appareil Bluetooth, mettez-le en mode association (souvent un appui long sur son bouton)."),
            _ => tr("Il manque une souris : si c'est un appareil Bluetooth, mettez-la en mode association (souvent un appui long sur son bouton)."),
        };
        let garde = self.garder_message.get().map(|t| Instant::now() < t).unwrap_or(false);
        if !garde { self.message(&manque); }
        for (chemin, ifs) in &objs {
            let Some(p) = ifs.get("org.bluez.Device1") else { continue };
            if !est_entree(p) {
                continue;
            }
            if booleen(p, "Paired") {
                // Déjà associé : on le rapproche si besoin (il se reconnecte aussi tout seul)
                if !booleen(p, "Trusted") {
                    let _ = regler(&self.c, chemin, "org.bluez.Device1", "Trusted", true.to_variant());
                }
                continue;
            }
            // Pas plus de 3 essais par appareil, espacés de 20 s
            let (n, quand) = self.essais.borrow().get(chemin).cloned().unwrap_or((0, self.debut - Duration::from_secs(60)));
            if n >= 3 || quand.elapsed() < Duration::from_secs(20) {
                continue;
            }
            self.essais.borrow_mut().insert(chemin.clone(), (n + 1, Instant::now()));
            self.associer(chemin.clone(), nom_appareil(p));
            break;
        }
        true
    }

    fn associer(self: &Rc<Self>, chemin: String, nom: String) {
        *self.en_cours.borrow_mut() = Some(chemin.clone());
        self.message(&trv("Association de « {nom} »…", &[("nom", &nom)]));
        let moi = self.clone();
        let cible = chemin.clone();
        self.c.call(Some(BLUEZ), &cible, "org.bluez.Device1", "Pair", None, None, gio::DBusCallFlags::NONE, 60_000,
            gio::Cancellable::NONE, move |r| {
                let ok = match r {
                    Ok(_) => true,
                    Err(e) => e.to_string().contains("AlreadyExists"),
                };
                if ok {
                    let _ = regler(&moi.c, &chemin, "org.bluez.Device1", "Trusted", true.to_variant());
                    let _ = appeler(&moi.c, &chemin, "org.bluez.Device1", "Connect", None);
                    moi.message(&trv("« {nom} » est associé.", &[("nom", &nom)]));
                    moi.garder_message.set(Some(Instant::now() + Duration::from_secs(8)));
                } else {
                    eprintln!("association de {nom} : échec");
                    moi.message(&trv("L'association de « {nom} » n'a pas abouti : remettez-le en mode association.", &[("nom", &nom)]));
                    moi.garder_message.set(Some(Instant::now() + Duration::from_secs(8)));
                }
                *moi.en_cours.borrow_mut() = None;
            });
    }

    fn terminer(&self) {
        if let Some(a) = self.adaptateur.borrow().as_ref() {
            let _ = appeler(&self.c, a, "org.bluez.Adapter1", "StopDiscovery", None);
        }
        if self.agent.get() {
            let args = (glib::variant::ObjectPath::try_from(CHEMIN_AGENT).unwrap(),).to_variant();
            let _ = appeler(&self.c, "/org/bluez", "org.bluez.AgentManager1", "UnregisterAgent", Some(&args));
        }
        let _ = std::fs::remove_file(FICHIER_MESSAGE);
    }
}

fn associer(duree: u64) -> i32 {
    let (clavier, souris) = peripheriques();
    if clavier && souris {
        println!("Clavier et souris présents : rien à faire.");
        return 0;
    }
    // Le service Bluetooth (bluetoothd) peut démarrer un peu après nous
    let mut connexion = None;
    for _ in 0..30 {
        if let Ok(c) = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) {
            connexion = Some(c);
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let Some(c) = connexion else {
        eprintln!("Bus système introuvable.");
        return 1;
    };
    let boucle = glib::MainLoop::new(None, false);
    let etat = Rc::new(Etat {
        c,
        adaptateur: RefCell::new(None),
        agent: Cell::new(false),
        essais: RefCell::new(HashMap::new()),
        en_cours: RefCell::new(None),
        debut: Instant::now(),
        duree: Duration::from_secs(duree),
        dernier_message: RefCell::new(String::new()),
        garder_message: Cell::new(None),
    });
    let b = boucle.clone();
    let e = etat.clone();
    if !e.tour() {
        return 0;
    }
    glib::timeout_add_local(Duration::from_secs(2), move || {
        if e.tour() {
            glib::ControlFlow::Continue
        } else {
            b.quit();
            glib::ControlFlow::Break
        }
    });
    boucle.run();
    0
}

/// Attend un clavier et une souris (écran de démarrage), sans bloquer plus de `duree` secondes
fn attendre(duree: u64) -> i32 {
    let debut = Instant::now();
    let mut prevenu = false;
    loop {
        let (clavier, souris) = peripheriques();
        if clavier && souris {
            return 0;
        }
        if debut.elapsed() > Duration::from_secs(duree) {
            return 0;
        }
        if !prevenu && !std::path::Path::new(FICHIER_MESSAGE).exists() {
            dire(&match (clavier, souris) {
                (true, false) => tr("Recherche d'une souris… Souris Bluetooth : mettez-la en mode association."),
                (false, true) => tr("Recherche d'un clavier… Clavier Bluetooth : mettez-le en mode association."),
                _ => tr("Recherche d'un clavier et d'une souris… Bluetooth : mettez-les en mode association."),
            });
            prevenu = true;
        }
        std::thread::sleep(Duration::from_millis(700));
    }
}

fn main() {
    tr::initialiser();
    let args: Vec<String> = std::env::args().collect();
    let duree = |defaut: u64| args.get(2).and_then(|d| d.parse().ok()).unwrap_or(defaut);
    let code = match args.get(1).map(String::as_str) {
        Some("etat") => {
            let (c, s) = peripheriques();
            println!("CLAVIER={}\nSOURIS={}", c as u8, s as u8);
            0
        }
        Some("associer") => associer(duree(900)),
        Some("attendre") => attendre(duree(180)),
        _ => {
            eprintln!("Usage : soleos-bluetooth etat | associer [secondes] | attendre [secondes]");
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXEMPLE: &str = "I: Bus=0019 Vendor=0000 Product=0001 Version=0000
N: Name=\"Power Button\"
H: Handlers=kbd event0
B: EV=3
B: KEY=10000000000000 0

I: Bus=0005 Vendor=046d Product=b35f Version=0001
N: Name=\"MX Keys\"
H: Handlers=sysrq kbd leds event5
B: EV=12001f
B: KEY=3f000301ff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17d000 677bfad9415fbe fe7ffffffffffffe

I: Bus=0005 Vendor=046d Product=b034 Version=0001
N: Name=\"MX Master 3\"
H: Handlers=mouse1 event6
B: EV=17
";
    #[test]
    fn detecte_clavier_et_souris() {
        assert_eq!(analyser(EXEMPLE), (true, true));
        // Bouton d'alimentation seul : pas un clavier
        assert_eq!(analyser(EXEMPLE.split("\n\n").next().unwrap()), (false, false));
    }
}
