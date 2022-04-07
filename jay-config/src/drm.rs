use {
    crate::drm::connector_type::{
        ConnectorType, CON_9PIN_DIN, CON_COMPONENT, CON_COMPOSITE, CON_DISPLAY_PORT, CON_DPI,
        CON_DSI, CON_DVIA, CON_DVID, CON_DVII, CON_EDP, CON_EMBEDDED_WINDOW, CON_HDMIA, CON_HDMIB,
        CON_LVDS, CON_SPI, CON_SVIDEO, CON_TV, CON_UNKNOWN, CON_USB, CON_VGA, CON_VIRTUAL,
        CON_WRITEBACK,
    },
    bincode::{Decode, Encode},
    std::str::FromStr,
};

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Mode {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) refresh_millihz: u32,
}

impl Mode {
    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn refresh_rate(&self) -> u32 {
        self.refresh_millihz
    }

    pub(crate) fn zeroed() -> Self {
        Self {
            width: 0,
            height: 0,
            refresh_millihz: 0,
        }
    }
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Connector(pub u64);

impl Connector {
    pub fn exists(self) -> bool {
        self.0 != 0
    }

    pub fn connected(self) -> bool {
        if !self.exists() {
            return false;
        }
        get!(false).connector_connected(self)
    }

    pub fn ty(self) -> ConnectorType {
        if !self.exists() {
            return CON_UNKNOWN;
        }
        get!(CON_UNKNOWN).connector_type(self)
    }

    pub fn mode(self) -> Mode {
        if !self.exists() {
            return Mode::zeroed();
        }
        get!(Mode::zeroed()).connector_mode(self)
    }

    pub fn width(self) -> i32 {
        self.mode().width
    }

    pub fn height(self) -> i32 {
        self.mode().height
    }

    pub fn refresh_rate(self) -> u32 {
        self.mode().refresh_millihz
    }

    pub fn set_position(self, x: i32, y: i32) {
        if !self.exists() {
            log::warn!("set_position called on a connector that does not exist");
            return;
        }
        get!().connector_set_position(self, x, y);
    }
}

pub fn on_new_connector<F: Fn(Connector) + 'static>(f: F) {
    get!().on_new_connector(f)
}

pub fn on_connector_connected<F: Fn(Connector) + 'static>(f: F) {
    get!().on_connector_connected(f)
}

pub fn get_connector(id: impl ToConnectorId) -> Connector {
    let (ty, idx) = match id.to_connector_id() {
        Ok(id) => id,
        Err(e) => {
            log::error!("{}", e);
            return Connector(0);
        }
    };
    get!(Connector(0)).get_connector(ty, idx)
}

pub trait ToConnectorId {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String>;
}

impl ToConnectorId for (ConnectorType, u32) {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String> {
        Ok(*self)
    }
}

impl ToConnectorId for &'_ str {
    fn to_connector_id(&self) -> Result<(ConnectorType, u32), String> {
        let pairs = [
            ("DP-", CON_DISPLAY_PORT),
            ("eDP-", CON_EDP),
            ("HDMI-A-", CON_HDMIA),
            ("HDMI-B-", CON_HDMIB),
            ("EmbeddedWindow-", CON_EMBEDDED_WINDOW),
            ("VGA-", CON_VGA),
            ("DVI-I-", CON_DVII),
            ("DVI-D-", CON_DVID),
            ("DVI-A-", CON_DVIA),
            ("Composite-", CON_COMPOSITE),
            ("SVIDEO-", CON_SVIDEO),
            ("LVDS-", CON_LVDS),
            ("Component-", CON_COMPONENT),
            ("DIN-", CON_9PIN_DIN),
            ("TV-", CON_TV),
            ("Virtual-", CON_VIRTUAL),
            ("DSI-", CON_DSI),
            ("DPI-", CON_DPI),
            ("Writeback-", CON_WRITEBACK),
            ("SPI-", CON_SPI),
            ("USB-", CON_USB),
        ];
        for (prefix, ty) in pairs {
            if let Some(suffix) = self.strip_prefix(prefix) {
                if let Ok(idx) = u32::from_str(suffix) {
                    return Ok((ty, idx));
                }
            }
        }
        Err(format!("`{}` is not a valid connector identifier", self))
    }
}

pub mod connector_type {
    use bincode::{Decode, Encode};

    #[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
    pub struct ConnectorType(pub u32);

    pub const CON_UNKNOWN: ConnectorType = ConnectorType(0);
    pub const CON_VGA: ConnectorType = ConnectorType(1);
    pub const CON_DVII: ConnectorType = ConnectorType(2);
    pub const CON_DVID: ConnectorType = ConnectorType(3);
    pub const CON_DVIA: ConnectorType = ConnectorType(4);
    pub const CON_COMPOSITE: ConnectorType = ConnectorType(5);
    pub const CON_SVIDEO: ConnectorType = ConnectorType(6);
    pub const CON_LVDS: ConnectorType = ConnectorType(7);
    pub const CON_COMPONENT: ConnectorType = ConnectorType(8);
    pub const CON_9PIN_DIN: ConnectorType = ConnectorType(9);
    pub const CON_DISPLAY_PORT: ConnectorType = ConnectorType(10);
    pub const CON_HDMIA: ConnectorType = ConnectorType(11);
    pub const CON_HDMIB: ConnectorType = ConnectorType(12);
    pub const CON_TV: ConnectorType = ConnectorType(13);
    pub const CON_EDP: ConnectorType = ConnectorType(14);
    pub const CON_VIRTUAL: ConnectorType = ConnectorType(15);
    pub const CON_DSI: ConnectorType = ConnectorType(16);
    pub const CON_DPI: ConnectorType = ConnectorType(17);
    pub const CON_WRITEBACK: ConnectorType = ConnectorType(18);
    pub const CON_SPI: ConnectorType = ConnectorType(19);
    pub const CON_USB: ConnectorType = ConnectorType(20);
    pub const CON_EMBEDDED_WINDOW: ConnectorType = ConnectorType(u32::MAX);
}
