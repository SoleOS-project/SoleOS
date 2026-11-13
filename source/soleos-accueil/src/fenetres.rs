//! © 2026 GDH – créé par Giuseppe D'Arco – SPDX-License-Identifier: LicenseRef-SoleOS (licence non commerciale, voir LICENSE)
//!
//! Suivi des fenêtres ouvertes (protocole Wayland « foreign toplevel »).
//!
//! Le gestionnaire d'affichage (labwc) nous annonce chaque fenêtre ouverte,
//! son titre et son identifiant d'application. On peut aussi lui demander
//! d'en mettre une au premier plan ou de la fermer.

use std::sync::{Arc, Mutex};
use std::thread;

use wayland_client::protocol::{wl_registry, wl_seat::WlSeat};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Une fenêtre ouverte, telle que vue par l'accueil.
#[derive(Clone, Debug, PartialEq)]
pub struct Fenetre {
    pub id: u32,
    pub titre: String,
    pub app_id: String,
    /// Ordre de la dernière mise au premier plan (0 = jamais) : sert à repérer l'appli utilisée en dernier
    pub activation: u64,
    /// Fenêtre au premier plan (celle qui reçoit le clavier)
    pub active: bool,
    /// Fenêtre réduite (invisible, seulement dans la barre des tâches)
    pub reduite: bool,
    /// Fenêtre en plein écran
    pub plein_ecran: bool,
    /// Fenêtre agrandie (tout l'écran, avec sa barre de titre)
    pub agrandie: bool,
}

struct Entree {
    poignee: ZwlrForeignToplevelHandleV1,
    fenetre: Fenetre,
    prete: bool,
}

#[derive(Default)]
struct Partage {
    entrees: Vec<Entree>,
    siege: Option<WlSeat>,
    compteur: u64,
}

struct Etat {
    partage: Arc<Mutex<Partage>>,
    prevenir: Box<dyn Fn() + Send>,
}

/// Point d'accès utilisé par l'interface.
#[derive(Clone)]
pub struct Gestionnaire {
    partage: Arc<Mutex<Partage>>,
    connexion: Connection,
}

impl Gestionnaire {
    /// Se connecte au gestionnaire d'affichage. `prevenir` est appelé
    /// (depuis un autre fil d'exécution) à chaque changement.
    pub fn demarrer(prevenir: impl Fn() + Send + 'static) -> Option<Self> {
        let connexion = Connection::connect_to_env().ok()?;
        let partage = Arc::new(Mutex::new(Partage::default()));
        let mut file = connexion.new_event_queue();
        let qh = file.handle();
        connexion.display().get_registry(&qh, ());

        let mut etat = Etat { partage: partage.clone(), prevenir: Box::new(prevenir) };
        // Premier aller-retour : découverte des services disponibles.
        file.roundtrip(&mut etat).ok()?;
        // Second aller-retour : réception de la liste des fenêtres déjà ouvertes.
        file.roundtrip(&mut etat).ok()?;

        thread::Builder::new()
            .name("fenetres".into())
            .spawn(move || {
                while file.blocking_dispatch(&mut etat).is_ok() {}
                eprintln!("soleos-accueil : connexion au gestionnaire d'affichage perdue");
            })
            .ok()?;

        Some(Gestionnaire { partage, connexion })
    }

    /// Fenêtres actuellement ouvertes (dans l'ordre d'ouverture).
    pub fn liste(&self) -> Vec<Fenetre> {
        let p = self.partage.lock().unwrap();
        p.entrees.iter().filter(|e| e.prete).map(|e| e.fenetre.clone()).collect()
    }

    /// Met une fenêtre au premier plan.
    pub fn activer(&self, id: u32) {
        let p = self.partage.lock().unwrap();
        if let (Some(e), Some(siege)) = (p.entrees.iter().find(|e| e.fenetre.id == id), &p.siege) {
            e.poignee.unset_minimized();
            e.poignee.activate(siege);
        }
        drop(p);
        let _ = self.connexion.flush();
    }

