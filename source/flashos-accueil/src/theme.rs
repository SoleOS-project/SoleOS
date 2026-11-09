//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Thème « Éclair » commun à toutes les fenêtres SoleOS : couleurs, polices, boutons,
//! et adaptation à la taille de l'écran (classes « compact » et « mini »).

#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{gdk, glib};

pub const FOND: &str = "#0B1020";
pub const SURFACE: &str = "#141A2E";
pub const BORD: &str = "#2A3352";
pub const ACCENT: &str = "#FFC23D";

/// Feuille de style de base (chargée avant celle de chaque fenêtre, qui peut la compléter)
pub const BASE: &str = r#"
@define-color fond #0B1020;
@define-color fond2 #0E1426;
@define-color surface #141A2E;
@define-color surface2 #1A2138;
@define-color bord #2A3352;
@define-color texte #E9ECF5;
@define-color texte2 #A7B0C8;
@define-color discret #8A94B0;
@define-color accent #FFC23D;
@define-color accent_survol #FFD36B;
@define-color accent_texte #0B2A5B;
@define-color danger #E5484D;
@define-color attention #FF9B6B;

window.flashos {
  font-family: "Inter", "Inter Variable", "DejaVu Sans", sans-serif;
  background-color: @fond; background-image: none;
  color: @texte;
}
window.flashos label { color: @texte; }
.flashos .titre, .flashos .diapo-titre, .flashos .titre-eclair {
  font-family: "Space Grotesk", "Inter", sans-serif; font-weight: 700; letter-spacing: -0.5px;
}
.flashos .sous-titre, .flashos .diapo-texte { color: @texte2; }
.flashos .discret { color: @discret; }
.flashos .etapes { color: @texte2; font-size: 14px; letter-spacing: 0; }
.flashos .erreur, .flashos .erreur-code { color: @attention; }
.flashos .marque-eclair { font-family: "Space Grotesk", "Inter", sans-serif; font-weight: 700; font-size: 19px; }
.accueil label.pastille-version, label.pastille-version { font-family: "Space Grotesk", "Inter", sans-serif; font-weight: 500; font-size: 12px; color: #FFC23D;
  border: 1.5px solid alpha(#FFC23D, 0.9); background-color: alpha(#FFC23D, 0.11); border-radius: 999px; padding: 1px 10px; }
label.pastille-version.grande { font-size: 15px; padding: 3px 14px; }
.flashos .segment { min-width: 52px; min-height: 4px; border-radius: 2px; background: #3A4360; }
.flashos .segment.fait { background: alpha(@accent, 0.45); }
.flashos .segment.actif { background: @accent; }
.flashos .barre-actions { background: @fond2; border-top: 1px solid #1E2640; padding: 16px 44px; }
.flashos .carte, .flashos .carte-choix {
  background: @surface; border: 1px solid @bord; border-radius: 18px;
}
.flashos entry, .flashos passwordentry {
  background: @surface; color: @texte; border: 1px solid @bord; box-shadow: none; outline: none;
  border-radius: 14px; caret-color: @accent;
}
.flashos entry:focus-within, .flashos passwordentry:focus-within { border-color: @accent; }
.flashos button.principal, .flashos button.installer {
  background: @accent; background-image: none; color: @accent_texte; border: none; box-shadow: none; outline: none;
  border-radius: 14px; font-weight: 600;
}
.flashos button.principal label, .flashos button.installer label { color: @accent_texte; }
.flashos button.principal:hover, .flashos button.installer:hover { background: @accent_survol; box-shadow: none; }
.flashos button.principal:focus-visible, .flashos button.secondaire:focus-visible { box-shadow: 0 0 0 3px alpha(@accent, 0.45); }
.flashos button.principal.danger, .flashos button.danger { background: @danger; background-image: none; color: white; }
.flashos button.principal.danger label, .flashos button.danger label { color: white; }
.flashos button.orange { background: #FF8A5B; background-image: none; color: @accent_texte; }
.flashos button.orange label { color: @accent_texte; }
.flashos button.secondaire {
  background: @surface; background-image: none; color: @texte; border: 1px solid @bord; box-shadow: none; border-radius: 14px;
}
.flashos button.secondaire:hover { background: @surface2; }
.flashos button:disabled { opacity: 0.4; }
.flashos checkbutton check { min-width: 22px; min-height: 22px; border-radius: 6px; border: 2px solid @texte2; background: transparent; }
.flashos checkbutton check:checked { background: @accent; border-color: @accent; color: @accent_texte; }
.flashos progressbar trough { background: alpha(white, 0.10); border: none; }
.flashos progressbar progress { background: @accent; border: none; }
.flashos scrollbar { background: none; }
.flashos scrollbar slider { background: alpha(white, 0.25); min-width: 6px; border-radius: 3px; }
.flashos .cle-grille {
  background: @surface; border: 1px solid @bord; border-radius: 18px; padding: 20px 26px;
}
.flashos .cle-groupe { font-family: "JetBrains Mono", "DejaVu Sans Mono", monospace; font-size: 26px; font-weight: 700; letter-spacing: 1px; }
.flashos .cle-numero { font-size: 13px; color: #6F7A99; }

/* Petits écrans : tout se resserre */
.compact .titre, .compact .diapo-titre { font-size: 32px; }
.compact .sous-titre, .compact .diapo-texte { font-size: 16px; }
.compact .cle-groupe { font-size: 22px; }
.compact .barre-actions { padding: 12px 28px; }
.mini .titre, .mini .diapo-titre { font-size: 26px; }
.mini .sous-titre, .mini .diapo-texte { font-size: 15px; }
.mini .cle-groupe { font-size: 19px; }
.mini .cle-grille { padding: 12px 16px; }
.mini .barre-actions { padding: 10px 18px; }
"#;

/// Règles qui doivent l'emporter sur les feuilles de chaque fenêtre (texte des boutons jaunes)
pub const PRIORITAIRE: &str = r#"
button.principal label, button.installer label, button.principal, button.installer { color: #0B2A5B; }
button.danger label, button.principal.danger label { color: white; }
"#;

/// Charge le thème de base pour tout l'affichage (une fois par programme)
pub fn charger() {
    thread_local! { static FAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    if FAIT.with(|f| f.replace(true)) {
        return;
    }
    // Le bureau SoleOS (accueil, assistant, installateur) reste sombre, même si les
    // applications sont en clair
    if let Some(r) = gtk::Settings::default() {
        r.set_gtk_application_prefer_dark_theme(true);
    }
    let fp = gtk::CssProvider::new();
    #[allow(deprecated)]
    fp.load_from_data(BASE);
    let fort = gtk::CssProvider::new();
    #[allow(deprecated)]
    fort.load_from_data(PRIORITAIRE);
    if let Some(ecran) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&ecran, &fp, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1);
        gtk::style_context_add_provider_for_display(&ecran, &fort, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 5);
    }
}

/// Classe de densité selon la taille disponible (en pixels logiques)
pub fn densite(largeur: i32, hauteur: i32) -> &'static str {
    if hauteur < 660 || largeur < 900 {
        "mini"
    } else if hauteur < 820 || largeur < 1180 {
        "compact"
    } else {
        "normal"
    }
}

/// Fenêtre SoleOS : thème Éclair, et classes « compact » / « mini » tenues à jour selon sa taille
/// (changement de résolution, de zoom, écran branché…).
pub fn adapter(fenetre: &impl IsA<gtk::Window>) {
    charger();
    let f = fenetre.as_ref().clone();
    f.add_css_class("flashos");
    // Dès le départ, d'après la taille de l'écran (avant même l'affichage de la fenêtre)
    let mut avant = "";
    if let Some(m) = gdk::Display::default().and_then(|d| d.monitors().item(0)).and_downcast::<gdk::Monitor>() {
        let g = m.geometry();
        avant = densite(g.width(), g.height());
        match avant {
            "mini" => {
                f.add_css_class("compact");
                f.add_css_class("mini");
            }
            "compact" => f.add_css_class("compact"),
            _ => {}
        }
    }
    let faible = f.downgrade();
    glib::timeout_add_local(std::time::Duration::from_millis(350), move || {
        let Some(f) = faible.upgrade() else { return glib::ControlFlow::Break };
        let (l, h) = (f.width(), f.height());
        if l > 0 && h > 0 {
            let d = densite(l, h);
            if d != avant {
                for c in ["compact", "mini"] {
                    f.remove_css_class(c);
                }
                // « mini » garde aussi les réglages « compact »
                match d {
                    "mini" => {
                        f.add_css_class("compact");
                        f.add_css_class("mini");
                    }
                    "compact" => f.add_css_class("compact"),
                    _ => {}
                }
                avant = d;
            }
        }
        glib::ControlFlow::Continue
    });
}

/// Contenu qui défile verticalement si l'écran est trop petit (jamais coupé)
pub fn defilant(contenu: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let s = gtk::ScrolledWindow::new();
    s.set_hscrollbar_policy(gtk::PolicyType::Never);
    s.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    s.set_propagate_natural_height(false);
    s.set_vexpand(true);
    s.set_hexpand(true);
    s.set_child(Some(contenu));
    s
}

/// Petit logo « S » + « SoleOS » (en-tête des pages)
pub fn marque() -> gtk::Box {
    marque_taille(20)
}

/// Version de SoleOS (« v12.1 ») dans une pastille jaune, comme à l'écran de démarrage
pub fn pastille_version() -> Option<gtk::Label> {
    let v = std::fs::read_to_string("/etc/flashos-version").ok()?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    let l = gtk::Label::new(Some(v));
    l.add_css_class("pastille-version");
    l.set_valign(gtk::Align::Center);
    l.set_halign(gtk::Align::Center);
    Some(l)
}

/// Logo « S » + « SoleOS » à la taille voulue (hauteur du logo en pixels)
pub fn marque_taille(px: i32) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, (px / 3).max(6));
    // Logo « S » de SoleOS
    let dessin = crate::icones::image("logo-s", px + px / 5);
    dessin.set_valign(gtk::Align::Center);
    b.append(&dessin);
    let t = gtk::Label::new(None);
    if px <= 24 {
        t.set_label("SoleOS");
    } else {
        // Grande marque (installateur, écrans d'accueil) : taille de texte proportionnelle
        t.set_markup(&format!("<span size=\"{}pt\">SoleOS</span>", px * 3 / 5));
    }
    t.add_css_class("marque-eclair");
    b.append(&t);
    b
}

/// Progression en segments (« étape n sur total »)
pub fn segments(n: u32, total: u32, texte: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    b.set_valign(gtk::Align::Center);
    for i in 1..=total {
        let s = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        s.add_css_class("segment");
        if i < n {
            s.add_css_class("fait");
        } else if i == n {
            s.add_css_class("actif");
        }
        s.set_valign(gtk::Align::Center);
        b.append(&s);
    }
    let l = gtk::Label::new(Some(texte));
    l.add_css_class("etapes");
    l.set_margin_start(8);
    b.append(&l);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn densites() {
        assert_eq!(densite(1920, 1080), "normal");
        assert_eq!(densite(1366, 768), "compact");
        assert_eq!(densite(1024, 600), "mini");
        assert_eq!(densite(1280, 800), "compact");
        assert_eq!(densite(800, 1280), "mini");
    }
}

/// Banc de test (FLASHOS_TEST_DEBORDEMENT=1) : après l'affichage, vérifie que la fenêtre occupe
/// tout l'écran, que son contenu y tient (taille minimale) et que les boutons principaux sont
/// visibles en entier. Écrit une ligne « VERIF … » sur la sortie standard.
pub fn verifier_si_demande(fenetre: &impl IsA<gtk::Window>) {
    let Ok(v) = std::env::var("FLASHOS_TEST_DEBORDEMENT") else { return };
    let delai = v.parse::<u64>().ok().filter(|d| *d > 100).unwrap_or(2500);
    let f = fenetre.as_ref().clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(delai), move || {
        let (l, h) = (f.width(), f.height());
        let (ecran_l, ecran_h) = gdk::Display::default()
            .and_then(|d| d.monitors().item(0))
            .and_downcast::<gdk::Monitor>()
            .map(|m| (m.geometry().width(), m.geometry().height()))
            .unwrap_or((0, 0));
        let (min_l, _, _, _) = f.measure(gtk::Orientation::Horizontal, -1);
        let (min_h, _, _, _) = f.measure(gtk::Orientation::Vertical, l);
        let mut problemes = Vec::new();
        // Banc local (labwc 0.7) : barre de titre ajoutée par le gestionnaire de fenêtres
        let barre: i32 = std::env::var("FLASHOS_TEST_BARRE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if (l - ecran_l).abs() > 2 || (h + barre - ecran_h).abs() > 2 {
            problemes.push(format!("fenetre {l}x{h} au lieu de {ecran_l}x{ecran_h}"));
        }
        if min_l > l || min_h > h {
            problemes.push(format!("contenu minimal {min_l}x{min_h} > {l}x{h}"));
        }
        // Boutons principaux : entièrement dans la fenêtre
        let mut pile = vec![f.clone().upcast::<gtk::Widget>()];
        let mut boutons = 0;
        while let Some(w) = pile.pop() {
            let mut enfant = w.first_child();
            while let Some(e) = enfant {
                pile.push(e.clone());
                enfant = e.next_sibling();
            }
            // Dans une zone qui défile, un bouton plus bas reste accessible : pas compté
            let dans_defilement = w.ancestor(gtk::ScrolledWindow::static_type()).is_some();
            if w.is_drawable() && w.has_css_class("principal") && !dans_defilement {
                boutons += 1;
                if let Some(b) = w.compute_bounds(&f) {
                    let (x, y, bl, bh) = (b.x(), b.y(), b.width(), b.height());
                    if x < -1.0 || y < -1.0 || x + bl > l as f32 + 1.0 || y + bh > h as f32 + 1.0 {
                        problemes.push(format!("bouton hors écran à {x:.0},{y:.0} ({bl:.0}x{bh:.0})"));
                    }
                }
            }
        }
        if problemes.is_empty() {
            println!("VERIF ok fenetre={l}x{h} ecran={ecran_l}x{ecran_h} min={min_l}x{min_h} boutons={boutons}");
        } else {
            println!("VERIF KO {} (min={min_l}x{min_h})", problemes.join(" ; "));
        }
    });
}

/// Taille de l'écran qui affiche la fenêtre (ou du premier écran)
pub fn taille_ecran(f: &gtk::Window) -> Option<(i32, i32)> {
    let ecran = gdk::Display::default()?;
    let m = f
        .surface()
        .and_then(|s| ecran.monitor_at_surface(&s))
        .or_else(|| ecran.monitors().item(0).and_downcast::<gdk::Monitor>())?;
    let g = m.geometry();
    Some((g.width(), g.height()))
}

/// Garde la fenêtre en plein écran (`plein = true`) ou agrandie à tout l'écran (`false`) :
/// demandé à l'affichage, puis vérifié toutes les 0,7 s. Certains gestionnaires d'affichage
/// ignorent une demande faite trop tôt, et l'écran peut changer (pilote chargé tard, zoom,
/// écran branché) : la fenêtre est alors redemandée en plein écran, jamais laissée petite ou décalée.
pub fn garder_plein_ecran(fenetre: &impl IsA<gtk::Window>, plein: bool) {
    let f = fenetre.as_ref().clone();
    let demander = move |f: &gtk::Window| {
        if plein {
            f.unfullscreen();
            f.fullscreen();
        } else {
            f.unmaximize();
            f.maximize();
        }
    };
    let d = demander.clone();
    f.connect_map(move |f| {
        let f = f.clone();
        let d = d.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || d(&f));
    });
    if plein {
        f.fullscreen();
    } else {
        f.maximize();
    }
    let faible = f.downgrade();
    let mut essais_rapproches = 0u32;
    let mut attente = 0u32;
    glib::timeout_add_local(std::time::Duration::from_millis(700), move || {
        let Some(f) = faible.upgrade() else { return glib::ControlFlow::Break };
        if !f.is_visible() || f.width() == 0 || std::env::var_os("FLASHOS_TEST_FENETRE").is_some() {
            return glib::ControlFlow::Continue;
        }
        let etat_ok = if plein { f.is_fullscreen() } else { f.is_maximized() || f.is_fullscreen() };
        let taille_ok = match taille_ecran(&f) {
            Some((l, h)) => (f.width() - l).abs() <= 8 && (f.height() - h).abs() <= if plein { 8 } else { 60 },
            None => true,
        };
        if etat_ok && taille_ok {
            essais_rapproches = 0;
            return glib::ControlFlow::Continue;
        }
        // Pas plus d'une nouvelle demande toutes les 0,7 s au début, puis toutes les 3 s
        if attente > 0 {
            attente -= 1;
            return glib::ControlFlow::Continue;
        }
        if std::env::var_os("FLASHOS_DEBUG").is_some() {
            eprintln!("plein écran redemandé ({}x{}, état {etat_ok})", f.width(), f.height());
        }
        demander(&f);
        essais_rapproches += 1;
        if essais_rapproches > 6 {
            attente = 4;
        }
        glib::ControlFlow::Continue
    });
}
