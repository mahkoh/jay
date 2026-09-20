use crate::client::Client;
use crate::ifs::wl_buffer::SyntheticWlBuffer;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1::BufferKey;
use crate::scale::Scale;
use crate::utils::clonecell::CloneCell;
use crate::utils::event_listener::EventListener;
use crate::utils::event_listener::EventSource;
use crate::utils::fx_hash::FHashMap;
use crate::utils::type_view::TypeViewExt1;
use crate::wire::JayIconSurfaceFactoryV1Id;
use crate::wire::JayIconSurfaceV1Id;
use crate::wire::JayWlSurfaceFactoryV1Id;
use crate::wire::WlBufferId;
use crate::wire::WlSurfaceId;
use crate::wire::WpFractionalScaleV1Id;
use crate::wire::WpViewportId;
use crate::wire::XdgToplevelId;
use std::cell::Cell;
use std::rc::Rc;

pub struct XdgToplevelIconBridge {
    client: Rc<Client>,
    toplevel: XdgToplevelId,
    bridge: CloneCell<Option<Rc<ToplevelIconBridge>>>,
}

struct ToplevelIconBridge {
    client: Rc<Client>,
    wl_surface_factory: JayWlSurfaceFactoryV1Id,
    factory: JayIconSurfaceFactoryV1Id,
    buffers: CloneCell<Rc<FHashMap<BufferKey, SyntheticWlBuffer>>>,
    pending_surface: Cell<WlSurfaceId>,
    surfaces: EventSource<ToplevelIconSurfaceBridge>,
}

struct ToplevelIconSurfaceBridge {
    bridge: Rc<ToplevelIconBridge>,
    client: Rc<Client>,
    surface: WlSurfaceId,
    icon: JayIconSurfaceV1Id,
    fractional_scale: WpFractionalScaleV1Id,
    viewport: WpViewportId,
    size: Cell<(i32, i32)>,
    pending_size: Cell<Option<(i32, i32)>>,
    scale: Cell<Scale>,
    _listener: EventListener<ToplevelIconSurfaceBridge>,
}

impl XdgToplevelIconBridge {
    pub fn new(client: &Rc<Client>, toplevel: XdgToplevelId) -> Self {
        Self {
            client: client.clone(),
            toplevel,
            bridge: Default::default(),
        }
    }

    pub fn set_buffers(&self, buffers: Option<Rc<FHashMap<BufferKey, SyntheticWlBuffer>>>) {
        let Some(buffers) = buffers else {
            self.destroy();
            return;
        };
        if let Some(bridge) = self.bridge.get() {
            bridge.buffers.set(buffers);
            bridge.surfaces.for_each(|surface| surface.commit());
            return;
        }
        let c = &self.client;
        let wl_surface_factory = c.send_jay_wl_surface_factory_manager_v1_create_factory();
        let subject = c.send_jay_toplevel_icon_subject_manager_v1_create_subject(self.toplevel);
        let factory =
            c.send_jay_icon_surface_manager_v1_create_factory(subject, wl_surface_factory);
        c.send_jay_icon_surface_subject_v1_destroy(subject);
        let bridge = Rc::new(ToplevelIconBridge {
            client: c.clone(),
            wl_surface_factory,
            factory,
            buffers: CloneCell::new(buffers),
            pending_surface: Cell::new(WlSurfaceId::NONE),
            surfaces: Default::default(),
        });
        c.set_synthetic_event_handler(
            bridge.wl_surface_factory,
            bridge.tv_wrap_rc_ref::<wl_surface_factory::View>(),
        );
        c.set_synthetic_event_handler(
            bridge.factory, //
            bridge.tv_wrap_rc_ref::<factory::View>(),
        );
        self.bridge.set(Some(bridge));
    }

    fn destroy(&self) {
        if let Some(bridge) = self.bridge.take() {
            self.client
                .send_jay_icon_surface_factory_v1_stop(bridge.factory);
        }
    }
}

impl Drop for XdgToplevelIconBridge {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl ToplevelIconSurfaceBridge {
    fn destroy(&self) {
        self.client.send_wp_viewport_destroy(self.viewport);
        self.client
            .send_wp_fractional_scale_v1_destroy(self.fractional_scale);
        self.client.send_jay_icon_surface_v1_destroy(self.icon);
        self.client.send_wl_surface_destroy(self.surface);
    }
}

impl ToplevelIconBridge {
    pub fn choose_buffer(&self, scale: Scale, width: i32, height: i32) -> WlBufferId {
        let scalef = scale.to_f64();
        let [width, height] = scale.pixel_size([width, height]);
        #[derive(Copy, Clone, PartialOrd, PartialEq)]
        enum Mismatch {
            Same,
            Greater(f64),
            Smaller(f64),
        }
        #[derive(Copy, Clone, PartialOrd, PartialEq)]
        struct Quality {
            width: Mismatch,
            height: Mismatch,
            scale: Mismatch,
        }
        let mut best_quality = None::<Quality>;
        let mut best = WlBufferId::NONE;
        for (key, buffer) in &*self.buffers.get() {
            let mut quality = Quality {
                width: Mismatch::Same,
                height: Mismatch::Same,
                scale: Mismatch::Same,
            };
            for (val, field) in [(width, &mut quality.width), (height, &mut quality.height)] {
                if key.size > val {
                    *field = Mismatch::Greater((key.size - val) as f64);
                } else if key.size < val {
                    *field = Mismatch::Smaller((val - key.size) as f64);
                }
            }
            let key_scale = key.scale as f64;
            if key_scale > scalef {
                quality.scale = Mismatch::Greater((key_scale - scalef) as f64);
            } else if key_scale < scalef {
                quality.scale = Mismatch::Smaller((scalef - key_scale) as f64);
            }
            if let Some(old) = best_quality
                && old < quality
            {
                continue;
            }
            best_quality = Some(quality);
            best = buffer.id;
        }
        best
    }
}

impl ToplevelIconSurfaceBridge {
    fn commit(&self) {
        let c = &self.client;
        let scale = self.scale.get();
        let (width, height) = self.size.get();
        if width <= 0 || height <= 0 {
            return;
        }
        let buffer = self.bridge.choose_buffer(scale, width, height);
        c.send_wl_surface_attach(self.surface, buffer, 0, 0);
        c.send_wl_surface_damage(self.surface, 0, 0, width, height);
        c.send_wp_viewport_set_destination(self.viewport, width, height);
        c.send_wl_surface_commit(self.surface);
    }
}

mod wl_surface_factory {
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::ToplevelIconBridge;
    use crate::utils::type_view::TypeView;
    use crate::wire::jay_wl_surface_factory_v1::*;
    use std::convert::Infallible;
    use std::rc::Rc;

