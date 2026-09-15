mod device;
mod error;
mod frame;
mod handle;
mod queue;
pub mod resources;
mod swapchain;
mod sync;
mod thread;
mod version;

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    ffi::CString,
    sync::Mutex,
};

use ash::vk::{
    ApplicationInfo, BufferCreateInfo, BufferUsageFlags, CommandBufferBeginInfo,
    CommandBufferUsageFlags, ComputePipelineCreateInfo, DescriptorPoolResetFlags,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType, DeviceCreateInfo,
    DeviceQueueCreateInfo, InstanceCreateInfo, MemoryHeapFlags, PhysicalDeviceFeatures2,
    PhysicalDeviceMemoryProperties2, PhysicalDeviceProperties2, PhysicalDeviceType,
    PipelineBindPoint, PipelineCache, PipelineLayoutCreateInfo, PipelineShaderStageCreateInfo,
    QueueFamilyProperties2, QueueFlags, ShaderModuleCreateInfo, ShaderStageFlags, StructureType,
    SubmitInfo, SurfaceKHR, SwapchainKHR,
};
use renkrs::RGB;
use vk_mem::Alloc;

use crate::{
    HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version,
    graphics_device::{Feature, GraphicsDevice},
    renderers::{
        BufferUsage, FeatureRequest, InitializeOptions, PipelineHandle, Renderer, RendererError,
        RendererResult, ResourceBinding, ResourceBindingType, Settings,
        thread_context::{ThreadContextIndex, ThreadContextMask},
        version::DriverVersion,
        vulkan::{
            device::DeviceContext,
            frame::Frame,
            queue::{QueueContext, QueueType},
            resources::*,
            swapchain::SwapchainContext,
            version::VulkanApiVersion,
        },
    },
    shader::ShaderSource,
};

thread_local! {
    /// Index of the current thread context.
    static THREAD_CONTEXT_INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
}

/// The Vulkan implementation of the `Renderer` trait.
pub struct VulkanRenderer {
    settings: Settings,
    current_frame_index: u32,

    entry: Option<ash::Entry>,
    instance: Option<ash::Instance>,
    latest_api_version: Cell<Option<Version>>,
    api_version: Version,

    window_surface: Option<SurfaceKHR>,
    window_surface_loader: Option<ash::khr::surface::Instance>,

    device_context: Option<DeviceContext>,

    main_thread_id: std::thread::ThreadId,
}

impl Renderer for VulkanRenderer {
    type ShaderHandle = VulkanShader;
    type BufferHandle = VulkanBuffer;
    type GraphicsPipelineHandle = VulkanGraphicsPipeline;
    type ComputePipelineHandle = VulkanComputePipeline;
    type ResourceSetHandle = VulkanResourceSet;
    type RecordedCommand = VulkanRecordedCommand;

    const MIN_SUPPORTED_API_VERSION: Version = Version::new(1, 0, 0);

    fn new() -> Self {
        Self {
            settings: Settings::default(),
            current_frame_index: 0,

            entry: None,
            instance: None,
            latest_api_version: Cell::new(None),
            api_version: Self::MIN_SUPPORTED_API_VERSION,

            window_surface: None,
            window_surface_loader: None,

            device_context: None,

            main_thread_id: std::thread::current().id(),
        }
    }

    fn latest_api_version(&self) -> RendererResult<Version> {
        if let Some(latest_api_version) = self.latest_api_version.get() {
            // Use the cached version.
            return Ok(latest_api_version);
        }

        let enumerate_instance_version = |entry: &ash::Entry| -> RendererResult<Version> {
            Ok(match unsafe { entry.try_enumerate_instance_version()? } {
                Some(v) => VulkanApiVersion(v).into(),
                None => Self::MIN_SUPPORTED_API_VERSION,
            })
        };

        // Cache the latest api version.
        let latest_api_version = if let Some(entry) = &self.entry {
            enumerate_instance_version(entry)?
        } else {
            unsafe { enumerate_instance_version(&ash::Entry::load()?)? }
        };
        self.latest_api_version.set(Some(latest_api_version));
        Ok(latest_api_version)
    }

    fn get_settings(&self) -> &Settings {
        &self.settings
    }

