use {
    crate::gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
    ash::vk::{DeviceAddress, ShaderModule, ShaderModuleCreateInfo},
    std::rc::Rc,
    uapi::Packed,
};

pub const FILL_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fill.vert.spv"));
pub const FILL_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fill.frag.spv"));
pub const TEX_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tex.vert.spv"));
pub const TEX_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tex.frag.spv"));
pub const OUT_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/out.vert.spv"));
pub const OUT_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/out.frag.spv"));
pub const LEGACY_FILL_VERT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/legacy_fill.vert.spv"));
pub const LEGACY_FILL_FRAG: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/legacy_fill.frag.spv"));
pub const LEGACY_TEX_VERT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/legacy_tex.vert.spv"));
pub const LEGACY_TEX_FRAG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/legacy_tex.frag.spv"));

pub struct VulkanShader {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) module: ShaderModule,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct FillPushConstants {
    pub color: [f32; 4],
    pub vertices: DeviceAddress,
    pub _padding1: u32,
    pub _padding2: u32,
}

unsafe impl Packed for FillPushConstants {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct LegacyFillPushConstants {
    pub pos: [[f32; 2]; 4],
    pub color: [f32; 4],
}

unsafe impl Packed for LegacyFillPushConstants {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TexVertex {
    pub pos: [[f32; 2]; 4],
    pub tex_pos: [[f32; 2]; 4],
}

unsafe impl Packed for TexVertex {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TexPushConstants {
    pub vertices: DeviceAddress,
    pub alpha: f32,
}

unsafe impl Packed for TexPushConstants {}

#[derive(Copy, Clone, Debug)]
#[repr(C, align(16))]
pub struct TexColorManagementData {
    pub matrix: [[f32; 4]; 4],
}

unsafe impl Packed for TexColorManagementData {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct LegacyTexPushConstants {
    pub pos: [[f32; 2]; 4],
    pub tex_pos: [[f32; 2]; 4],
    pub alpha: f32,
}

unsafe impl Packed for LegacyTexPushConstants {}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct OutPushConstants {
    pub vertices: DeviceAddress,
}

unsafe impl Packed for OutPushConstants {}

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
