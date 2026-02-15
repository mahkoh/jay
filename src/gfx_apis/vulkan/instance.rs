use {
    crate::{gfx_apis::vulkan::VulkanError, vulkan_core::VulkanCoreInstance},
    log::Level,
    std::{
        ops::{Deref, DerefMut},
        rc::Rc,
    },
};

pub struct VulkanInstance {
    instance: VulkanCoreInstance,
}

impl Deref for VulkanInstance {
    type Target = VulkanCoreInstance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl DerefMut for VulkanInstance {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl VulkanInstance {
    pub fn new(log_level: Level) -> Result<Rc<Self>, VulkanError> {
        Ok(Rc::new(Self {
            instance: VulkanCoreInstance::new(log_level)?,
        }))
    }
}