    fn set_settings(&mut self, settings: Settings) -> RendererResult<()> {
        self.main_thread_only()?;

        if self.device_context.is_some() {
            let temp_settings = self.settings;
            let temp_current_frame_index = self.current_frame_index;

            self.destroy_swapchain()?;
            self.uninitialize_thread()?;
            self.destroy_frame_sync()?;

            self.settings = settings;
            self.current_frame_index = 0;

            let device_context = self.device_context.as_mut().unwrap();
            let mut resize_frames = |queue_context: &mut QueueContext| -> RendererResult<()> {
                let masks = device_context.thread_context_masks.lock()?;

                for mask in masks.iter() {
                    if *mask != 0 {
                        self.settings = temp_settings;
                        self.current_frame_index = temp_current_frame_index;
                        return Err(RendererError::invalid_operation(
                            "All worker threads must be uninitialized before changing the settings.",
                        ));
                    }
                }
                queue_context
                    .frames
                    .resize_with(self.settings.frames_in_flight as usize, Frame::default);
                Ok(())
            };

            resize_frames(&mut device_context.graphics_queue_context)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                resize_frames(transfer_queue_context)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                resize_frames(compute_queue_context)?;
            }
            self.create_frame_sync()?;
            self.initialize_thread()?;
            self.create_swapchain()?;
        } else {
            self.settings = settings;
            self.current_frame_index = 0;
        }

