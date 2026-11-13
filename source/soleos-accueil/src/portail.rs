//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Portail « fond d'écran » de SoleOS : « Définir comme arrière-plan » dans Fichiers (ou toute
//! application qui le demande) change le fond de l'accueil.
//! L'accueil répond lui-même au service org.freedesktop.impl.portal.Wallpaper
//! (déclaré dans /usr/share/xdg-desktop-portal/portals/soleos.portal).

use gtk::gio;
use gtk::prelude::*;


pub const NOM: &str = "org.freedesktop.impl.portal.desktop.soleos";
const CHEMIN: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = r#"
<node>
  <interface name="org.freedesktop.impl.portal.Wallpaper">
    <method name="SetWallpaperURI">
      <arg type="o" name="handle" direction="in"/>
      <arg type="s" name="app_id" direction="in"/>
      <arg type="s" name="parent_window" direction="in"/>
      <arg type="s" name="uri" direction="in"/>
      <arg type="a{sv}" name="options" direction="in"/>
      <arg type="u" name="response" direction="out"/>
    </method>
  </interface>
</node>
"#;

/// Démarre le service ; `appliquer` reçoit le chemin de l'image choisie et renvoie vrai si c'est fait
pub fn demarrer(appliquer: impl Fn(&std::path::Path) -> bool + 'static) {
    let appliquer = std::rc::Rc::new(appliquer);
    gio::bus_own_name(
        gio::BusType::Session,
        NOM,
        gio::BusNameOwnerFlags::REPLACE | gio::BusNameOwnerFlags::ALLOW_REPLACEMENT,
        move |connexion, _| {
            let Ok(noeud) = gio::DBusNodeInfo::for_xml(INTERFACE) else { return };
            let Some(info) = noeud.lookup_interface("org.freedesktop.impl.portal.Wallpaper") else { return };
            let appliquer = appliquer.clone();
            let _ = connexion
                .register_object(CHEMIN, &info)
                .method_call(move |_, _, _, _, methode, parametres, invocation| {
                    if methode != "SetWallpaperURI" {
                        invocation.return_error(gio::IOErrorEnum::NotSupported, "méthode inconnue");
                        return;
                    }
                    let uri: String = parametres.child_value(3).get().unwrap_or_default();
                    let chemin = if uri.starts_with('/') {
                        Some(std::path::PathBuf::from(&uri))
                    } else {
                        gio::File::for_uri(&uri).path()
                    };
                    let ok = chemin.map(|c| c.is_file() && appliquer(&c)).unwrap_or(false);
                    // 0 = fait, 2 = échec
                    invocation.return_value(Some(&(if ok { 0u32 } else { 2u32 },).into()));
                })
                .build();
        },
        |_, _| {},
        |_, nom| eprintln!("soleos-accueil : service {nom} indisponible"),
    );
}
