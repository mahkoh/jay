// SPDX-License-Identifier: MIT OR Apache-2.0
// Author: Zakarum <zakarumych@ya.ru>
// https://github.com/zakarumych/gpu-alloc

use {
    ash::{
        Device, Instance,
        vk::{
            self, PhysicalDeviceFeatures2, PhysicalDeviceProperties2,
            PhysicalDeviceVulkan11Properties,
        },
    },
    gpu_alloc_types::{
        AllocationFlags, DeviceMapError, DeviceProperties, MappedMemoryRange, MemoryDevice,
        MemoryHeap, MemoryPropertyFlags, MemoryType, OutOfMemory,
    },
    smallvec::SmallVec,
    std::{mem, ptr::NonNull},
};

#[repr(transparent)]
pub struct AshMemoryDevice {
    device: Device,
}

impl AshMemoryDevice {
    pub fn wrap(device: &Device) -> &Self {
        unsafe { mem::transmute::<&Device, &Self>(device) }
    }
}

impl MemoryDevice<vk::DeviceMemory> for AshMemoryDevice {
    unsafe fn allocate_memory(
        &self,
        size: u64,
        memory_type: u32,
        flags: AllocationFlags,
    ) -> Result<vk::DeviceMemory, OutOfMemory> {
        assert!((flags & !AllocationFlags::DEVICE_ADDRESS).is_empty());
        let mut info = vk::MemoryAllocateInfo::default()
            .allocation_size(size)
            .memory_type_index(memory_type);
        let mut info_flags;
        if flags.contains(AllocationFlags::DEVICE_ADDRESS) {
            info_flags = vk::MemoryAllocateFlagsInfo::default()
                .flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);
            info = info.push_next(&mut info_flags);
        }
        let res = unsafe { self.device.allocate_memory(&info, None) };
        match res {
            Ok(memory) => Ok(memory),
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(OutOfMemory::OutOfDeviceMemory),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(OutOfMemory::OutOfHostMemory),
            Err(vk::Result::ERROR_TOO_MANY_OBJECTS) => panic!("Too many objects"),
            Err(err) => panic!("Unexpected Vulkan error: `{}`", err),
        }
    }

    unsafe fn deallocate_memory(&self, memory: vk::DeviceMemory) {
        unsafe {
            self.device.free_memory(memory, None);
        }
    }

    unsafe fn map_memory(
        &self,
        memory: &mut vk::DeviceMemory,
        offset: u64,
        size: u64,
    ) -> Result<NonNull<u8>, DeviceMapError> {
        let res = unsafe {
            self.device
                .map_memory(*memory, offset, size, vk::MemoryMapFlags::empty())
        };
        match res {
            Ok(ptr) => {
                Ok(NonNull::new(ptr as *mut u8)
                    .expect("Pointer to memory mapping must not be null"))
            }
            Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY) => Err(DeviceMapError::OutOfDeviceMemory),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY) => Err(DeviceMapError::OutOfHostMemory),
            Err(vk::Result::ERROR_MEMORY_MAP_FAILED) => Err(DeviceMapError::MapFailed),
            Err(err) => panic!("Unexpected Vulkan error: `{}`", err),
        }
    }

    unsafe fn unmap_memory(&self, memory: &mut vk::DeviceMemory) {
        unsafe {
            self.device.unmap_memory(*memory);
        }
    }

    unsafe fn invalidate_memory_ranges(
        &self,
        ranges: &[MappedMemoryRange<'_, vk::DeviceMemory>],
    ) -> Result<(), OutOfMemory> {
        unsafe {
            self.device
                .invalidate_mapped_memory_ranges(&map_ranges(ranges))
                .map_err(|err| match err {
                    vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory::OutOfDeviceMemory,
                    vk::Result::ERROR_OUT_OF_HOST_MEMORY => OutOfMemory::OutOfHostMemory,
                    err => panic!("Unexpected Vulkan error: `{}`", err),
                })
        }
    }

    unsafe fn flush_memory_ranges(
        &self,
        ranges: &[MappedMemoryRange<'_, vk::DeviceMemory>],
    ) -> Result<(), OutOfMemory> {
        unsafe {
            self.device
                .flush_mapped_memory_ranges(&map_ranges(ranges))
                .map_err(|err| match err {
                    vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemory::OutOfDeviceMemory,
                    vk::Result::ERROR_OUT_OF_HOST_MEMORY => OutOfMemory::OutOfHostMemory,
                    err => panic!("Unexpected Vulkan error: `{}`", err),
                })
        }
    }
}

pub unsafe fn device_properties(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<DeviceProperties<'static>, vk::Result> {
    let mut properties_11 = PhysicalDeviceVulkan11Properties::default();
    let mut properties = PhysicalDeviceProperties2::default().push_next(&mut properties_11);
    unsafe {
        instance.get_physical_device_properties2(physical_device, &mut properties);
    };
    let limits = properties.properties.limits;
    let memory_properties =
        unsafe { instance.get_physical_device_memory_properties(physical_device) };
    let buffer_device_address = {
        let mut bda_features = vk::PhysicalDeviceBufferDeviceAddressFeatures::default();
        let mut features = PhysicalDeviceFeatures2::default().push_next(&mut bda_features);
        unsafe {
            instance.get_physical_device_features2(physical_device, &mut features);
        }
        bda_features.buffer_device_address != 0
    };
    Ok(DeviceProperties {
        max_memory_allocation_count: limits.max_memory_allocation_count,
        max_memory_allocation_size: properties_11.max_memory_allocation_size,
        non_coherent_atom_size: limits.non_coherent_atom_size,
        memory_types: memory_properties.memory_types
            [..memory_properties.memory_type_count as usize]
            .iter()
            .map(|memory_type| MemoryType {
                props: memory_properties_from_ash(memory_type.property_flags),
                heap: memory_type.heap_index,
            })
            .collect(),
        memory_heaps: memory_properties.memory_heaps
            [..memory_properties.memory_heap_count as usize]
            .iter()
            .map(|&memory_heap| MemoryHeap {
                size: memory_heap.size,
            })
            .collect(),
        buffer_device_address,
    })
}

fn memory_properties_from_ash(props: vk::MemoryPropertyFlags) -> MemoryPropertyFlags {
    let mut result = MemoryPropertyFlags::empty();
    if props.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL) {
        result |= MemoryPropertyFlags::DEVICE_LOCAL;
    }
    if props.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
        result |= MemoryPropertyFlags::HOST_VISIBLE;
    }
    if props.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {
        result |= MemoryPropertyFlags::HOST_COHERENT;
    }
    if props.contains(vk::MemoryPropertyFlags::HOST_CACHED) {
        result |= MemoryPropertyFlags::HOST_CACHED;
    }
    if props.contains(vk::MemoryPropertyFlags::LAZILY_ALLOCATED) {
        result |= MemoryPropertyFlags::LAZILY_ALLOCATED;
    }
    result
}

fn map_ranges(
    ranges: &[MappedMemoryRange<'_, vk::DeviceMemory>],
) -> SmallVec<[vk::MappedMemoryRange<'static>; 4]> {
    ranges
        .iter()
        .map(|range| {
            vk::MappedMemoryRange::default()
                .memory(*range.memory)
                .offset(range.offset)
                .size(range.size)
        })
        .collect()
}