    pub struct View;
    synthetic_event_handler!(TypeView<ToplevelIconBridge, View>);

    impl JayWlSurfaceFactoryV1EventHandler for TypeView<ToplevelIconBridge, View> {
        type Error = Infallible;

        fn surface(&self, ev: Surface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.pending_surface.set(ev.id);
            Ok(())
        }
    }
}

mod factory {
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::ToplevelIconBridge;
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::ToplevelIconSurfaceBridge;
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::fractional_scale;
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::icon_surface;
    use crate::utils::event_listener::EventListener;
    use crate::utils::type_view::TypeView;
    use crate::utils::type_view::TypeViewExt1;
    use crate::utils::type_view::TypeViewExt2;
    use crate::wire::jay_icon_surface_factory_v1::*;
    use std::convert::Infallible;
    use std::rc::Rc;

    pub struct View;
    synthetic_event_handler!(TypeView<ToplevelIconBridge, View>);

    impl JayIconSurfaceFactoryV1EventHandler for TypeView<ToplevelIconBridge, View> {
        type Error = Infallible;

        fn stopped(&self, _ev: Stopped, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.client
                .send_jay_icon_surface_factory_v1_destroy(self.factory);
            self.client
                .send_jay_wl_surface_factory_v1_destroy(self.wl_surface_factory);
            self.surfaces.for_each(|surface| surface.destroy());
            Ok(())
        }

        fn surface(&self, ev: Surface, slf: &Rc<Self>) -> Result<(), Self::Error> {
            let surface = self.pending_surface.get();
            let fractional_scale = self
                .client
                .send_wp_fractional_scale_manager_v1_get_fractional_scale(surface);
            let viewport = self.client.send_wp_viewporter_get_viewport(surface);
            let icon =
                Rc::<ToplevelIconSurfaceBridge>::new_cyclic(|weak| ToplevelIconSurfaceBridge {
                    bridge: slf.tv_unwrap_rc_ref().clone(),
                    client: self.client.clone(),
                    surface,
                    icon: ev.id,
                    fractional_scale,
                    viewport,
                    size: Default::default(),
                    pending_size: Default::default(),
                    scale: Default::default(),
                    _listener: EventListener::attached(weak.clone(), &self.surfaces),
                });
            self.client.set_synthetic_event_handler(
                icon.icon,
                icon.tv_wrap_rc_ref::<icon_surface::View>(),
            );
            self.client.set_synthetic_event_handler(
                icon.fractional_scale,
                icon.tv_wrap_rc_ref::<fractional_scale::View>(),
            );
            Ok(())
        }
    }
}

mod icon_surface {
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::ToplevelIconSurfaceBridge;
    use crate::utils::type_view::TypeView;
    use crate::wire::jay_icon_surface_v1::*;
    use std::convert::Infallible;
    use std::rc::Rc;

    pub struct View;
    synthetic_event_handler!(TypeView<ToplevelIconSurfaceBridge, View>);

    impl JayIconSurfaceV1EventHandler for TypeView<ToplevelIconSurfaceBridge, View> {
        type Error = Infallible;

        fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.destroy();
            Ok(())
        }

        fn configure(&self, ev: Configure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.client
                .send_jay_icon_surface_v1_ack_configure(self.icon, ev.serial);
            if let Some(v) = self.pending_size.take() {
                self.size.set(v);
            }
            self.commit();
            Ok(())
        }

        fn configure_size(&self, ev: ConfigureSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.pending_size.set(Some((ev.width, ev.height)));
            Ok(())
        }
    }
}

mod fractional_scale {
    use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1_bridge::ToplevelIconSurfaceBridge;
    use crate::scale::Scale;
    use crate::utils::type_view::TypeView;
    use crate::wire::wp_fractional_scale_v1::*;
    use std::convert::Infallible;
    use std::rc::Rc;

    pub struct View;
    synthetic_event_handler!(TypeView<ToplevelIconSurfaceBridge, View>);

    impl WpFractionalScaleV1EventHandler for TypeView<ToplevelIconSurfaceBridge, View> {
        type Error = Infallible;

        fn preferred_scale(&self, ev: PreferredScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
            self.scale.set(Scale::from_wl(ev.scale));
            self.commit();
            Ok(())
        }
    }
}