        Ok(())
    }

    fn initialize(&mut self, options: &InitializeOptions) -> RendererResult<()> {
        self.main_thread_only()?;
        if self.entry.is_some() || self.instance.is_some() {
            return Err(RendererError::invalid_operation(
                "VulkanRenderer is already initialized",
            ));
        }

        // Create instance.

        let entry = unsafe { ash::Entry::load()? };
        let requested_api_version = match options.api_version {
            Some(v) => v,
            None => self.latest_api_version()?,
        };
        if !self.is_api_version_supported(requested_api_version)? {
            return Err(RendererError::InvalidArgument(format!(
                "Requested Vulkan API version is not supported '{}'.",
                requested_api_version
            )));
        }

        let c_app_name = CString::new(options.app_name)?;
        self.api_version = requested_api_version;
        let app_info = ApplicationInfo {
            s_type: StructureType::APPLICATION_INFO,
            p_engine_name: HEPHGL_ENGINE_NAME.as_ptr(),
            p_application_name: c_app_name.as_ptr(),
            application_version: ash::vk::make_api_version(
                0,
                HEPHGL_ENGINE_VERSION.major,
                HEPHGL_ENGINE_VERSION.minor,
                HEPHGL_ENGINE_VERSION.patch,
            ),
            api_version: VulkanApiVersion::from(self.api_version).0,
            ..Default::default()
        };

        let create_flags = if cfg!(target_os = "macos") {
            ash::vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            ash::vk::InstanceCreateFlags::empty()
        };

        let mut required_extension_names =
            ash_window::enumerate_required_extensions(options.display_handle)?.to_vec();
        if cfg!(target_os = "macos") {
            required_extension_names.push(c"VK_KHR_portability_enumeration".as_ptr());
        }

        let available_layers: Vec<_> = unsafe {
            entry
                .enumerate_instance_layer_properties()
                .unwrap_or_default()
                .into_iter()
                .map(|properties| {
                    std::ffi::CStr::from_ptr(properties.layer_name.as_ptr()).to_owned()
                })
                .collect()
        };
        let validation_layer_name = std::ffi::CString::new("VK_LAYER_KHRONOS_validation").unwrap();
        let mut enabled_layer_names = Vec::new();
        if available_layers.contains(&validation_layer_name) {
            enabled_layer_names.push(validation_layer_name.as_ptr());
        }

        let instance_create_info = InstanceCreateInfo {
            s_type: StructureType::INSTANCE_CREATE_INFO,
            p_application_info: &app_info,
            flags: create_flags,
            enabled_extension_count: required_extension_names.len() as u32,
            pp_enabled_extension_names: required_extension_names.as_ptr(),
            enabled_layer_count: enabled_layer_names.len() as u32,
            pp_enabled_layer_names: enabled_layer_names.as_ptr(),
            ..Default::default()
        };

        let instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        // WSI for rendering to the native window.

        self.window_surface = Some(unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                options.display_handle,
                options.window_handle,
                None,
            )?
        });
        self.window_surface_loader = Some(ash::khr::surface::Instance::new(&entry, &instance));

        self.entry = Some(entry);
        self.instance = Some(instance);

        Ok(())
    }

    fn uninitialize(&mut self) -> RendererResult<()> {
        self.main_thread_only()?;
        self.uninitialize_device()?;

        if let (Some(surface), Some(surface_loader)) =
            (self.window_surface, self.window_surface_loader.as_ref())
        {
            unsafe {
                surface_loader.destroy_surface(surface, None);
            }
        }

        if let Some(instance) = self.instance.as_ref() {
            unsafe {
                instance.destroy_instance(None);
            }
        }

        self.window_surface_loader = None;
        self.window_surface = None;
        self.instance = None;
        self.entry = None;

        Ok(())
    }

    fn enumerate_devices(&self) -> RendererResult<Vec<GraphicsDevice>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(RendererError::invalid_operation(
                "Renderer is not initialized",
            ))?;

        let mut devices = Vec::<GraphicsDevice>::new();
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };

        for physical_device in physical_devices {
            let mut properties2 = PhysicalDeviceProperties2::default();
            let mut memory_properties2 = PhysicalDeviceMemoryProperties2::default();
            let mut physical_features2 = PhysicalDeviceFeatures2::default();
            let mut queue_family_properties2_vec = Vec::<QueueFamilyProperties2>::default();
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut properties2);
                instance.get_physical_device_memory_properties2(
                    physical_device,
                    &mut memory_properties2,
                );
                instance.get_physical_device_features2(physical_device, &mut physical_features2);

                let queue_family_properties2_vec_size =
                    instance.get_physical_device_queue_family_properties2_len(physical_device);
                queue_family_properties2_vec.resize(
                    queue_family_properties2_vec_size,
                    QueueFamilyProperties2::default(),
                );
                instance.get_physical_device_queue_family_properties2(
                    physical_device,
                    &mut queue_family_properties2_vec,
                );
            };

            let device_name = unsafe {
                std::ffi::CStr::from_ptr(properties2.properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };

            let device_type = match properties2.properties.device_type {
                PhysicalDeviceType::DISCRETE_GPU => crate::graphics_device::Type::DiscreteGpu,
                PhysicalDeviceType::INTEGRATED_GPU => crate::graphics_device::Type::IntegratedGpu,
                PhysicalDeviceType::VIRTUAL_GPU => crate::graphics_device::Type::VirtualGpu,
                PhysicalDeviceType::CPU => crate::graphics_device::Type::Cpu,
                PhysicalDeviceType::OTHER => crate::graphics_device::Type::Other,
                _ => crate::graphics_device::Type::Invalid,
            };

            let device_vendor_id = properties2.properties.vendor_id;
            let device_id = properties2.properties.device_id;

            let device_api_version = VulkanApiVersion(properties2.properties.api_version).into();
            let device_driver_version = DriverVersion::new(
                properties2.properties.driver_version,
                GraphicsDevice::vendor_from_id(properties2.properties.vendor_id),
            )
            .into();

            // VRAM is the sum of the sizes of all DEVICE_LOCAL heaps
            let mut device_vram: u64 = 0;
            let heap_count = memory_properties2.memory_properties.memory_heap_count as usize;
            for heap in memory_properties2.memory_properties.memory_heaps[..heap_count].iter() {
                if heap.flags.contains(MemoryHeapFlags::DEVICE_LOCAL) {
                    device_vram += heap.size;
                }
            }

            let mut supported_features = HashSet::<crate::graphics_device::Feature>::default();
            let extension_properties =
                unsafe { instance.enumerate_device_extension_properties(physical_device)? };
            if physical_features2.features.geometry_shader == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::GeometryShaders);
            }
            if physical_features2.features.fill_mode_non_solid == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WireframeMode);
            }
            if physical_features2.features.wide_lines == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WideLines);
            }
            if physical_features2.features.sampler_anisotropy == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::AnisotropicFiltering);
            }
            for ext in extension_properties {
                let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
                if name.to_string_lossy() == "VK_KHR_ray_tracing_pipeline" {
                    supported_features.insert(crate::graphics_device::Feature::RayTracing);
                }
            }
            for queue_family_properties2 in &queue_family_properties2_vec {
                let queue_flags = queue_family_properties2.queue_family_properties.queue_flags;
                if queue_flags.contains(QueueFlags::COMPUTE) {
                    supported_features.insert(crate::graphics_device::Feature::ComputeShaders);
                }
                if queue_flags.contains(QueueFlags::VIDEO_DECODE_KHR) {
                    supported_features.insert(crate::graphics_device::Feature::VideoDecoding);
                }
                if queue_flags.contains(QueueFlags::VIDEO_ENCODE_KHR) {
                    supported_features.insert(crate::graphics_device::Feature::VideoEncoding);
                }
                if queue_flags.contains(QueueFlags::OPTICAL_FLOW_NV) {
                    supported_features.insert(crate::graphics_device::Feature::OpticalFlow);
                }
                // Vulkan guarantees that the main graphics family will also support
                // TRANSFER. Thus, we only consider families that support TRANSFER
                // but do not support GRAPHICS to be dedicated async (DMA) transfer queues.
                if queue_flags.contains(QueueFlags::TRANSFER)
                    && !queue_flags.contains(QueueFlags::GRAPHICS)
                {
                    supported_features.insert(crate::graphics_device::Feature::AsyncTransfer);
                }
            }

            devices.push(GraphicsDevice {
                name: device_name,
                device_type,
                vendor_id: device_vendor_id,
                device_id,
                api_version: device_api_version,
                driver_version: device_driver_version,
                vram: device_vram,
                supported_features,
            });
        }

        Ok(devices)
    }

    fn get_device(&self) -> Option<&GraphicsDevice> {
        self.device_context
            .as_ref()
            .map(|device_context| &device_context.graphics_device)
    }

    fn set_device(
        &mut self,
        device: Option<&GraphicsDevice>,
        requested_features: &[FeatureRequest],
    ) -> RendererResult<()> {
        self.main_thread_only()?;
        if self.device_context.is_some() {
            self.uninitialize_device()?;
        }

        let devices;
        let device = match device {
            Some(device) => device,
            None => {
                devices = self.enumerate_devices()?;
                devices
                    .first()
                    .ok_or(RendererError::fail("No active graphics device found."))?
            }
        };

        let instance = self
            .instance
            .as_ref()
            .ok_or(RendererError::invalid_operation(
                "Renderer is not initialized",
            ))?;

        // Create logical device and queues.

        let mut available_features = Vec::<Feature>::default();
        for requested_feature in requested_features {
            if device
                .supported_features
                .contains(&requested_feature.feature)
            {
                available_features.push(requested_feature.feature);
            } else if requested_feature.required {
                return Err(RendererError::UnsupportedRequiredFeature(
                    requested_feature.feature,
                ));
            }
        }

        let queue_families = self.get_device_queue_families(device)?;
        let mut queue_family_queue_counts: HashMap<u32, u32> = HashMap::new();
        let mut request_queue = |family_index: u32, max_queues: u32| {
            let count = queue_family_queue_counts.entry(family_index).or_insert(0);
            if *count < max_queues {
                *count += 1;
            }
        };

        let graphics_family = queue_families
            .iter()
            .find(|f| f.queue_flags.contains(QueueFlags::GRAPHICS) && f.present_supported)
            .ok_or_else(|| {
                RendererError::invalid_operation(
                    "No queue family supports both graphics and presentation.",
                )
            })?;
        request_queue(graphics_family.index, graphics_family.queue_count);

        // Use DMA Transfer family if available. Otherwise, use the main graphics
        // family.
        let mut transfer_family = None;
        if available_features.contains(&Feature::AsyncTransfer) {
            let transfer_family = transfer_family.insert(
                queue_families
                    .iter()
                    .find(|f| {
                        f.queue_flags.contains(QueueFlags::TRANSFER)
                            && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                            && !f.queue_flags.contains(QueueFlags::COMPUTE)
                    })
                    .or_else(|| {
                        queue_families.iter().find(|f| {
                            f.queue_flags.contains(QueueFlags::TRANSFER)
                                && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                        })
                    })
                    .unwrap_or(graphics_family),
            );
            request_queue(transfer_family.index, transfer_family.queue_count);
        }

        // Use the pure compute family if available. Otherwise, use the main graphics
        // family.
        let mut compute_family = None;
        if available_features.contains(&Feature::ComputeShaders) {
            let compute_family = compute_family.insert(
                queue_families
                    .iter()
                    .find(|f| {
                        f.queue_flags.contains(QueueFlags::COMPUTE)
                            && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                    })
                    .unwrap_or(graphics_family),
            );
            request_queue(compute_family.index, compute_family.queue_count);
        }

        let queue_setup_data: Vec<(u32, Vec<f32>)> = queue_family_queue_counts
            .into_iter()
            .map(|(index, count)| (index, vec![1.0; count as usize]))
            .collect();
        let mut queue_create_infos = Vec::new();
        for (family_index, priorities) in &queue_setup_data {
            let create_info = DeviceQueueCreateInfo::default()
                .queue_family_index(*family_index)
                .queue_priorities(priorities);
            queue_create_infos.push(create_info);
        }

        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        let mut physical_device = None;
        let mut physical_device_api_version = Self::MIN_SUPPORTED_API_VERSION;
        for pd in physical_devices {
            let mut properties2 = PhysicalDeviceProperties2::default();
            unsafe {
                instance.get_physical_device_properties2(pd, &mut properties2);
            }
            if properties2.properties.device_id == device.device_id {
                physical_device = Some(pd);
                physical_device_api_version =
                    VulkanApiVersion(properties2.properties.api_version).into();
                break;
            }
        }
        if !self.is_api_version_supported(physical_device_api_version)? {
            return Err(RendererError::InvalidArgument(format!(
                "Device Vulkan API version is not supported '{}'.",
                physical_device_api_version
            )));
        }
        let physical_device = physical_device.ok_or_else(|| {
            RendererError::InvalidArgument(format!(
                "Graphics device with the id {:#06X} not found.",
                device.device_id
            ))
        })?;

        let device_extension_names = [ash::vk::KHR_SWAPCHAIN_NAME.as_ptr()];
        let mut physical_features2 = PhysicalDeviceFeatures2::default();
        if available_features.contains(&Feature::GeometryShaders) {
            physical_features2.features.geometry_shader = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::WireframeMode) {
            physical_features2.features.fill_mode_non_solid = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::WideLines) {
            physical_features2.features.wide_lines = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::AnisotropicFiltering) {
            physical_features2.features.sampler_anisotropy = ash::vk::TRUE;
        }

        let mut supports_timeline_semaphore = false;

        let mut vulkan_12_features = ash::vk::PhysicalDeviceVulkan12Features::default();
        let mut vulkan_13_features = ash::vk::PhysicalDeviceVulkan13Features::default();
        let mut device_create_info = DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names);
        const V12: Version = Version::new(1, 2, 0);
        if self.api_version >= V12 && physical_device_api_version >= V12 {
            let mut features =
                ash::vk::PhysicalDeviceFeatures2::default().push_next(&mut vulkan_12_features);
            unsafe {
                instance.get_physical_device_features2(physical_device, &mut features);
            }
            supports_timeline_semaphore = vulkan_12_features.timeline_semaphore == ash::vk::TRUE;

            vulkan_12_features = ash::vk::PhysicalDeviceVulkan12Features::default();
            vulkan_12_features.timeline_semaphore = if supports_timeline_semaphore {
                ash::vk::TRUE
            } else {
                ash::vk::FALSE
            };
            device_create_info = device_create_info.push_next(&mut vulkan_12_features);
        }
        if self.api_version >= Version::new(1, 3, 0) {
            let mut features =
                ash::vk::PhysicalDeviceFeatures2::default().push_next(&mut vulkan_13_features);
            unsafe {
                instance.get_physical_device_features2(physical_device, &mut features);
            }
            // TODO: Set `supports_dynamic_rendering_semaphore`.

            vulkan_13_features = ash::vk::PhysicalDeviceVulkan13Features::default();
            device_create_info = device_create_info.push_next(&mut vulkan_13_features);
        }
        device_create_info = device_create_info.push_next(&mut physical_features2);
        let logical_device =
            unsafe { instance.create_device(physical_device, &device_create_info, None)? };

        let mut extracted_queue_counts: HashMap<u32, u32> = HashMap::new();
        let mut get_next_queue = |family_index: u32, max_queues: u32| {
            let queue_index = extracted_queue_counts.entry(family_index).or_insert(0);
            let queue = unsafe { logical_device.get_device_queue(family_index, *queue_index) };
            *queue_index = (*queue_index + 1) % max_queues;
            queue
        };

        let graphics_queue = get_next_queue(graphics_family.index, graphics_family.queue_count);

        let mut transfer_queue_handle = None;
        if let Some(transfer_family) = transfer_family {
            transfer_queue_handle = Some(get_next_queue(
                transfer_family.index,
                transfer_family.queue_count,
            ));
        }

        let mut compute_queue_handle = None;
        if let Some(compute_family) = compute_family {
            compute_queue_handle = Some(get_next_queue(
                compute_family.index,
                compute_family.queue_count,
            ));
        }

        // Initialize VMA.

        let mut allocator_create_info =
            vk_mem::AllocatorCreateInfo::new(instance, &logical_device, physical_device);
        allocator_create_info.vulkan_api_version = VulkanApiVersion::from(self.api_version).0;
        let vma_allocator = unsafe { vk_mem::Allocator::new(allocator_create_info)? };

        self.device_context = Some(DeviceContext {
            graphics_device: device.clone(),

            vma_allocator,

            graphics_queue_context: QueueContext {
                queue: graphics_queue,
                queue_type: QueueType::Graphics,
                queue_family_index: graphics_family.index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| Frame::default())
                    .collect::<Vec<Frame>>(),
                frame_sync: None,
            },
            transfer_queue_context: transfer_queue_handle.map(|queue| QueueContext {
                queue,
                queue_type: QueueType::Transfer,
                queue_family_index: transfer_family.unwrap().index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| Frame::default())
                    .collect::<Vec<Frame>>(),
                frame_sync: None,
            }),
            compute_queue_context: compute_queue_handle.map(|queue| QueueContext {
                queue,
                queue_type: QueueType::Compute,
                queue_family_index: compute_family.unwrap().index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| Frame::default())
                    .collect::<Vec<Frame>>(),
                frame_sync: None,
            }),

            swapchain_context: SwapchainContext {
                loader: ash::khr::swapchain::Device::new(instance, &logical_device),
                swapchain: SwapchainKHR::null(),
                format: ash::vk::Format::default(),
                extent: ash::vk::Extent2D::default(),
                images: Vec::default(),
                image_views: Vec::default(),
                semaphores: Vec::default(),
                depth_image: ash::vk::Image::null(),
                depth_image_allocation: None,
                depth_image_view: ash::vk::ImageView::null(),
            },

            physical_device,
            logical_device,
            supports_timeline_semaphore,

            thread_context_masks: Mutex::new(std::array::from_fn(|_| ThreadContextMask::default())),
        });

        self.create_frame_sync()?;
        self.initialize_thread()?;
        self.create_swapchain()?;

        Ok(())
    }

    fn create_shader(&self, source: &ShaderSource) -> RendererResult<Self::ShaderHandle> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let (prefix, code_u32, suffix) = unsafe { source.data.align_to::<u32>() };

        // Data is not aligned properly.
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(RendererError::Fail(format!(
                "Shader data from '{}' is not valid SPIR-V (not 4-byte aligned).",
                source.file_path
            )));
        }

        let create_info = ShaderModuleCreateInfo::default().code(code_u32);
        unsafe {
            let shader_module = device_context
                .logical_device
                .create_shader_module(&create_info, None)?;
            Ok(Self::ShaderHandle {
                module: shader_module,
            })
        }
    }

    fn destroy_shader(&self, shader: &Self::ShaderHandle) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            device_context
                .logical_device
                .destroy_shader_module(shader.module, None);
        }
        Ok(())
    }

    fn create_resource_set(
        &self,
        pipeline_handle: &PipelineHandle<Self::GraphicsPipelineHandle, Self::ComputePipelineHandle>,
        bindings: &[ResourceBinding<Self::BufferHandle>],
    ) -> RendererResult<Self::ResourceSetHandle> {
        match pipeline_handle {
            PipelineHandle::Compute(pipeline) => {
                for binding in bindings {
                    match binding.resource {
                        ResourceBindingType::Buffer {
                            handle,
                            offset,
                            size,
                            ..
                        } => {
                            if (offset + size) > handle.size {
                                return Err(RendererError::invalid_argument(
                                    "Buffer overflow when binding resources.",
                                ));
                            }
                        }
                    }
                }

                let device_context = self
                    .device_context
                    .as_ref()
                    .ok_or(RendererError::invalid_operation("Device is not set."))?;
                let compute_queue_context = device_context.compute_queue_context.as_ref().ok_or(
                    RendererError::invalid_operation(
                        "Device is not initialized with `ComputeShaders` feature.",
                    ),
                )?;
                let thread_context_index = Self::thread_context_index()?;
                let current_frame = &compute_queue_context.frames
                    [self.current_frame_index as usize]
                    .thread_contexts[thread_context_index];
                let descriptor_pool = current_frame.descriptor_pool;

                let alloc_info = ash::vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(std::slice::from_ref(&pipeline.descriptor_layout));

                let descriptor_set = unsafe {
                    device_context
                        .logical_device
                        .allocate_descriptor_sets(&alloc_info)?[0]
                };

                let buffer_infos: Vec<_> = bindings
                    .iter()
                    .map(|binding| match &binding.resource {
                        ResourceBindingType::Buffer {
                            handle,
                            offset,
                            size,
                            ..
                        } => ash::vk::DescriptorBufferInfo::default()
                            .buffer(handle.buffer)
                            .offset(*offset as u64)
                            .range(*size as u64),
                    })
                    .collect();

                let writes: Vec<_> = buffer_infos
                    .iter()
                    .enumerate()
                    .map(|(i, info)| {
                        ash::vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_set)
                            .dst_binding(i as u32)
                            .descriptor_type(ash::vk::DescriptorType::STORAGE_BUFFER)
                            .buffer_info(std::slice::from_ref(info))
                    })
                    .collect();

                unsafe {
                    device_context
                        .logical_device
                        .update_descriptor_sets(&writes, &[]);
                }

                Ok(Self::ResourceSetHandle { descriptor_set })
            }
            _ => todo!(),
        }
    }

    fn create_buffer(&self, size: usize, usage: BufferUsage) -> RendererResult<Self::BufferHandle> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let vk_usage = match usage {
            BufferUsage::Storage => BufferUsageFlags::STORAGE_BUFFER,
            BufferUsage::Uniform => BufferUsageFlags::UNIFORM_BUFFER,
            BufferUsage::Vertex => BufferUsageFlags::VERTEX_BUFFER,
            BufferUsage::Index => BufferUsageFlags::INDEX_BUFFER,
        };

        let buffer_info = BufferCreateInfo::default()
            .size(size as u64)
            .usage(vk_usage);
        let alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::MAPPED
                | vk_mem::AllocationCreateFlags::HOST_ACCESS_RANDOM,
            ..Default::default()
        };
        let (buffer, vma_allocation) = unsafe {
            device_context
                .vma_allocator
                .create_buffer(&buffer_info, &alloc_info)?
        };

        Ok(VulkanBuffer {
            buffer,
            vma_allocation,
            size,
        })
    }

    fn write_buffer(&self, buffer: &Self::BufferHandle, data: &[u8]) -> RendererResult<()> {
        if data.len() > buffer.size {
            return Err(RendererError::fail("Data exceeds buffer size!"));
        }

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                alloc_info.mapped_data as *mut u8,
                data.len(),
            );
        }

        Ok(())
    }

    fn read_buffer(&self, buffer: &Self::BufferHandle, dest: &mut [u8]) -> RendererResult<()> {
        if dest.len() > buffer.size {
            return Err(RendererError::fail(
                "Destination slice is larger than buffer!",
            ));
        }

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                alloc_info.mapped_data as *const u8,
                dest.as_mut_ptr(),
                dest.len(),
            );
        }

        Ok(())
    }

    fn destroy_buffer(&self, buffer: &mut Self::BufferHandle) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            device_context
                .vma_allocator
                .destroy_buffer(buffer.buffer, &mut buffer.vma_allocation);
            buffer.size = 0;
        }
        Ok(())
    }

    fn create_compute_pipeline(
        &self,
        shader: &Self::ShaderHandle,
    ) -> RendererResult<Self::ComputePipelineHandle> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        // TODO: Pass number of bindings as a parameter.
        let bindings = (0..3)
            .map(|i| {
                DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(ShaderStageFlags::COMPUTE)
            })
            .collect::<Vec<_>>();

        let layout_info = DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_layout = unsafe {
            device_context
                .logical_device
                .create_descriptor_set_layout(&layout_info, None)?
        };

        let pipeline_layout_info = PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_layout));
        let layout = unsafe {
            device_context
                .logical_device
                .create_pipeline_layout(&pipeline_layout_info, None)?
        };

        let entry_name = std::ffi::CString::new("main")?;
        let stage_info = PipelineShaderStageCreateInfo::default()
            .stage(ShaderStageFlags::COMPUTE)
            .module(shader.module)
            .name(&entry_name);

        let compute_info = ComputePipelineCreateInfo::default()
            .layout(layout)
            .stage(stage_info);
        let pipeline = unsafe {
            device_context.logical_device.create_compute_pipelines(
                PipelineCache::null(),
                &[compute_info],
                None,
            )?[0]
        };

        Ok(VulkanComputePipeline {
            pipeline,
            layout,
            descriptor_layout,
        })
    }

    fn destroy_compute_pipeline(
        &self,
        pipeline: &Self::ComputePipelineHandle,
    ) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            device_context
                .logical_device
                .destroy_pipeline(pipeline.pipeline, None);
            device_context
                .logical_device
                .destroy_pipeline_layout(pipeline.layout, None);
            device_context
                .logical_device
                .destroy_descriptor_set_layout(pipeline.descriptor_layout, None);
        }
        Ok(())
    }

    fn record_compute_pass(
        &mut self,
        pipeline: &Self::ComputePipelineHandle,
        resource_sets: &[&Self::ResourceSetHandle],
        group_count: (u32, u32, u32),
    ) -> RendererResult<Self::RecordedCommand> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let compute_queue_context = device_context.compute_queue_context.as_mut().ok_or(
            RendererError::invalid_operation(
                "Device is not initialized with `ComputeShaders` feature.",
            ),
        )?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut compute_queue_context.frames[self.current_frame_index as usize]
            .thread_contexts[thread_context_index];

        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        let mapped_sets: Vec<ash::vk::DescriptorSet> =
            resource_sets.iter().map(|set| set.descriptor_set).collect();
        unsafe {
            device_context
                .logical_device
                .begin_command_buffer(current_frame.command_buffer, &begin_info)?;
            device_context.logical_device.cmd_bind_pipeline(
                current_frame.command_buffer,
                PipelineBindPoint::COMPUTE,
                pipeline.pipeline,
            );
            device_context.logical_device.cmd_bind_descriptor_sets(
                current_frame.command_buffer,
                PipelineBindPoint::COMPUTE,
                pipeline.layout,
                0,
                &mapped_sets,
                &[],
            );
            device_context.logical_device.cmd_dispatch(
                current_frame.command_buffer,
                group_count.0,
                group_count.1,
                group_count.2,
            );
            device_context
                .logical_device
                .end_command_buffer(current_frame.command_buffer)?;
        }

        Ok(Self::RecordedCommand {
            queue_type: QueueType::Compute,
            frame_index: self.current_frame_index,
            thread_context_index,
        })
    }

    fn submit_commands(
        &mut self,
        recorded_commands: &[Self::RecordedCommand],
    ) -> RendererResult<()> {
        self.main_thread_only()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let submit_queue = |queue_context: &mut QueueContext| {
            const INVALID_FRAME_INDEX: usize = usize::MAX;
            let mut command_buffers = Vec::with_capacity(recorded_commands.len());
            let mut frame_index = INVALID_FRAME_INDEX;
            for recorded_command in recorded_commands {
                if recorded_command.queue_type != queue_context.queue_type {
                    continue;
                }

                if frame_index == INVALID_FRAME_INDEX {
                    frame_index = recorded_command.frame_index as usize;
                } else if frame_index != recorded_command.frame_index as usize {
                    return Err(RendererError::invalid_argument(
                        "All recorded commands for a queue must belong to the same frame.",
                    ));
                }

                command_buffers.push(
                    queue_context.frames[frame_index].thread_contexts
                        [recorded_command.thread_context_index]
                        .command_buffer,
                );
            }

            if !command_buffers.is_empty() {
                let submit_info = SubmitInfo::default().command_buffers(&command_buffers);
                let frame_sync = queue_context
                    .frame_sync
                    .as_mut()
                    .ok_or(RendererError::invalid_operation("Device is not set."))?;
                frame_sync.submit(
                    &device_context.logical_device,
                    frame_index,
                    queue_context.queue,
                    &[submit_info],
                )?;
            }

            Ok(())
        };

        submit_queue(&mut device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            submit_queue(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            submit_queue(compute_queue_context)?;
        }

        Ok(())
    }

    fn begin_frame(&mut self) -> RendererResult<()> {
        self.main_thread_only()?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame_index = self.current_frame_index as usize;

        // Wait for current frame to finish.
        let wait_frame_sync = |queue_context: &mut QueueContext| -> RendererResult<()> {
            let frame_sync = queue_context
                .frame_sync
                .as_mut()
                .ok_or(RendererError::invalid_operation("Device is not set."))?;
            frame_sync.wait(&device_context.logical_device, current_frame_index)
        };
        wait_frame_sync(&mut device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            wait_frame_sync(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            wait_frame_sync(compute_queue_context)?;
        }

        // Reset the command pools.
        let reset_command_pool = |queue_context: &QueueContext| -> RendererResult<()> {
            unsafe {
                device_context.logical_device.reset_command_pool(
                    queue_context.frames[current_frame_index].thread_contexts[thread_context_index]
                        .command_pool,
                    ash::vk::CommandPoolResetFlags::empty(),
                )?;
                Ok(())
            }
        };
        reset_command_pool(&device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &device_context.transfer_queue_context {
            reset_command_pool(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &device_context.compute_queue_context {
            reset_command_pool(compute_queue_context)?;
        }

        // Reset the descriptor pools.
        let reset_descriptor_pool = |queue_context: &QueueContext| -> RendererResult<()> {
            unsafe {
                device_context.logical_device.reset_descriptor_pool(
                    queue_context.frames[current_frame_index].thread_contexts[thread_context_index]
                        .descriptor_pool,
                    DescriptorPoolResetFlags::empty(),
                )?;
                Ok(())
            }
        };
        reset_descriptor_pool(&device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &device_context.transfer_queue_context {
            reset_descriptor_pool(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &device_context.compute_queue_context {
            reset_descriptor_pool(compute_queue_context)?;
        }

        Ok(())
    }

    fn end_frame(&mut self) -> RendererResult<()> {
        self.main_thread_only()?;

        self.current_frame_index = (self.current_frame_index + 1) % self.settings.frames_in_flight;

        Ok(())
    }

    fn wait_idle(&self) -> RendererResult<()> {
        self.main_thread_only()?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            device_context.logical_device.device_wait_idle()?;
        }
        Ok(())
    }

    fn clear(&mut self, _color: RGB<f32>) -> RendererResult<()> {
        todo!();
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        if let Err(e) = self.uninitialize() {
            eprintln!("Failed to uninitialize renderer on drop: {}", e);
        }
    }
}
