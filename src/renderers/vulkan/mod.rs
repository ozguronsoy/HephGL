mod device;
mod error;
mod frame;
mod handle;
mod queue;
mod rendering;
pub mod resources;
mod settings;
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
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DeviceCreateInfo,
    DeviceQueueCreateInfo, InstanceCreateInfo, MemoryHeapFlags, PhysicalDeviceFeatures2,
    PhysicalDeviceType, PipelineBindPoint, PipelineCache, PipelineLayoutCreateInfo,
    PipelineShaderStageCreateInfo, QueueFlags, ShaderModuleCreateInfo, StructureType, SubmitInfo,
    SurfaceKHR, SwapchainKHR,
};
use renkrs::RGB;
use vk_mem::Alloc;

use crate::{
    HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version,
    graphics_device::{Feature, GraphicsDevice},
    renderers::{
        BufferUsage, FeatureRequest, InitializeOptions, Renderer, RendererError, RendererResult,
        ResourceBinding, Settings,
        resources::ResourceBindingType,
        settings::{GraphicsPipelineOptions, Msaa, SamplerOptions, TextureOptions},
        thread_context::{ThreadContextIndex, ThreadContextMask, is_thread_context_active},
        version::DriverVersion,
        vulkan::{
            device::DeviceContext,
            frame::{Frame, FrameState},
            queue::QueueContext,
            resources::*,
            swapchain::SwapchainContext,
            version::VulkanApiVersion,
        },
    },
    shader::Shader,
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
    type Buffer = VulkanBuffer;
    type Texture = VulkanTexture;
    type Sampler = VulkanSampler;
    type GraphicsPipeline = VulkanGraphicsPipeline;
    type ComputePipeline = VulkanComputePipeline;

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

            self.destroy_rendering()?;
            self.destroy_swapchain()?;
            self.destroy_frame_sync()?;
            self.uninitialize_thread()?;

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
            resize_frames(&mut device_context.transfer_queue_context)?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                resize_frames(compute_queue_context)?;
            }
            self.create_frame_sync()?;
            self.initialize_thread()?;
            self.create_swapchain()?;
            self.create_rendering()?;
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
                "Requested Vulkan API version is not supported (requested: `{}`, min: `{}`, max: `{}`).",
                requested_api_version,
                Self::MIN_SUPPORTED_API_VERSION,
                self.latest_api_version()?
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
            let (properties, memory_properties, physical_features, queue_family_properties_vec) = unsafe {
                (
                    instance.get_physical_device_properties(physical_device),
                    instance.get_physical_device_memory_properties(physical_device),
                    instance.get_physical_device_features(physical_device),
                    instance.get_physical_device_queue_family_properties(physical_device),
                )
            };

            let device_name = unsafe {
                std::ffi::CStr::from_ptr(properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };

            let device_type = match properties.device_type {
                PhysicalDeviceType::DISCRETE_GPU => crate::graphics_device::Type::DiscreteGpu,
                PhysicalDeviceType::INTEGRATED_GPU => crate::graphics_device::Type::IntegratedGpu,
                PhysicalDeviceType::VIRTUAL_GPU => crate::graphics_device::Type::VirtualGpu,
                PhysicalDeviceType::CPU => crate::graphics_device::Type::Cpu,
                PhysicalDeviceType::OTHER => crate::graphics_device::Type::Other,
                _ => crate::graphics_device::Type::Invalid,
            };

            let device_vendor_id = properties.vendor_id;
            let device_id = properties.device_id;

            let device_api_version = VulkanApiVersion(properties.api_version).into();
            let device_driver_version = DriverVersion::new(
                properties.driver_version,
                GraphicsDevice::vendor_from_id(properties.vendor_id),
            )
            .into();

            // VRAM is the sum of the sizes of all DEVICE_LOCAL heaps
            let mut device_vram: u64 = 0;
            let heap_count = memory_properties.memory_heap_count as usize;
            for heap in memory_properties.memory_heaps[..heap_count].iter() {
                if heap.flags.contains(MemoryHeapFlags::DEVICE_LOCAL) {
                    device_vram += heap.size;
                }
            }

            let mut supported_features = HashSet::<crate::graphics_device::Feature>::default();
            let extension_properties =
                unsafe { instance.enumerate_device_extension_properties(physical_device)? };
            if physical_features.geometry_shader == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::GeometryShaders);
            }
            if physical_features.fill_mode_non_solid == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WireframeMode);
            }
            if physical_features.wide_lines == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WideLines);
            }
            if physical_features.sampler_anisotropy == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::AnisotropicFiltering);
            }
            for ext in extension_properties {
                let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
                if name.to_string_lossy() == "VK_KHR_ray_tracing_pipeline" {
                    supported_features.insert(crate::graphics_device::Feature::RayTracing);
                }
            }
            for queue_family_properties in &queue_family_properties_vec {
                let queue_flags = queue_family_properties.queue_flags;
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
                // but do not support GRAPHICS and COMPUTE to be dedicated async (DMA) transfer
                // queues.
                if queue_flags.contains(QueueFlags::TRANSFER)
                    && !queue_flags.contains(QueueFlags::GRAPHICS)
                    && !queue_flags.contains(QueueFlags::COMPUTE)
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
        let transfer_family = if available_features.contains(&Feature::AsyncTransfer) {
            queue_families
                .iter()
                .find(|f| {
                    f.queue_flags.contains(QueueFlags::TRANSFER)
                        && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                        && !f.queue_flags.contains(QueueFlags::COMPUTE)
                })
                .ok_or(RendererError::fail(
                    "Asynchronous transfer is not supported by this device.",
                ))?
        } else {
            graphics_family
        };
        request_queue(transfer_family.index, transfer_family.queue_count);

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
        let mut supported_msaa_list = Vec::default();
        for pd in physical_devices {
            let properties = unsafe { instance.get_physical_device_properties(pd) };
            if properties.device_id == device.device_id {
                physical_device = Some(pd);
                physical_device_api_version = VulkanApiVersion(properties.api_version).into();

                let sample_counts = properties.limits.framebuffer_color_sample_counts
                    & properties.limits.framebuffer_depth_sample_counts;
                const MSAA_ALL: [Msaa; 7] = [
                    Msaa::X1,
                    Msaa::X2,
                    Msaa::X4,
                    Msaa::X8,
                    Msaa::X16,
                    Msaa::X32,
                    Msaa::X64,
                ];
                supported_msaa_list = MSAA_ALL
                    .into_iter()
                    .filter(|msaa| sample_counts.contains((*msaa).into()))
                    .collect::<Vec<_>>();
                break;
            }
        }
        if physical_device_api_version < Self::MIN_SUPPORTED_API_VERSION {
            return Err(RendererError::InvalidArgument(format!(
                "Device Vulkan API version is not supported (device: `{}`, min: `{}`).",
                physical_device_api_version,
                Self::MIN_SUPPORTED_API_VERSION
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
        let mut supports_dynamic_rendering = false;

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
            vulkan_12_features = vulkan_12_features.timeline_semaphore(supports_timeline_semaphore);
            device_create_info = device_create_info.push_next(&mut vulkan_12_features);
        }
        if self.api_version >= Version::new(1, 3, 0) {
            let mut features =
                ash::vk::PhysicalDeviceFeatures2::default().push_next(&mut vulkan_13_features);
            unsafe {
                instance.get_physical_device_features2(physical_device, &mut features);
            }
            supports_dynamic_rendering = vulkan_13_features.dynamic_rendering == ash::vk::TRUE;

            vulkan_13_features = ash::vk::PhysicalDeviceVulkan13Features::default();
            vulkan_13_features = vulkan_13_features.dynamic_rendering(supports_dynamic_rendering);
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
        let transfer_queue = get_next_queue(transfer_family.index, transfer_family.queue_count);
        let compute_queue = compute_family.map(|f| get_next_queue(f.index, f.queue_count));

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
                queue_family_index: graphics_family.index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| Frame::default())
                    .collect::<Vec<Frame>>(),
                frame_sync: None,
            },
            transfer_queue_context: QueueContext {
                queue: transfer_queue,
                queue_family_index: transfer_family.index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| Frame::default())
                    .collect::<Vec<Frame>>(),
                frame_sync: None,
            },
            compute_queue_context: compute_queue.map(|queue| QueueContext {
                queue,
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
                msaa_color_image: ash::vk::Image::null(),
                msaa_color_image_allocation: None,
                msaa_color_image_view: ash::vk::ImageView::null(),
                current_image_index: 0,
                image_avaliable_semaphore_index: SwapchainContext::INVALID_INDEX,
            },
            rendering: None,

            physical_device,
            logical_device,
            supports_timeline_semaphore,
            supports_dynamic_rendering,
            supported_msaa_list,

            thread_context_masks: Mutex::new(std::array::from_fn(|_| ThreadContextMask::default())),
        });

        self.initialize_thread()?;
        self.create_frame_sync()?;
        self.create_swapchain()?;
        self.create_rendering()?;

        Ok(())
    }

    fn supported_msaa_list(&self) -> RendererResult<&Vec<Msaa>> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        Ok(&device_context.supported_msaa_list)
    }

    fn create_buffer(&self, size: usize, usage: BufferUsage) -> RendererResult<Self::Buffer> {
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

    fn write_buffer(&self, buffer: &Self::Buffer, data: &[u8]) -> RendererResult<()> {
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

    fn read_buffer(&self, buffer: &Self::Buffer, dest: &mut [u8]) -> RendererResult<()> {
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

    fn destroy_buffer(&self, buffer: &mut Self::Buffer) -> RendererResult<()> {
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

    fn create_texture(
        &mut self,
        options: &TextureOptions,
        data: &[u8],
    ) -> RendererResult<Self::Texture> {
        if options.width == 0 || options.height == 0 {
            return Err(RendererError::invalid_argument(
                "Texture dimensions must be greater than zero.",
            ));
        }

        let expected_size = (options.width as usize)
            .checked_mul(options.height as usize)
            .and_then(|size| size.checked_mul(options.format.bytes_per_pixel()))
            .ok_or(RendererError::invalid_argument(
                "Texture dimensions are too large.",
            ))?;
        if data.len() != expected_size {
            return Err(RendererError::invalid_argument(format!(
                "Texture data size does not match texture dimensions. Expected {expected_size} bytes, got {}.",
                data.len()
            )));
        }

        if options.mip_level_count == 0 {
            return Err(RendererError::invalid_argument(
                "Mip level count cannot be zero.",
            ));
        }
        let max_mip_level_count = u32::BITS - options.width.max(options.height).leading_zeros();
        if options.mip_level_count > max_mip_level_count {
            return Err(RendererError::InvalidArgument(format!(
                "Requested mip level count `{}` is greater than the maximum possible count this texture can have `{}`.",
                options.mip_level_count, max_mip_level_count
            )));
        }

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut device_context.transfer_queue_context.frames
            [self.current_frame_index as usize]
            .thread_contexts[thread_context_index];

        let staging_buffer_info = ash::vk::BufferCreateInfo::default()
            .size(expected_size as u64)
            .usage(ash::vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(ash::vk::SharingMode::EXCLUSIVE);
        let staging_allocation_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::AutoPreferHost,
            flags: vk_mem::AllocationCreateFlags::MAPPED
                | vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let (staging_buffer, mut staging_allocation) = unsafe {
            device_context
                .vma_allocator
                .create_buffer(&staging_buffer_info, &staging_allocation_info)?
        };
        unsafe {
            let allocation_info = device_context
                .vma_allocator
                .get_allocation_info(&staging_allocation);
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                allocation_info.mapped_data as *mut u8,
                data.len(),
            );
        }
        if let Err(error) = device_context.vma_allocator.flush_allocation(
            &staging_allocation,
            0,
            expected_size as u64,
        ) {
            unsafe {
                device_context
                    .vma_allocator
                    .destroy_buffer(staging_buffer, &mut staging_allocation);
            }
            return Err(error.into());
        }

        let vk_format: ash::vk::Format = options.format.into();
        let image_info = ash::vk::ImageCreateInfo::default()
            .image_type(ash::vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(ash::vk::Extent3D {
                width: options.width,
                height: options.height,
                depth: 1,
            })
            .mip_levels(options.mip_level_count)
            .array_layers(1)
            .samples(ash::vk::SampleCountFlags::TYPE_1)
            .tiling(ash::vk::ImageTiling::OPTIMAL)
            .usage(
                ash::vk::ImageUsageFlags::TRANSFER_SRC
                    | ash::vk::ImageUsageFlags::TRANSFER_DST
                    | ash::vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(ash::vk::SharingMode::EXCLUSIVE)
            .initial_layout(ash::vk::ImageLayout::UNDEFINED);
        let image_allocation_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::AutoPreferDevice,
            ..Default::default()
        };
        let (image, mut image_allocation) = match unsafe {
            device_context
                .vma_allocator
                .create_image(&image_info, &image_allocation_info)
        } {
            Ok(result) => result,
            Err(error) => {
                unsafe {
                    device_context
                        .vma_allocator
                        .destroy_buffer(staging_buffer, &mut staging_allocation);
                }
                return Err(error.into());
            }
        };
        let image_view_info = ash::vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(ash::vk::ImageViewType::TYPE_2D)
            .format(vk_format)
            .subresource_range(
                ash::vk::ImageSubresourceRange::default()
                    .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(options.mip_level_count)
                    .base_array_layer(0)
                    .layer_count(1),
            );
        let image_view = match unsafe {
            device_context
                .logical_device
                .create_image_view(&image_view_info, None)
        } {
            Ok(image_view) => image_view,
            Err(error) => {
                unsafe {
                    device_context
                        .vma_allocator
                        .destroy_image(image, &mut image_allocation);
                    device_context
                        .vma_allocator
                        .destroy_buffer(staging_buffer, &mut staging_allocation);
                }
                return Err(error.into());
            }
        };

        Self::transition_image_layout(
            &device_context.logical_device,
            current_frame.command_buffer,
            image,
            ash::vk::ImageLayout::UNDEFINED,
            ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ash::vk::AccessFlags::empty(),
            ash::vk::AccessFlags::TRANSFER_WRITE,
            ash::vk::PipelineStageFlags::TOP_OF_PIPE,
            ash::vk::PipelineStageFlags::TRANSFER,
            ash::vk::ImageAspectFlags::COLOR,
            1,
        );

        let copy_region = ash::vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                ash::vk::ImageSubresourceLayers::default()
                    .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .image_offset(ash::vk::Offset3D::default())
            .image_extent(ash::vk::Extent3D {
                width: options.width,
                height: options.height,
                depth: 1,
            });
        unsafe {
            device_context.logical_device.cmd_copy_buffer_to_image(
                current_frame.command_buffer,
                staging_buffer,
                image,
                ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&copy_region),
            );
        }

        Self::transition_image_layout(
            &device_context.logical_device,
            current_frame.command_buffer,
            image,
            ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ash::vk::AccessFlags::TRANSFER_WRITE,
            ash::vk::AccessFlags::SHADER_READ,
            ash::vk::PipelineStageFlags::TRANSFER,
            ash::vk::PipelineStageFlags::ALL_GRAPHICS,
            ash::vk::ImageAspectFlags::COLOR,
            1,
        );

        current_frame.transfer_buffers.push(VulkanBuffer {
            buffer: staging_buffer,
            vma_allocation: staging_allocation,
            size: expected_size,
        });
        current_frame.recorded = true;

        Ok(VulkanTexture {
            image,
            image_view,
            vma_allocation: image_allocation,
            texture_options: *options,
        })
    }

    fn destroy_texture(&self, texture: &mut Self::Texture) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        unsafe {
            device_context
                .logical_device
                .destroy_image_view(texture.image_view, None);
            device_context
                .vma_allocator
                .destroy_image(texture.image, &mut texture.vma_allocation);
        }

        texture.image = ash::vk::Image::null();
        texture.image_view = ash::vk::ImageView::null();
        texture.texture_options = TextureOptions::default();

        Ok(())
    }

    fn create_sampler(&self, options: &SamplerOptions) -> RendererResult<Self::Sampler> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let create_info = ash::vk::SamplerCreateInfo::default()
            .mag_filter(options.mag_filter.into())
            .min_filter(options.min_filter.into())
            .mipmap_mode(ash::vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(options.address_mode_u.into())
            .address_mode_v(options.address_mode_v.into())
            .address_mode_w(options.address_mode_w.into())
            .min_lod(0.0)
            .max_lod(0.0)
            .anisotropy_enable(false);
        let sampler = unsafe {
            device_context
                .logical_device
                .create_sampler(&create_info, None)?
        };
        Ok(VulkanSampler { sampler })
    }

    fn destroy_sampler(&self, sampler: &mut Self::Sampler) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        unsafe {
            device_context
                .logical_device
                .destroy_sampler(sampler.sampler, None);
        }
        sampler.sampler = ash::vk::Sampler::null();
        Ok(())
    }

    fn create_compute_pipeline(&self, shader: &Shader) -> RendererResult<Self::ComputePipeline> {
        struct ShaderModuleGuard<'a> {
            module: ash::vk::ShaderModule,
            device_context: &'a DeviceContext,
        }
        impl<'a> Drop for ShaderModuleGuard<'a> {
            fn drop(&mut self) {
                unsafe {
                    self.device_context
                        .logical_device
                        .destroy_shader_module(self.module, None);
                }
            }
        }

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let (prefix, code_u32, suffix) = unsafe { shader.data.align_to::<u32>() };
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(RendererError::Fail(format!(
                "Shader data from '{}' is not valid SPIR-V (not 4-byte aligned).",
                shader.file_path
            )));
        }
        let shader_module_guard = ShaderModuleGuard {
            module: unsafe {
                device_context
                    .logical_device
                    .create_shader_module(&ShaderModuleCreateInfo::default().code(code_u32), None)?
            },
            device_context,
        };
        let shader_stage = Self::convert_shader_stage(shader.metadata.stage)?;

        let mut binding_sets = vec![
            Vec::with_capacity(shader.metadata.descriptor_bindings.len());
            shader.descriptor_group_count()
        ];
        for binding in &shader.metadata.descriptor_bindings {
            binding_sets[binding.group as usize].push(
                DescriptorSetLayoutBinding::default()
                    .binding(binding.binding)
                    .descriptor_type(binding.binding_type.into())
                    .descriptor_count(1)
                    .stage_flags(shader_stage),
            );
        }

        let mut descriptor_layouts = Vec::with_capacity(binding_sets.len());
        for binding_set in &binding_sets {
            let layout_info = DescriptorSetLayoutCreateInfo::default().bindings(binding_set);
            descriptor_layouts.push(unsafe {
                device_context
                    .logical_device
                    .create_descriptor_set_layout(&layout_info, None)?
            });
        }

        let pipeline_layout_info =
            PipelineLayoutCreateInfo::default().set_layouts(&descriptor_layouts);
        let layout = unsafe {
            device_context
                .logical_device
                .create_pipeline_layout(&pipeline_layout_info, None)?
        };

        let shader_entry_name = CString::new(shader.metadata.entry_name.clone())?;
        let stage_info = PipelineShaderStageCreateInfo::default()
            .stage(shader_stage)
            .module(shader_module_guard.module)
            .name(&shader_entry_name);

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
            descriptor_layouts,
        })
    }

    fn destroy_compute_pipeline(&self, pipeline: &Self::ComputePipeline) -> RendererResult<()> {
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
            for descriptor_layout in &pipeline.descriptor_layouts {
                device_context
                    .logical_device
                    .destroy_descriptor_set_layout(*descriptor_layout, None);
            }
        }
        Ok(())
    }

    fn record_compute_command(
        &mut self,
        pipeline: &Self::ComputePipeline,
        binding_sets: &[&[ResourceBinding<Self::Buffer, Self::Texture, Self::Sampler>]],
        group_count: (u32, u32, u32),
    ) -> RendererResult<()> {
        let mapped_sets = self.create_compute_resource_sets(pipeline, binding_sets)?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let compute_queue_context = device_context.compute_queue_context.as_mut().ok_or(
            RendererError::invalid_operation(
                "Device is not initialized with the `ComputeShaders` feature.",
            ),
        )?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut compute_queue_context.frames[self.current_frame_index as usize]
            .thread_contexts[thread_context_index];

        unsafe {
            device_context.logical_device.cmd_bind_pipeline(
                current_frame.command_buffer,
                PipelineBindPoint::COMPUTE,
                pipeline.pipeline,
            );
            if !mapped_sets.is_empty() {
                device_context.logical_device.cmd_bind_descriptor_sets(
                    current_frame.command_buffer,
                    PipelineBindPoint::COMPUTE,
                    pipeline.layout,
                    0,
                    &mapped_sets,
                    &[],
                );
            }
            device_context.logical_device.cmd_dispatch(
                current_frame.command_buffer,
                group_count.0,
                group_count.1,
                group_count.2,
            );
        }

        current_frame.recorded = true;
        Ok(())
    }

    fn create_graphics_pipeline(
        &mut self,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> RendererResult<Self::GraphicsPipeline> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        rendering.create_graphics_pipeline(&device_context.logical_device, shaders, options)
    }

    fn destroy_graphics_pipeline(
        &mut self,
        pipeline: &Self::GraphicsPipeline,
    ) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        rendering.destroy_graphics_pipeline(&device_context.logical_device, pipeline)
    }

    fn record_graphics_command(
        &mut self,
        pipeline: &Self::GraphicsPipeline,
        binding_sets: &[&[ResourceBinding<Self::Buffer, Self::Texture, Self::Sampler>]],
        draw_count: u32,
        instance_count: u32,
    ) -> RendererResult<()> {
        let mapped_sets = self.create_graphics_resource_sets(pipeline, binding_sets)?;
        let vertex_bindings = binding_sets
            .iter()
            .flat_map(|set| set.iter())
            .filter_map(|binding| match binding.resource {
                ResourceBindingType::Buffer {
                    handle,
                    usage: BufferUsage::Vertex,
                    offset,
                    size,
                } => Some((binding.binding, handle, offset, size)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let index_bindings = binding_sets
            .iter()
            .flat_map(|set| set.iter())
            .filter_map(|binding| match binding.resource {
                ResourceBindingType::Buffer {
                    handle,
                    usage: BufferUsage::Index,
                    offset,
                    size,
                } => Some((binding.binding, handle, offset, size)),
                _ => None,
            })
            .collect::<Vec<_>>();

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let graphics_queue_context = &mut device_context.graphics_queue_context;
        let current_frame_index = self.current_frame_index as usize;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut graphics_queue_context.frames[current_frame_index].thread_contexts
            [thread_context_index];
        unsafe {
            device_context.logical_device.cmd_bind_pipeline(
                current_frame.command_buffer,
                PipelineBindPoint::GRAPHICS,
                pipeline.pipeline,
            );
            if !mapped_sets.is_empty() {
                device_context.logical_device.cmd_bind_descriptor_sets(
                    current_frame.command_buffer,
                    PipelineBindPoint::GRAPHICS,
                    pipeline.layout,
                    0,
                    &mapped_sets,
                    &[],
                );
            }
            for (binding, handle, offset, size) in vertex_bindings {
                if offset + size > handle.size {
                    return Err(RendererError::invalid_argument(
                        "Buffer overflow when binding vertex buffer.",
                    ));
                }

                device_context.logical_device.cmd_bind_vertex_buffers(
                    current_frame.command_buffer,
                    binding,
                    std::slice::from_ref(&handle.buffer),
                    std::slice::from_ref(&(offset as u64)),
                );
            }

            if !index_bindings.is_empty() {
                if index_bindings.len() > 1 {
                    return Err(RendererError::invalid_argument(
                        "Only one index buffer can be bound per draw.",
                    ));
                }
                device_context.logical_device.cmd_bind_index_buffer(
                    current_frame.command_buffer,
                    index_bindings[0].1.buffer,
                    index_bindings[0].2 as u64,
                    ash::vk::IndexType::UINT32,
                );
                device_context.logical_device.cmd_draw_indexed(
                    current_frame.command_buffer,
                    draw_count,
                    instance_count,
                    0,
                    0,
                    0,
                );
            } else {
                device_context.logical_device.cmd_draw(
                    current_frame.command_buffer,
                    draw_count,
                    instance_count,
                    0,
                    0,
                );
            }
        }

        current_frame.recorded = true;
        Ok(())
    }

    fn begin_frame(&mut self) -> RendererResult<()> {
        let is_in_main_thread = self.is_in_main_thread();
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame_index = self.current_frame_index as usize;

        // Wait for current frame to finish.
        if is_in_main_thread {
            let wait_frame_sync = |queue_context: &mut QueueContext| -> RendererResult<()> {
                let frame_sync = queue_context
                    .frame_sync
                    .as_mut()
                    .ok_or(RendererError::invalid_operation("Device is not set."))?;
                frame_sync.wait(&device_context.logical_device, current_frame_index)
            };
            wait_frame_sync(&mut device_context.graphics_queue_context)?;
            wait_frame_sync(&mut device_context.transfer_queue_context)?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                wait_frame_sync(compute_queue_context)?;
            }

            if device_context
                .swapchain_context
                .image_avaliable_semaphore_index
                == SwapchainContext::INVALID_INDEX
            {
                device_context.swapchain_context.current_image_index = unsafe {
                    device_context
                        .swapchain_context
                        .loader
                        .acquire_next_image(
                            device_context.swapchain_context.swapchain,
                            1e9 as u64,
                            device_context.swapchain_context.semaphores[current_frame_index].0,
                            ash::vk::Fence::null(),
                        )?
                        .0 as usize
                };
                device_context
                    .swapchain_context
                    .image_avaliable_semaphore_index = current_frame_index;
            }
        }
        // TODO: Wait for main thread to call this function first.

        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let mut begin_queue = |queue_context: &mut QueueContext,
                               is_graphics: bool|
         -> RendererResult<()> {
            let current_frame = &mut queue_context.frames[current_frame_index].thread_contexts
                [thread_context_index];
            let (state_mutex, _) = &current_frame.sync_state;
            let mut state = state_mutex.lock()?;
            if *state != FrameState::Idle {
                return Err(RendererError::invalid_operation(
                    "Invalid frame state, did you forget to call `end_frame`?",
                ));
            }
            unsafe {
                device_context.logical_device.reset_command_pool(
                    current_frame.command_pool,
                    ash::vk::CommandPoolResetFlags::empty(),
                )?;
                device_context.logical_device.reset_descriptor_pool(
                    current_frame.descriptor_pool,
                    DescriptorPoolResetFlags::empty(),
                )?;
                for transfer_buffer in &mut current_frame.transfer_buffers {
                    device_context
                        .vma_allocator
                        .destroy_buffer(transfer_buffer.buffer, &mut transfer_buffer.vma_allocation)
                }
                current_frame.transfer_buffers.clear();

                let begin_info = CommandBufferBeginInfo::default()
                    .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
                device_context
                    .logical_device
                    .begin_command_buffer(current_frame.command_buffer, &begin_info)?;
                if is_graphics {
                    let layer_count = match self.settings.stereoscopic_3d_rendering {
                        true => 2,
                        false => 1,
                    };
                    Self::transition_image_layout(
                        &device_context.logical_device,
                        current_frame.command_buffer,
                        device_context.swapchain_context.images
                            [device_context.swapchain_context.current_image_index],
                        ash::vk::ImageLayout::UNDEFINED,
                        ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        ash::vk::AccessFlags::empty(),
                        ash::vk::AccessFlags::COLOR_ATTACHMENT_READ
                            | ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        ash::vk::ImageAspectFlags::COLOR,
                        layer_count,
                    );
                    Self::transition_image_layout(
                        &device_context.logical_device,
                        current_frame.command_buffer,
                        device_context.swapchain_context.depth_image,
                        ash::vk::ImageLayout::UNDEFINED,
                        ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | ash::vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        ash::vk::ImageAspectFlags::DEPTH,
                        layer_count,
                    );
                    if self.settings.msaa != Msaa::X1 {
                        Self::transition_image_layout(
                            &device_context.logical_device,
                            current_frame.command_buffer,
                            device_context.swapchain_context.msaa_color_image,
                            ash::vk::ImageLayout::UNDEFINED,
                            ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            ash::vk::AccessFlags::empty(),
                            ash::vk::AccessFlags::COLOR_ATTACHMENT_READ
                                | ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                            ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            ash::vk::ImageAspectFlags::COLOR,
                            layer_count,
                        );
                    }
                    rendering.begin(
                        &device_context.logical_device,
                        current_frame.command_buffer,
                        &device_context.swapchain_context,
                    )?;

                    let extent = device_context.swapchain_context.extent;
                    let viewport = ash::vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: extent.width as f32,
                        height: extent.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    };
                    let scissor = ash::vk::Rect2D {
                        offset: ash::vk::Offset2D { x: 0, y: 0 },
                        extent,
                    };
                    device_context.logical_device.cmd_set_viewport(
                        current_frame.command_buffer,
                        0,
                        std::slice::from_ref(&viewport),
                    );
                    device_context.logical_device.cmd_set_scissor(
                        current_frame.command_buffer,
                        0,
                        std::slice::from_ref(&scissor),
                    );
                }
            }
            *state = FrameState::Started;
            Ok(())
        };
        begin_queue(&mut device_context.graphics_queue_context, true)?;
        begin_queue(&mut device_context.transfer_queue_context, false)?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            begin_queue(compute_queue_context, false)?;
        }

        Ok(())
    }

    fn end_frame(&mut self) -> RendererResult<()> {
        let is_in_main_thread = self.is_in_main_thread();
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let current_frame_index = self.current_frame_index as usize;
        let thread_context_index = Self::thread_context_index()?;
        let image_index = device_context.swapchain_context.current_image_index;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;

        let mut end_queue = |queue_context: &mut QueueContext,
                             is_graphics: bool|
         -> RendererResult<()> {
            let current_frame = &mut queue_context.frames[current_frame_index].thread_contexts
                [thread_context_index];
            let (state_mutex, condvar) = &current_frame.sync_state;
            let mut state = state_mutex.lock()?;
            if *state != FrameState::Started {
                return Err(RendererError::invalid_operation(
                    "Cannot end a frame that hasn't started yet.",
                ));
            }
            unsafe {
                if is_graphics {
                    rendering.end(&device_context.logical_device, current_frame.command_buffer)?;
                    Self::transition_image_layout(
                        &device_context.logical_device,
                        current_frame.command_buffer,
                        device_context.swapchain_context.images
                            [device_context.swapchain_context.current_image_index],
                        ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        ash::vk::ImageLayout::PRESENT_SRC_KHR,
                        ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        ash::vk::AccessFlags::empty(),
                        ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        ash::vk::ImageAspectFlags::COLOR,
                        match self.settings.stereoscopic_3d_rendering {
                            true => 2,
                            false => 1,
                        },
                    );
                }
                device_context
                    .logical_device
                    .end_command_buffer(current_frame.command_buffer)?;
            }
            *state = FrameState::Finished;
            condvar.notify_one();
            Ok(())
        };
        end_queue(&mut device_context.graphics_queue_context, true)?;
        end_queue(&mut device_context.transfer_queue_context, false)?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            end_queue(compute_queue_context, false)?;
        }

        if is_in_main_thread {
            let image_available_semaphores = [device_context.swapchain_context.semaphores
                [device_context
                    .swapchain_context
                    .image_avaliable_semaphore_index]
                .0];
            let render_finished_semaphores = [device_context.swapchain_context.semaphores
                [device_context.swapchain_context.current_image_index]
                .1];
            let wait_stages = [ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            let mut graphics_command_buffers = Vec::new();
            let mut transfer_command_buffers = Vec::new();
            let mut compute_command_buffers = Vec::new();
            let masks = device_context.thread_context_masks.lock()?;
            for thread_context_index in 0..super::thread_context::thread_context_count() {
                if !is_thread_context_active(thread_context_index, &masks) {
                    continue;
                }

                let add_command_buffer = |queue_context: &mut QueueContext,
                                          command_buffers: &mut Vec<ash::vk::CommandBuffer>|
                 -> RendererResult<()> {
                    let current_frame = &mut queue_context.frames[current_frame_index]
                        .thread_contexts[thread_context_index];

                    // Wait for the worker thread to signal the frame is finished.
                    let (state_mutex, condvar) = &current_frame.sync_state;
                    let mut state = state_mutex.lock()?;
                    if *state == FrameState::Idle {
                        return Err(RendererError::invalid_operation(
                            "Invalid frame state, did you forget to call `begin_frame`?",
                        ));
                    }
                    while *state != FrameState::Finished {
                        state = condvar.wait(state)?;
                    }
                    *state = FrameState::Idle;

                    if current_frame.recorded {
                        command_buffers.push(current_frame.command_buffer);
                        current_frame.recorded = false;
                    }

                    Ok(())
                };
                add_command_buffer(
                    &mut device_context.graphics_queue_context,
                    &mut graphics_command_buffers,
                )?;
                add_command_buffer(
                    &mut device_context.transfer_queue_context,
                    &mut transfer_command_buffers,
                )?;
                if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                    add_command_buffer(compute_queue_context, &mut compute_command_buffers)?;
                }
            }

            let submit_commands = |queue_context: &mut QueueContext,
                                   command_buffers: &Vec<ash::vk::CommandBuffer>,
                                   is_graphics: bool|
             -> RendererResult<()> {
                if !command_buffers.is_empty() {
                    let submit_info = match is_graphics {
                        true => SubmitInfo::default()
                            .command_buffers(command_buffers)
                            .wait_semaphores(&image_available_semaphores)
                            .wait_dst_stage_mask(&wait_stages)
                            .signal_semaphores(&render_finished_semaphores),
                        false => SubmitInfo::default().command_buffers(command_buffers),
                    };
                    let frame_sync = queue_context
                        .frame_sync
                        .as_mut()
                        .ok_or(RendererError::invalid_operation("Device is not set."))?;
                    frame_sync.submit(
                        &device_context.logical_device,
                        current_frame_index,
                        queue_context.queue,
                        &[submit_info],
                    )?;
                }
                Ok(())
            };
            // TODO: This works when the device does not support `AsyncTransfer`. Implement
            // synchronization via semaphores when asynchronous transfers are supported.
            submit_commands(
                &mut device_context.transfer_queue_context,
                &transfer_command_buffers,
                false,
            )?;
            submit_commands(
                &mut device_context.graphics_queue_context,
                &graphics_command_buffers,
                true,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                submit_commands(compute_queue_context, &compute_command_buffers, false)?;
            }

            if !graphics_command_buffers.is_empty() {
                let wait_semaphores = [device_context.swapchain_context.semaphores[image_index].1];
                let swapchains = [device_context.swapchain_context.swapchain];
                let image_indices = [image_index as u32];
                let present_info = ash::vk::PresentInfoKHR::default()
                    .wait_semaphores(&wait_semaphores)
                    .swapchains(&swapchains)
                    .image_indices(&image_indices);
                let _suboptimal = unsafe {
                    device_context
                        .swapchain_context
                        .loader
                        .queue_present(device_context.graphics_queue_context.queue, &present_info)?
                };
                // TODO: Recreate swapchain if suboptimal.
                device_context
                    .swapchain_context
                    .image_avaliable_semaphore_index = SwapchainContext::INVALID_INDEX;
            }

            self.current_frame_index =
                (self.current_frame_index + 1) % self.settings.frames_in_flight;
        }

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

    fn clear(&mut self, color: RGB<f32>) -> RendererResult<()> {
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(RendererError::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame_index = self.current_frame_index as usize;
        let current_frame = &mut device_context.graphics_queue_context.frames[current_frame_index]
            .thread_contexts[thread_context_index];

        {
            let state = current_frame.sync_state.0.lock()?;
            if *state != FrameState::Started {
                return Err(RendererError::invalid_operation(
                    "Cannot clear outside of a frame.",
                ));
            }
        }

        let clear_attachment = ash::vk::ClearAttachment::default()
            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
            .color_attachment(0)
            .clear_value(ash::vk::ClearValue {
                color: ash::vk::ClearColorValue {
                    float32: [color.r, color.g, color.b, 1.0],
                },
            });
        let clear_rect = ash::vk::ClearRect::default()
            .rect(ash::vk::Rect2D {
                offset: ash::vk::Offset2D { x: 0, y: 0 },
                extent: device_context.swapchain_context.extent,
            })
            .base_array_layer(0)
            .layer_count(match self.settings.stereoscopic_3d_rendering {
                true => 2,
                false => 1,
            });
        unsafe {
            device_context.logical_device.cmd_clear_attachments(
                current_frame.command_buffer,
                std::slice::from_ref(&clear_attachment),
                std::slice::from_ref(&clear_rect),
            );
        }

        current_frame.recorded = true;
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        if let Err(e) = self.uninitialize() {
            eprintln!("Failed to uninitialize renderer on drop: {}", e);
        }
    }
}
