use {
    crate::{
        cpu_worker::CpuWorker,
        format::{Format, FormatShmInfo},
        gfx_api::SyncFile,
        gfx_apis::vulkan::{
            allocator::VulkanAllocation,
            command::VulkanCommandBuffer,
            fence::VulkanFence,
            image::{QueueFamily, QueueState, VulkanImage, VulkanImageMemory},
            renderer::{image_barrier, VulkanRenderer},
            staging::VulkanStagingBuffer,
            transfer::{TransferType, VulkanShmImageAsyncData},
            VulkanError,
        },
        rect::Rect,
        utils::{errorfmt::ErrorFmt, on_drop::OnDrop},
    },
    ash::vk::{
        AccessFlags2, BufferImageCopy2, BufferMemoryBarrier2, CommandBufferBeginInfo,
        CommandBufferSubmitInfo, CommandBufferUsageFlags, CopyBufferToImageInfo2,
        CopyImageToBufferInfo2, DependencyInfoKHR, DeviceSize, Extent3D, ImageAspectFlags,
        ImageCreateInfo, ImageLayout, ImageSubresourceLayers, ImageSubresourceRange, ImageTiling,
        ImageType, ImageUsageFlags, ImageViewCreateInfo, ImageViewType, Offset3D,
        PipelineStageFlags2, SampleCountFlags, SharingMode, SubmitInfo2,
    },
    gpu_alloc::UsageFlags,
    isnt::std_1::primitive::IsntSliceExt,
    std::{cell::Cell, ptr, rc::Rc, slice},
};

pub struct VulkanShmImage {
    pub(super) size: DeviceSize,
    pub(super) stride: u32,
    pub(super) _allocation: VulkanAllocation,
    pub(super) shm_info: &'static FormatShmInfo,
    pub(super) async_data: Option<VulkanShmImageAsyncData>,
}

