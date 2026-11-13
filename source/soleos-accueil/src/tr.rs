//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Traductions (gettext, domaine « soleos ») : chaque texte affiché passe par `tr("…")`.
//! Le texte d'origine est en français ; les traductions sont dans
//! /usr/share/locale/<langue>/LC_MESSAGES/soleos.mo (fichiers .po du dossier « po »).
//! Sans traduction pour la langue choisie, le texte français est affiché.

#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

unsafe extern "C" {
    fn dgettext(domaine: *const c_char, texte: *const c_char) -> *mut c_char;
    fn bindtextdomain(domaine: *const c_char, dossier: *const c_char) -> *mut c_char;
    fn bind_textdomain_codeset(domaine: *const c_char, codage: *const c_char) -> *mut c_char;
    fn setlocale(categorie: i32, locale: *const c_char) -> *mut c_char;
}

const DOMAINE: &CStr = c"soleos";
const LC_ALL: i32 = 6;

/// Langue choisie pour tout l'ordinateur (écrite au démarrage par « soleos-systeme langue-appliquer »,
/// ou sur la clé par la page de choix de la langue)
pub const FICHIER_LANGUE_SYSTEME: &str = "/run/soleos/langue";

/// À appeler une fois au démarrage de chaque programme
pub fn initialiser() {
    // La langue choisie pour l'ordinateur l'emporte sur celle héritée : services du système
    // (leur variable LANG est fixée avant la lecture du choix), et programmes de la session
    // relancés après un changement de langue
    let fichier = std::env::var_os("SOLEOS_FICHIER_LANGUE").unwrap_or_else(|| FICHIER_LANGUE_SYSTEME.into());
    if let Ok(code) = std::fs::read_to_string(fichier) {
        // Hors session (écran de démarrage) : pas de police chinoise à ce moment-là,
        // les messages sont donc en anglais pour le chinois
        let hors_session = std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("XDG_SESSION_TYPE").is_none();
        let code = if hors_session && code.trim() == "zh" { "en".to_string() } else { code };
        if let Some(l) = langue(code.trim()) {
            // SAFETY : appelé au tout début du programme, avant tout autre fil d'exécution
            unsafe {
                std::env::set_var("LANGUAGE", l.language);
                std::env::set_var("LANG", l.locale);
                std::env::remove_var("LC_ALL");
                std::env::remove_var("LC_MESSAGES");
                // Chinois : écriture pinyin (fcitx5) dans les applications GTK et Qt
                // (marche avec tous les compositeurs ; les autres passent par text-input-v3)
                if l.code == "zh" && !hors_session {
                    std::env::set_var("GTK_IM_MODULE", "fcitx");
                    std::env::set_var("QT_IM_MODULE", "fcitx");
                } else if std::env::var("GTK_IM_MODULE").as_deref() == Ok("fcitx") {
                    std::env::remove_var("GTK_IM_MODULE");
                    std::env::remove_var("QT_IM_MODULE");
                }
            }
        }
    }
    initialiser_locale();
}

/// (Re)lit la langue dans l'environnement (après un changement de LANG / LANGUAGE)
pub fn initialiser_locale() {
    let dossier = std::env::var("SOLEOS_LOCALE").unwrap_or_else(|_| "/usr/share/locale".into());
    let Ok(dossier) = CString::new(dossier) else { return };
    unsafe {
        setlocale(LC_ALL, c"".as_ptr());
        bindtextdomain(DOMAINE.as_ptr(), dossier.as_ptr());
        bind_textdomain_codeset(DOMAINE.as_ptr(), c"UTF-8".as_ptr());
    }
}

/// Texte traduit (ou le texte d'origine s'il n'y a pas de traduction)
pub fn tr(texte: &str) -> String {
    // gettext("") renvoie l'en-tête du fichier de traduction : un texte vide reste vide
    if texte.is_empty() {
        return String::new();
    }
    let Ok(c) = CString::new(texte) else { return texte.to_string() };
    unsafe {
        let r = dgettext(DOMAINE.as_ptr(), c.as_ptr());
        if r.is_null() {
            return texte.to_string();
        }
        CStr::from_ptr(r).to_string_lossy().into_owned()
    }
}

