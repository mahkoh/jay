use {
    crate::gfx_apis::vulkan::{device::VulkanDevice, VulkanError},
    ash::vk::{ShaderModule, ShaderModuleCreateInfo},
    std::rc::Rc,
    uapi::Packed,
};

pub const FILL_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fill.vert.spv"));
pub const FILL_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fill.frag.spv"));
pub const TEX_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tex.vert.spv"));
pub const TEX_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tex.frag.spv"));
pub const TEX_FRAG_MULT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tex.frag.mult.spv"));

pub struct VulkanShader {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) module: ShaderModule,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct FillVertPushConstants {
    pub pos: [[f32; 2]; 4],
}

unsafe impl Packed for FillVertPushConstants {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FillFragPushConstants {
    pub color: [f32; 4],
}

unsafe impl Packed for FillFragPushConstants {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TexVertPushConstants {
    pub pos: [[f32; 2]; 4],
    pub tex_pos: [[f32; 2]; 4],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TexFragPushConstants {
    pub alpha: f32,
}

unsafe impl Packed for TexVertPushConstants {}
unsafe impl Packed for TexFragPushConstants {}

impl VulkanDevice {
    pub(super) fn create_shader(
        self: &Rc<Self>,
        src: &[u8],
    ) -> Result<Rc<VulkanShader>, VulkanError> {
        let src: Vec<u32> = uapi::pod_iter(src).unwrap().collect();
        let create_info = ShaderModuleCreateInfo::default().code(&src);
        let module = unsafe { self.device.create_shader_module(&create_info, None) };
        module
            .map_err(VulkanError::CreateShaderModule)
            .map(|m| VulkanShader {
                device: self.clone(),
                module: m,
            })
            .map(Rc::new)
    }
}

impl Drop for VulkanShader {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_shader_module(self.module, None);
        }
    }
}