impl VulkanShmImage {
    pub fn upload(
        &self,
        img: &Rc<VulkanImage>,
        buffer: &[Cell<u8>],
        damage: Option<&[Rect]>,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;
        if let Some(damage) = damage {
            if damage.is_empty() {
                return Ok(());
            }
        }
        let copy = |full: bool, off, x, y, width, height| {
            let mut builder = BufferImageCopy2::default()
                .buffer_offset(off)
                .image_offset(Offset3D { x, y, z: 0 })
                .image_extent(Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .image_subresource(ImageSubresourceLayers {
                    aspect_mask: ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            if full {
                builder = builder
                    .buffer_image_height(img.height)
                    .buffer_row_length(img.stride / self.shm_info.bpp);
            }
            builder
        };
        let mut total_size;
        let cpy_one;
        let mut cpy_many;
        let cpy;
        if let Some(damage) = damage {
            total_size = 0;
            cpy_many = Vec::with_capacity(damage.len());
            for damage in damage {
                let Some(damage) = Rect::new(
                    damage.x1().max(0),
                    damage.y1().max(0),
                    damage.x2().min(img.width as i32),
                    damage.y2().min(img.height as i32),
                ) else {
                    continue;
                };
                if damage.is_empty() {
                    continue;
                }
                cpy_many.push(copy(
                    false,
                    total_size as DeviceSize,
                    damage.x1(),
                    damage.y1(),
                    damage.width() as u32,
                    damage.height() as u32,
                ));
                total_size += damage.width() as u32 * damage.height() as u32 * self.shm_info.bpp;
            }
            cpy = &cpy_many[..];
        } else {
            cpy_one = copy(true, 0, 0, 0, img.width, img.height);
            cpy = slice::from_ref(&cpy_one);
            total_size = img.height * img.stride;
        }
        let staging = img.renderer.device.create_staging_buffer(
            &img.renderer.allocator,
            total_size as u64,
            true,
            false,
            true,
        )?;
        staging.upload(|mem, _| unsafe {
            let buf = buffer.as_ptr() as *const u8;
            if damage.is_some() {
                let mut off = 0;
                for cpy in cpy {
                    let x = cpy.image_offset.x as usize;
                    let y = cpy.image_offset.y as usize;
                    let width = cpy.image_extent.width as usize;
                    let height = cpy.image_extent.height as usize;
                    let stride = self.stride as usize;
                    let bpp = self.shm_info.bpp as usize;
                    for dy in 0..height {
                        let lo = (y + dy) * stride + x * bpp;
                        let len = width * bpp;
                        ptr::copy_nonoverlapping(buf.add(lo), mem.add(off), len);
                        off += len;
                    }
                }
            } else {
                ptr::copy_nonoverlapping(buf, mem, total_size as usize);
            }
        })?;
        let Some((cmd, fence, sync_file, point)) =
            self.submit_buffer_image_copy(img, &staging, cpy, false, TransferType::Upload)?
        else {
            return Ok(());
        };
        let future = img.renderer.eng.spawn(
            "await upload",
            await_upload(point, img.clone(), cmd, sync_file, fence, staging),
        );
        img.renderer.pending_submits.set(point, future);
        Ok(())
    }

    pub(super) fn submit_buffer_image_copy(
        &self,
        img: &Rc<VulkanImage>,
        staging: &VulkanStagingBuffer,
        regions: &[BufferImageCopy2],
        use_transfer_queue: bool,
        tt: TransferType,
    ) -> Result<Option<(Rc<VulkanCommandBuffer>, Rc<VulkanFence>, SyncFile, u64)>, VulkanError>
    {
        let memory_barrier = |sam, ssm, dam, dsm| {
            BufferMemoryBarrier2::default()
                .buffer(staging.buffer)
                .offset(0)
                .size(staging.size)
                .src_access_mask(sam)
                .src_stage_mask(ssm)
                .dst_access_mask(dam)
                .dst_stage_mask(dsm)
        };
        let mut transfer_queue_family_idx = img.renderer.device.graphics_queue_idx;
        if use_transfer_queue {
            if let Some(idx) = img.renderer.device.distinct_transfer_queue_family_idx {
                transfer_queue_family_idx = idx;
            }
        }
        let mut initial_image_barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(img.renderer.device.graphics_queue_idx)
            .dst_queue_family_index(transfer_queue_family_idx)
            .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER)
            .old_layout(if img.is_undefined.get() {
                ImageLayout::UNDEFINED
            } else {
                match tt {
                    TransferType::Upload => ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    TransferType::Download => ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                }
            })
            .new_layout(match tt {
                TransferType::Upload => ImageLayout::TRANSFER_DST_OPTIMAL,
                TransferType::Download => ImageLayout::TRANSFER_SRC_OPTIMAL,
            });
        if transfer_queue_family_idx == img.renderer.device.graphics_queue_idx {
            initial_image_barrier = initial_image_barrier
                .src_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
        }
        let initial_buffer_barrier = memory_barrier(
            match tt {
                TransferType::Upload => AccessFlags2::HOST_WRITE,
                TransferType::Download => AccessFlags2::HOST_READ,
            },
            PipelineStageFlags2::HOST,
            match tt {
                TransferType::Upload => AccessFlags2::TRANSFER_READ,
                TransferType::Download => AccessFlags2::TRANSFER_WRITE,
            },
            PipelineStageFlags2::TRANSFER,
        );
        let initial_dep_info = DependencyInfoKHR::default()
            .buffer_memory_barriers(slice::from_ref(&initial_buffer_barrier))
            .image_memory_barriers(slice::from_ref(&initial_image_barrier));
        let mut final_image_barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(transfer_queue_family_idx)
            .dst_queue_family_index(img.renderer.device.graphics_queue_idx)
            .src_access_mask(match tt {
                TransferType::Upload => AccessFlags2::TRANSFER_WRITE,
                TransferType::Download => AccessFlags2::TRANSFER_READ,
            })
            .src_stage_mask(PipelineStageFlags2::TRANSFER)
            .old_layout(match tt {
                TransferType::Upload => ImageLayout::TRANSFER_DST_OPTIMAL,
                TransferType::Download => ImageLayout::TRANSFER_SRC_OPTIMAL,
            })
            .new_layout(match tt {
                TransferType::Upload => ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                TransferType::Download => ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            });
        if transfer_queue_family_idx == img.renderer.device.graphics_queue_idx {
            final_image_barrier = final_image_barrier
                .dst_access_mask(match tt {
                    TransferType::Upload => AccessFlags2::SHADER_SAMPLED_READ,
                    TransferType::Download => AccessFlags2::COLOR_ATTACHMENT_WRITE,
                })
                .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER);
        }
        let final_buffer_barrier = memory_barrier(
            match tt {
                TransferType::Upload => AccessFlags2::TRANSFER_READ,
                TransferType::Download => AccessFlags2::TRANSFER_WRITE,
            },
            PipelineStageFlags2::TRANSFER,
            match tt {
                TransferType::Upload => AccessFlags2::HOST_WRITE,
                TransferType::Download => AccessFlags2::HOST_READ,
            },
            PipelineStageFlags2::HOST,
        );
        let final_dep_info = DependencyInfoKHR::default()
            .buffer_memory_barriers(slice::from_ref(&final_buffer_barrier))
            .image_memory_barriers(slice::from_ref(&final_image_barrier));
        let cmd = match &img.renderer.transfer_command_buffers {
            Some(b) if use_transfer_queue => b.allocate()?,
            _ => img.renderer.gfx_command_buffers.allocate()?,
        };
        let dev = &img.renderer.device.device;
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(cmd.buffer);
        let submit_info =
            SubmitInfo2::default().command_buffer_infos(slice::from_ref(&command_buffer_info));
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let release_fence = img.renderer.device.create_fence()?;
        unsafe {
            dev.begin_command_buffer(cmd.buffer, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)?;
            dev.cmd_pipeline_barrier2(cmd.buffer, &initial_dep_info);
            match tt {
                TransferType::Upload => {
                    let cpy_info = CopyBufferToImageInfo2::default()
                        .src_buffer(staging.buffer)
                        .dst_image(img.image)
                        .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                        .regions(regions);
                    dev.cmd_copy_buffer_to_image2(cmd.buffer, &cpy_info);
                }
                TransferType::Download => {
                    let cpy_info = CopyImageToBufferInfo2::default()
                        .dst_buffer(staging.buffer)
                        .src_image(img.image)
                        .src_image_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .regions(regions);
                    dev.cmd_copy_image_to_buffer2(cmd.buffer, &cpy_info);
                }
            }
            dev.cmd_pipeline_barrier2(cmd.buffer, &final_dep_info);
            dev.end_command_buffer(cmd.buffer)
                .map_err(VulkanError::EndCommandBuffer)?;
            dev.queue_submit2(
                match img.renderer.device.transfer_queue {
                    Some(q) if use_transfer_queue => q,
                    _ => img.renderer.device.graphics_queue,
                },
                slice::from_ref(&submit_info),
                release_fence.fence,
            )
            .map_err(VulkanError::Submit)?;
        }
        if tt == TransferType::Upload {
            img.is_undefined.set(false);
            img.contents_are_undefined.set(false);
        }
        let release_sync_file = match release_fence.export_sync_file() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not export sync file from fence: {}", ErrorFmt(e));
                img.renderer.block();
                return Ok(None);
            }
        };
        let point = img.renderer.allocate_point();
        Ok(Some((cmd, release_fence, release_sync_file, point)))
    }
}

