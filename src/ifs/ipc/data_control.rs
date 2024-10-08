use {
    crate::ifs::ipc::{DynDataSource, IpcLocation},
    std::rc::Rc,
};

mod private;
pub mod zwlr_data_control_device_v1;
pub mod zwlr_data_control_manager_v1;
pub mod zwlr_data_control_offer_v1;
pub mod zwlr_data_control_source_v1;

linear_ids!(DataControlDeviceIds, DataControlDeviceId, u64);

pub trait DynDataControlDevice {
    fn id(&self) -> DataControlDeviceId;

    fn handle_new_source(
        self: Rc<Self>,
        location: IpcLocation,
        source: Option<Rc<dyn DynDataSource>>,
    );
}