/// Marque un texte à traduire plus tard (listes constantes) : `n_("Éclair")`, puis `tr(nom)` à l'affichage
pub const fn n_(texte: &'static str) -> &'static str {
    texte
}

/// Une langue proposée par SoleOS
pub struct Langue {
    /// « fr », « en », « it », « es », « de », « zh »
    pub code: &'static str,
    /// Nom écrit dans sa propre langue
    pub nom: &'static str,
    /// Locale du système (LANG)
    pub locale: &'static str,
    /// LANGUAGE (ordre de préférence des traductions)
    pub language: &'static str,
    /// Clavier proposé par défaut (disposition, variante)
    pub clavier: (&'static str, &'static str),
    /// Fuseau horaire proposé par défaut
    pub fuseau: &'static str,
    /// Séparateur décimal
    pub decimal: char,
    /// Page d'accueil du navigateur et adresse de recherche (le texte cherché est ajouté à la fin)
    pub web: &'static str,
    pub recherche: &'static str,
    /// « Bonjour » dans cette langue (page de choix)
    pub bonjour: &'static str,
}

pub const LANGUES: &[Langue] = &[
    Langue { code: "fr", nom: "Français", locale: "fr_FR.UTF-8", language: "fr_FR:fr", clavier: ("fr", ""), fuseau: "Europe/Paris",
             decimal: ',', web: "https://www.google.fr/", recherche: "https://www.google.fr/search?q=", bonjour: "Bonjour" },
    Langue { code: "en", nom: "English", locale: "en_US.UTF-8", language: "en_US:en", clavier: ("us", ""), fuseau: "Europe/London",
             decimal: '.', web: "https://www.google.com/", recherche: "https://www.google.com/search?q=", bonjour: "Hello" },
    Langue { code: "it", nom: "Italiano", locale: "it_IT.UTF-8", language: "it_IT:it", clavier: ("it", ""), fuseau: "Europe/Rome",
             decimal: ',', web: "https://www.google.it/", recherche: "https://www.google.it/search?q=", bonjour: "Ciao" },
    Langue { code: "es", nom: "Español", locale: "es_ES.UTF-8", language: "es_ES:es", clavier: ("es", ""), fuseau: "Europe/Madrid",
             decimal: ',', web: "https://www.google.es/", recherche: "https://www.google.es/search?q=", bonjour: "Hola" },
    Langue { code: "de", nom: "Deutsch", locale: "de_DE.UTF-8", language: "de_DE:de", clavier: ("de", ""), fuseau: "Europe/Berlin",
             decimal: ',', web: "https://www.google.de/", recherche: "https://www.google.de/search?q=", bonjour: "Hallo" },
    Langue { code: "zh", nom: "简体中文", locale: "zh_CN.UTF-8", language: "zh_CN:zh", clavier: ("us", ""), fuseau: "Asia/Shanghai",
             decimal: '.', web: "https://www.bing.com/", recherche: "https://www.bing.com/search?q=", bonjour: "你好" },
];

/// Langue en cours (variables LANGUAGE, LC_ALL, LC_MESSAGES, LANG) ; français par défaut
pub fn courante() -> &'static Langue {
    for v in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(t) = std::env::var(v) {
            let t = t.split(':').next().unwrap_or("").to_lowercase();
            if t.is_empty() || t == "c" || t.starts_with("c.") || t == "posix" {
                continue;
            }
            if let Some(l) = LANGUES.iter().find(|l| t.starts_with(l.code)) {
                return l;
            }
            return &LANGUES[1]; // langue inconnue : anglais
        }
    }
    &LANGUES[0]
}

/// Langue d'après son code (« fr », « en »…)
pub fn langue(code: &str) -> Option<&'static Langue> {
    LANGUES.iter().find(|l| l.code == code)
}

/// Nombre avec le séparateur décimal de la langue : 7,5 (français) ou 7.5 (anglais)
pub fn decimal(x: f64, chiffres: usize) -> String {
    let t = format!("{x:.chiffres$}");
    t.replace('.', &courante().decimal.to_string())
}

/// Entier avec séparateur de milliers : « 476 940 » (espace fine en français, virgule en anglais)
pub fn entier_espace(n: u64) -> String {
    let c = n.to_string();
    let sep = if courante().decimal == ',' { "\u{202f}" } else { "," };
    let mut t = String::new();
    for (i, ch) in c.chars().enumerate() {
        if i > 0 && (c.len() - i) % 3 == 0 {
            t.push_str(sep);
        }
        t.push(ch);
    }
    t
}

/// Pourcentage écrit à la façon de la langue : « 85 % » (français), « 85% » (anglais)
pub fn pourcent(n: impl std::fmt::Display) -> String {
    trv("{n} %", &[("n", &n.to_string())])
}

/// Texte traduit avec des valeurs : `trv("C'est prêt, {prenom} !", &[("prenom", &p)])`
pub fn trv(texte: &str, valeurs: &[(&str, &str)]) -> String {
    let mut t = tr(texte);
    for (nom, valeur) in valeurs {
        t = t.replace(&format!("{{{nom}}}"), valeur);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sans_traduction() {
        assert_eq!(tr("Bonjour"), "Bonjour");
        assert_eq!(trv("C'est prêt, {prenom} !", &[("prenom", "Ana")]), "C'est prêt, Ana !");
    }
    #[test]
    fn langues() {
        assert_eq!(LANGUES.len(), 6);
        assert_eq!(langue("zh").map(|l| l.clavier.0), Some("us"));
    }
}