async fn await_upload(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Rc<VulkanCommandBuffer>,
    sync_file: SyncFile,
    _fence: Rc<VulkanFence>,
    _staging: VulkanStagingBuffer,
) {
    let res = img.renderer.ring.readable(&sync_file.0).await;
    if let Err(e) = res {
        log::error!(
            "Could not wait for sync file to become readable: {}",
            ErrorFmt(e)
        );
        img.renderer.block();
    }
    img.renderer.gfx_command_buffers.buffers.push(buf);
    img.renderer.pending_submits.remove(&id);
}

impl VulkanRenderer {
    pub fn create_shm_texture(
        self: &Rc<Self>,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        data: &[Cell<u8>],
        for_download: bool,
        cpu_worker: Option<&Rc<CpuWorker>>,
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(VulkanError::UnsupportedShmFormat(format.name));
        };
        if width <= 0 || height <= 0 || stride <= 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = width as u32;
        let height = height as u32;
        let stride = stride as u32;
        if stride % shm_info.bpp != 0 || stride / shm_info.bpp < width {
            return Err(VulkanError::InvalidStride);
        }
        let vk_format = self
            .device
            .formats
            .get(&format.drm)
            .ok_or(VulkanError::FormatNotSupported)?;
        let shm = vk_format.shm.as_ref().ok_or(VulkanError::ShmNotSupported)?;
        if width > shm.limits.max_width || height > shm.limits.max_height {
            return Err(VulkanError::ImageTooLarge);
        }
        let size = stride.checked_mul(height).ok_or(VulkanError::ShmOverflow)? as u64;
        let usage = ImageUsageFlags::TRANSFER_SRC
            | match for_download {
                true => ImageUsageFlags::COLOR_ATTACHMENT,
                false => ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
            };
        let create_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .format(format.vk_format)
            .mip_levels(1)
            .array_layers(1)
            .tiling(ImageTiling::OPTIMAL)
            .samples(SampleCountFlags::TYPE_1)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .initial_layout(ImageLayout::UNDEFINED)
            .extent(Extent3D {
                width,
                height,
                depth: 1,
            })
            .usage(usage);
        let image = unsafe { self.device.device.create_image(&create_info, None) };
        let image = image.map_err(VulkanError::CreateImage)?;
        let destroy_image = OnDrop(|| unsafe { self.device.device.destroy_image(image, None) });
        let memory_requirements =
            unsafe { self.device.device.get_image_memory_requirements(image) };
        let allocation =
            self.allocator
                .alloc(&memory_requirements, UsageFlags::FAST_DEVICE_ACCESS, false)?;
        let res = unsafe {
            self.device
                .device
                .bind_image_memory(image, allocation.memory, allocation.offset)
        };
        res.map_err(VulkanError::BindImageMemory)?;
        let image_view_create_info = ImageViewCreateInfo::default()
            .image(image)
            .format(format.vk_format)
            .view_type(ImageViewType::TYPE_2D)
            .subresource_range(ImageSubresourceRange {
                aspect_mask: ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe {
            self.device
                .device
                .create_image_view(&image_view_create_info, None)
        };
        let view = view.map_err(VulkanError::CreateImageView)?;
        let mut async_data = None;
        if let Some(cpu) = cpu_worker {
            async_data = Some(VulkanShmImageAsyncData {
                busy: Cell::new(false),
                io_job: Default::default(),
                copy_job: Default::default(),
                staging: Default::default(),
                client_mem: Default::default(),
                callback: Default::default(),
                callback_id: Cell::new(0),
                regions: Default::default(),
                cpu: cpu.clone(),
                last_sample: Default::default(),
                data_copied: Default::default(),
            });
        }
        let shm = VulkanShmImage {
            size,
            stride,
            _allocation: allocation,
            shm_info,
            async_data,
        };
        destroy_image.forget();
        let img = Rc::new(VulkanImage {
            renderer: self.clone(),
            format,
            width,
            height,
            stride,
            texture_view: view,
            render_view: None,
            image,
            is_undefined: Cell::new(true),
            contents_are_undefined: Cell::new(true),
            queue_state: Cell::new(QueueState::Acquired {
                family: QueueFamily::Gfx,
            }),
            ty: VulkanImageMemory::Internal(shm),
            bridge: None,
        });
        let shm = match &img.ty {
            VulkanImageMemory::DmaBuf(_) => unreachable!(),
            VulkanImageMemory::Internal(s) => s,
        };
        if data.is_not_empty() {
            shm.upload(&img, data, None)?;
        }
        Ok(img)
    }
}
