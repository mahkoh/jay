use {
    crate::{
        gfx_api::FdSync,
        utils::errorfmt::ErrorFmt,
        vulkan_core::{
            VulkanCoreError,
            device::VulkanDeviceInf,
            fence::{VulkanDeviceFenceExt, VulkanFence},
        },
    },
    ash::vk::Fence,
    std::rc::Rc,
};

pub enum VulkanSync<D>
where
    D: VulkanDeviceInf,
{
    Fence(Rc<VulkanFence<D>>),
}

impl<D> VulkanSync<D>
where
    D: VulkanDeviceInf,
{
    pub fn handle_validation(&self) {
        // nothing
    }

    pub fn fence(&self) -> Fence {
        match self {
            VulkanSync::Fence(f) => f.fence,
        }
    }

    pub fn to_sync(&self, block: impl FnOnce()) -> Option<FdSync> {
        match self {
            VulkanSync::Fence(release_fence) => {
                zone!("export_sync_file");
                let release_sync_file = match release_fence.export_sync_file() {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Could not export sync file from fence: {}", ErrorFmt(e));
                        block();
                        None
                    }
                };
                release_sync_file.map(FdSync::SyncFile)
            }
        }
    }
}

pub trait VulkanDeviceSyncExt: VulkanDeviceInf {
    fn create_sync(self: &Rc<Self>) -> Result<VulkanSync<Self>, VulkanCoreError>;
}

impl<D> VulkanDeviceSyncExt for D
where
    D: VulkanDeviceInf,
{
    fn create_sync(self: &Rc<Self>) -> Result<VulkanSync<Self>, VulkanCoreError> {
        self.create_fence().map(VulkanSync::Fence)
    }
}
