//! Application dismissal policy over the SDK's surface-bound Wayland interface.
use halley_api::wayland::client::{
    halley_surface_dismissal_manager_v1::{self as manager, HalleySurfaceDismissalManagerV1},
    halley_surface_dismissal_v1::{self as watch, HalleySurfaceDismissalV1},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, globals::GlobalList, protocol::wl_surface,
};

use crate::LiftApp;

pub struct OutsideDismissal {
    _manager: Option<HalleySurfaceDismissalManagerV1>,
    watch: Option<HalleySurfaceDismissalV1>,
}

impl OutsideDismissal {
    pub fn bind(
        globals: &GlobalList,
        qh: &QueueHandle<LiftApp>,
        surface: &wl_surface::WlSurface,
    ) -> Self {
        let manager: Option<HalleySurfaceDismissalManagerV1> = globals.bind(qh, 1..=1, ()).ok();
        let watch = manager
            .as_ref()
            .map(|manager| manager.watch(surface, qh, ()));
        Self {
            _manager: manager,
            watch,
        }
    }

    fn finish(&mut self) {
        if let Some(watch) = self.watch.take() {
            watch.destroy();
        }
    }
}

impl Dispatch<HalleySurfaceDismissalManagerV1, ()> for LiftApp {
    fn event(
        _: &mut Self,
        _: &HalleySurfaceDismissalManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HalleySurfaceDismissalV1, ()> for LiftApp {
    fn event(
        app: &mut Self,
        _: &HalleySurfaceDismissalV1,
        event: watch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let watch::Event::OutsidePress = event {
            app.outside_dismissal.finish();
            app.escape_dismissal.cancel();
            app.exit("outside-press");
        }
    }
}