    /// Réduit une fenêtre (elle reste ouverte, cachée).
    pub fn minimiser(&self, id: u32) {
        let p = self.partage.lock().unwrap();
        if let Some(e) = p.entrees.iter().find(|e| e.fenetre.id == id) {
            e.poignee.set_minimized();
        }
        drop(p);
        let _ = self.connexion.flush();
    }

    /// Agrandit une fenêtre (tout l'écran, barres comprises).
    pub fn agrandir(&self, id: u32) {
        let p = self.partage.lock().unwrap();
        if let Some(e) = p.entrees.iter().find(|e| e.fenetre.id == id) {
            e.poignee.set_maximized();
        }
        drop(p);
        let _ = self.connexion.flush();
    }

    /// Demande poliment à une fenêtre de se fermer.
    pub fn fermer(&self, id: u32) {
        let p = self.partage.lock().unwrap();
        if let Some(e) = p.entrees.iter().find(|e| e.fenetre.id == id) {
            e.poignee.close();
        }
        drop(p);
        let _ = self.connexion.flush();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Etat {
    fn event(
        etat: &mut Self,
        registre: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == ZwlrForeignToplevelManagerV1::interface().name {
                registre.bind::<ZwlrForeignToplevelManagerV1, _, _>(name, version.min(3), qh, ());
            } else if interface == WlSeat::interface().name {
                let mut p = etat.partage.lock().unwrap();
                if p.siege.is_none() {
                    p.siege = Some(registre.bind::<WlSeat, _, _>(name, version.min(1), qh, ()));
                }
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for Etat {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Etat {
    fn event(
        etat: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            let id = toplevel.id().protocol_id();
            etat.partage.lock().unwrap().entrees.push(Entree {
                poignee: toplevel,
                fenetre: Fenetre { id, titre: String::new(), app_id: String::new(), activation: 0, active: false, reduite: false, plein_ecran: false, agrandie: false },
                prete: false,
            });
        }
    }

    event_created_child!(Etat, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Etat {
    fn event(
        etat: &mut Self,
        poignee: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        let id = poignee.id().protocol_id();
        let mut changement = false;
        {
            let mut p = etat.partage.lock().unwrap();
            match event {
                Event::Title { title } => {
                    if let Some(e) = p.entrees.iter_mut().find(|e| e.fenetre.id == id) {
                        e.fenetre.titre = title;
                    }
                }
                Event::AppId { app_id } => {
                    if let Some(e) = p.entrees.iter_mut().find(|e| e.fenetre.id == id) {
                        e.fenetre.app_id = app_id;
                    }
                }
                Event::State { state } => {
                    // Liste d'entiers 32 bits : 0 = agrandie, 1 = réduite, 2 = active, 3 = plein écran
                    let etats: Vec<u32> = state.chunks_exact(4).map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect();
                    let active = etats.contains(&2);
                    if let Some(e) = p.entrees.iter_mut().find(|e| e.fenetre.id == id) {
                        e.fenetre.active = active;
                        e.fenetre.reduite = etats.contains(&1);
                        e.fenetre.plein_ecran = etats.contains(&3);
                        e.fenetre.agrandie = etats.contains(&0);
                    }
                    if active {
                        p.compteur += 1;
                        let n = p.compteur;
                        if let Some(e) = p.entrees.iter_mut().find(|e| e.fenetre.id == id) {
                            e.fenetre.activation = n;
                        }
                    }
                }
                Event::Done => {
                    if let Some(e) = p.entrees.iter_mut().find(|e| e.fenetre.id == id) {
                        e.prete = true;
                    }
                    changement = true;
                }
                Event::Closed => {
                    p.entrees.retain(|e| e.fenetre.id != id);
                    poignee.destroy();
                    changement = true;
                }
                _ => {}
            }
        }
        if changement {
            (etat.prevenir)();
        }
    }
}
