//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! (v1.2) Serveur de notifications de SoleOS (org.freedesktop.Notifications) : les applications
//! (Thunderbird, Chrome, Telegram…) peuvent afficher une petite bulle en bas à droite.
//! Les nouveaux mails de Thunderbird sont aussi comptés : enveloppe dans la barre du bas.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio;
use gtk::glib;

const NOM: &str = "org.freedesktop.Notifications";
const CHEMIN: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = r#"
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities"><arg type="as" direction="out"/></method>
    <method name="Notify">
      <arg type="s" name="app_name" direction="in"/>
      <arg type="u" name="replaces_id" direction="in"/>
      <arg type="s" name="app_icon" direction="in"/>
      <arg type="s" name="summary" direction="in"/>
      <arg type="s" name="body" direction="in"/>
      <arg type="as" name="actions" direction="in"/>
      <arg type="a{sv}" name="hints" direction="in"/>
      <arg type="i" name="expire_timeout" direction="in"/>
      <arg type="u" name="id" direction="out"/>
    </method>
    <method name="CloseNotification"><arg type="u" name="id" direction="in"/></method>
    <method name="GetServerInformation">
      <arg type="s" name="name" direction="out"/>
      <arg type="s" name="vendor" direction="out"/>
      <arg type="s" name="version" direction="out"/>
      <arg type="s" name="spec_version" direction="out"/>
    </method>
    <signal name="NotificationClosed"><arg type="u" name="id"/><arg type="u" name="reason"/></signal>
    <signal name="ActionInvoked"><arg type="u" name="id"/><arg type="s" name="action_key"/></signal>
  </interface>
</node>
"#;

#[derive(Clone, Debug)]
pub struct Notif {
    pub id: u32,
    pub appli: String,
    pub titre: String,
    pub texte: String,
    /// Vrai pour un nouveau mail de Thunderbird
    pub mail: bool,
    /// Nombre de mails annoncés (« 3 nouveaux messages » → 3)
    pub nombre: u32,
    pub duree_ms: u32,
}

/// Retire les balises (le texte peut contenir <b>, <a href…>) et décode les entités courantes
pub fn texte_simple(t: &str) -> String {
    let mut r = String::new();
    let mut dans = false;
    for c in t.chars() {
        match c {
            '<' => dans = true,
            '>' if dans => dans = false,
            _ if !dans => r.push(c),
            _ => {}
        }
    }
    r.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'").replace("&amp;", "&")
}

/// Notification de Thunderbird ? (nom de l'application ou application d'origine, directe ou par le portail)
pub fn est_thunderbird(appli: &str, entree: &str) -> bool {
    appli.to_lowercase().contains("thunderbird") || entree.to_lowercase().contains("thunderbird")
}

/// « 3 new messages » / « 3 nouveaux messages » → 3 ; sinon 1
pub fn nombre_annonce(titre: &str) -> u32 {
    let n: String = titre.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse().ok().filter(|n| *n > 0 && *n < 1000).unwrap_or(1)
}

pub struct Serveur {
    connexion: RefCell<Option<gio::DBusConnection>>,
    suivant: Cell<u32>,
}

impl Serveur {
    pub fn signaler_clic(&self, id: u32) {
        if let Some(c) = self.connexion.borrow().as_ref() {
            let _ = c.emit_signal(None, CHEMIN, NOM, "ActionInvoked", Some(&(id, "default").into()));
        }
        self.signaler_fermee(id, 2);
    }
    /// raison : 1 = expirée, 2 = fermée par l'utilisateur, 3 = fermée par l'application
    pub fn signaler_fermee(&self, id: u32, raison: u32) {
        if let Some(c) = self.connexion.borrow().as_ref() {
            let _ = c.emit_signal(None, CHEMIN, NOM, "NotificationClosed", Some(&(id, raison).into()));
        }
    }
}

/// Démarre le serveur (s'il n'y en a pas déjà un). `recevoir` est appelé pour chaque notification,
/// `fermer` quand une application retire la sienne.
pub fn demarrer(recevoir: impl Fn(Notif) + 'static, fermer: impl Fn(u32) + 'static) -> Rc<Serveur> {
    let serveur = Rc::new(Serveur { connexion: RefCell::new(None), suivant: Cell::new(1) });
    let (recevoir, fermer) = (Rc::new(recevoir), Rc::new(fermer));
    let s = serveur.clone();
    gio::bus_own_name(
        gio::BusType::Session,
        NOM,
        gio::BusNameOwnerFlags::NONE,
        move |connexion, _| {
            let Ok(noeud) = gio::DBusNodeInfo::for_xml(INTERFACE) else { return };
            let Some(info) = noeud.lookup_interface(NOM) else { return };
            *s.connexion.borrow_mut() = Some(connexion.clone());
            let (s2, recevoir, fermer) = (s.clone(), recevoir.clone(), fermer.clone());
            let _ = connexion
                .register_object(CHEMIN, &info)
                .method_call(move |_, _, _, _, methode, p, invocation| match methode {
                    "GetCapabilities" => {
                        invocation.return_value(Some(&(vec!["body".to_string(), "actions".to_string()],).into()));
                    }
                    "GetServerInformation" => {
                        invocation.return_value(Some(&("SoleOS", "GDH", "1.2", "1.2").into()));
                    }
                    "CloseNotification" => {
                        let id: u32 = p.child_value(0).get().unwrap_or(0);
                        fermer(id);
                        s2.signaler_fermee(id, 3);
                        invocation.return_value(None);
                    }
                    "Notify" => {
                        let appli: String = p.child_value(0).get().unwrap_or_default();
                        let remplace: u32 = p.child_value(1).get().unwrap_or(0);
                        let titre: String = p.child_value(3).get().unwrap_or_default();
                        let texte: String = p.child_value(4).get().unwrap_or_default();
                        let indices = p.child_value(6);
                        let entree = indices
                            .get::<std::collections::HashMap<String, glib::Variant>>()
                            .and_then(|h| h.get("desktop-entry").and_then(|v| v.get::<String>()))
                            .unwrap_or_default();
                        let expire: i32 = p.child_value(7).get().unwrap_or(-1);
                        let id = if remplace > 0 {
                            remplace
                        } else {
                            let i = s2.suivant.get();
                            s2.suivant.set(i.wrapping_add(1).max(1));
                            i
                        };
                        let mail = est_thunderbird(&appli, &entree);
                        let titre = texte_simple(&titre);
                        let n = Notif {
                            id,
                            nombre: nombre_annonce(&titre),
                            appli,
                            titre,
                            texte: texte_simple(&texte),
                            mail,
                            duree_ms: if expire > 0 { (expire as u32).clamp(3000, 10_000) } else { 6000 },
                        };
                        invocation.return_value(Some(&(id,).into()));
                        recevoir(n);
                    }
                    _ => invocation.return_error(gio::IOErrorEnum::NotSupported, "méthode inconnue"),
                })
                .build();
        },
        |_, _| {},
        |_, nom| eprintln!("soleos-accueil : service {nom} indisponible (un autre serveur de notifications ?)"),
    );
    serveur
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texte_et_nombre() {
        assert_eq!(texte_simple("<b>Paul</b> &amp; Marie &lt;x&gt;"), "Paul & Marie <x>");
        assert_eq!(nombre_annonce("3 nouveaux messages"), 3);
        assert_eq!(nombre_annonce("Facture de mars"), 1);
        assert!(est_thunderbird("Thunderbird", ""));
        assert!(est_thunderbird("", "org.mozilla.Thunderbird"));
        assert!(!est_thunderbird("Google Chrome", "com.google.Chrome"));
    }
}
