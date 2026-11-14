mod canvas;
mod descriptor_pool;
mod device;
mod error;
mod frame;
mod handle;
mod pipeline;
mod queue;
mod rendering;
mod resources;
mod settings;
mod swapchain;
mod sync;
mod thread;
mod version;

use std::{
    any::Any,
    cell::Cell,
    collections::{BTreeSet, HashMap, HashSet},
    ffi::CString,
    sync::Mutex,
    time::{Duration, Instant},
};

use ash::vk::{
    ApplicationInfo, BufferCreateInfo, BufferUsageFlags, CommandBufferBeginInfo,
    CommandBufferUsageFlags, ComputePipelineCreateInfo, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateInfo, DeviceCreateInfo, DeviceQueueCreateInfo, InstanceCreateInfo,
    MemoryHeapFlags, PhysicalDeviceFeatures2, PhysicalDeviceType, PipelineBindPoint, PipelineCache,
    PipelineLayoutCreateInfo, PipelineShaderStageCreateInfo, QueueFlags, ShaderModuleCreateInfo,
    StructureType, SubmitInfo, SurfaceKHR, SwapchainKHR,
};
use renkrs::RGB;
use vk_mem::Alloc;

use crate::{
    Error, GraphicsDeviceId, GraphicsDeviceVendor, HEPHGL_VERSION, Result, Version,
    graphics_device::{GraphicsDevice, GraphicsDeviceFeature, GraphicsDeviceType},
    renderers::{
        BufferUsage, FeatureRequest, InitializeOptions, Renderer, RendererCanvas,
        RendererComputePipeline, RendererGraphicsPipeline, ResourceBinding, Settings, resource_mut,
        resource_ref,
        resources::{RendererBuffer, RendererSampler, RendererTexture},
        settings::{CanvasOptions, GraphicsPipelineOptions, Msaa, SamplerOptions, TextureOptions},
        thread_context::{
            MAIN_THREAD_CONTEXT_INDEX, ThreadContextIndex, ThreadContextMask,
            ThreadContextMaskArray, is_thread_context_active,
        },
        version::DriverVersion,
        vulkan::{
            canvas::{VulkanCanvas, VulkanColorCanvas, VulkanDepthCanvas, vulkan_canvas_mut},
            device::{DeviceContext, VulkanGraphicsDeviceId},
            frame::{ComputeFrame, FrameState, GraphicsFrame, TransferFrame, VulkanFrame},
            pipeline::{VulkanComputePipeline, VulkanGraphicsPipeline},
            queue::QueueContext,
            rendering::VulkanRendering,
            resources::{DescriptorSetCache, VulkanBuffer, VulkanSampler, VulkanTexture},
            swapchain::SwapchainContext,
            thread::{
                ComputeThreadContext, GraphicsThreadContext, TransferThreadContext,
                VulkanThreadContext,
            },
            version::VulkanApiVersion,
        },
    },
    shader::Shader,
};

const MIN_API_VERSION: Version = Version::new(1, 1, 0);

thread_local! {
    /// Index of the current thread context.
    static THREAD_CONTEXT_INDEX: ThreadContextIndex = const { ThreadContextIndex::new() };
}

/// The Vulkan implementation of the `Renderer` trait.
pub struct VulkanRenderer {
    settings: Settings,

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
    fn new() -> Self {
        Self {
            settings: Settings::default(),

            entry: None,
            instance: None,
            latest_api_version: Cell::new(None),
            api_version: MIN_API_VERSION,

            window_surface: None,
            window_surface_loader: None,

            device_context: None,

            main_thread_id: std::thread::current().id(),
        }
    }

    fn min_api_version(&self) -> Version {
        MIN_API_VERSION
    }

    fn latest_api_version(&self) -> Result<Version> {
        if let Some(latest_api_version) = self.latest_api_version.get() {
            // Use the cached version.
            return Ok(latest_api_version);
        }

        let enumerate_instance_version = |entry: &ash::Entry| -> Result<Version> {
            Ok(match unsafe { entry.try_enumerate_instance_version()? } {
                Some(v) => Version::from(VulkanApiVersion(v)).max(MIN_API_VERSION),
                None => MIN_API_VERSION,
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

    fn set_settings(&mut self, settings: Settings) -> Result<()> {
        self.main_thread_only()?;
        if settings.max_fps == Some(0) {
            return Err(Error::invalid_argument("`max_fps` must be greater than 0."));
        }

        if self.device_context.is_some() {
            let temp_settings = self.settings;

            self.destroy_rendering()?;
            self.destroy_swapchain()?;
            self.destroy_frame_sync()?;
            self.uninitialize_thread()?;

            self.settings = settings;

            let device_context = self.device_context.as_mut().unwrap();

            fn resize_frames<Frame: VulkanFrame>(
                thread_context_masks: &Mutex<ThreadContextMaskArray>,
                settings: &mut Settings,
                temp_settings: Settings,
                queue_context: &mut QueueContext<Frame>,
            ) -> Result<()> {
                let masks = thread_context_masks.lock()?;

                for mask in masks.iter() {
                    if *mask != 0 {
                        *settings = temp_settings;
                        return Err(Error::invalid_operation(
                            "All worker threads must be uninitialized before changing the settings.",
                        ));
                    }
                }
                queue_context
                    .frames
                    .resize_with(settings.frames_in_flight, Frame::default);
                Ok(())
            }
            resize_frames(
                &device_context.thread_context_masks,
                &mut self.settings,
                temp_settings,
                &mut device_context.graphics_queue_context,
            )?;
            resize_frames(
                &device_context.thread_context_masks,
                &mut self.settings,
                temp_settings,
                &mut device_context.transfer_queue_context,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                resize_frames(
                    &device_context.thread_context_masks,
                    &mut self.settings,
                    temp_settings,
                    compute_queue_context,
                )?;
            }

            device_context.current_frame_index = 0;
            device_context.next_frame_deadline = None;

            self.create_frame_sync()?;
            self.initialize_thread()?;
            self.create_swapchain()?;
            self.create_rendering()?;
        } else {
            self.settings = settings;
        }

        Ok(())
    }

    fn initialize(&mut self, options: &InitializeOptions) -> Result<()> {
        self.main_thread_only()?;
        if self.entry.is_some() || self.instance.is_some() {
            return Err(Error::invalid_operation(
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
            return Err(Error::InvalidArgument(format!(
                "Requested Vulkan API version is not supported (requested: `{}`, min: `{}`, max: `{}`).",
                requested_api_version,
                MIN_API_VERSION,
                self.latest_api_version()?
            )));
        }

        let engine_name = c"HephGL";
        let c_app_name = CString::new(options.app_name)?;
        self.api_version = requested_api_version;
        let app_info = ApplicationInfo {
            s_type: StructureType::APPLICATION_INFO,
            p_engine_name: engine_name.as_ptr(),
            engine_version: VulkanApiVersion::from(HEPHGL_VERSION).0,
            p_application_name: c_app_name.as_ptr(),
            api_version: VulkanApiVersion::from(self.api_version).0,
            ..Default::default()
        };

        let extension_properties = unsafe { entry.enumerate_instance_extension_properties(None)? };
        let supports_portability_enumeration = extension_properties.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name == ash::vk::KHR_PORTABILITY_ENUMERATION_NAME
        });
        let create_flags = match supports_portability_enumeration {
            true => ash::vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR,
            false => ash::vk::InstanceCreateFlags::empty(),
        };

        let mut required_extension_names =
            ash_window::enumerate_required_extensions(options.display_handle)?.to_vec();
        if supports_portability_enumeration {
            required_extension_names.push(ash::vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr());
        }

        let mut enabled_layer_names = Vec::new();
        if cfg!(debug_assertions) {
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
            let validation_layer_name = c"VK_LAYER_KHRONOS_validation";
            if available_layers
                .iter()
                .any(|name| name == validation_layer_name)
            {
                enabled_layer_names.push(validation_layer_name.as_ptr());
            }
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

    fn uninitialize(&mut self) -> Result<()> {
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

    fn enumerate_devices(&self) -> Result<Vec<GraphicsDevice>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;

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
            let device_api_version: Version = VulkanApiVersion(properties.api_version).into();
            if device_api_version < MIN_API_VERSION {
                continue;
            }

            let device_name = unsafe {
                std::ffi::CStr::from_ptr(properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };

            let device_type = match properties.device_type {
                PhysicalDeviceType::DISCRETE_GPU => GraphicsDeviceType::DiscreteGpu,
                PhysicalDeviceType::INTEGRATED_GPU => GraphicsDeviceType::IntegratedGpu,
                PhysicalDeviceType::VIRTUAL_GPU => GraphicsDeviceType::VirtualGpu,
                PhysicalDeviceType::CPU => GraphicsDeviceType::Cpu,
                PhysicalDeviceType::OTHER => GraphicsDeviceType::Other,
                _ => GraphicsDeviceType::Invalid,
            };

            let device_vendor_id = properties.vendor_id;
            let device_id = Box::new(self.get_device_id(physical_device)?);

            let device_driver_version = DriverVersion::new(
                properties.driver_version,
                GraphicsDeviceVendor::from_id(properties.vendor_id),
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
            let supported_msaa_levels = MSAA_ALL
                .into_iter()
                .filter(|msaa| sample_counts.contains((*msaa).into()))
                .collect::<BTreeSet<Msaa>>();

            let mut supported_features = BTreeSet::<GraphicsDeviceFeature>::default();
            let extension_properties =
                unsafe { instance.enumerate_device_extension_properties(physical_device)? };
            if physical_features.sampler_anisotropy == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::AnisotropicFiltering);
            }
            if physical_features.texture_compression_bc == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::BcTextures);
            }
            if physical_features.depth_bias_clamp == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::DepthBiasClamp);
            }
            if physical_features.depth_clamp == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::DepthClamp);
            }
            if physical_features.geometry_shader == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::GeometryShaders);
            }
            if physical_features.sample_rate_shading == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::SampleRateShading);
            }
            if physical_features.tessellation_shader == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::TessellationShaders);
            }
            if physical_features.fill_mode_non_solid == ash::vk::TRUE {
                let _ = supported_features.insert(GraphicsDeviceFeature::WireframeMode);
            }
            for ext in extension_properties {
                let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
                if name == ash::vk::KHR_RAY_TRACING_PIPELINE_NAME {
                    let _ = supported_features.insert(GraphicsDeviceFeature::RayTracing);
                }
            }
            for queue_family_properties in &queue_family_properties_vec {
                let queue_flags = queue_family_properties.queue_flags;
                if queue_flags.contains(QueueFlags::COMPUTE) {
                    let _ = supported_features.insert(GraphicsDeviceFeature::ComputeShaders);
                }
                if queue_flags.contains(QueueFlags::VIDEO_DECODE_KHR) {
                    let _ = supported_features.insert(GraphicsDeviceFeature::VideoDecoding);
                }
                if queue_flags.contains(QueueFlags::VIDEO_ENCODE_KHR) {
                    let _ = supported_features.insert(GraphicsDeviceFeature::VideoEncoding);
                }
                // Vulkan guarantees that the main graphics family will also support
                // TRANSFER. Thus, we only consider families that support TRANSFER
                // but do not support GRAPHICS and COMPUTE to be dedicated async (DMA) transfer
                // queues.
                if queue_flags.contains(QueueFlags::TRANSFER)
                    && !queue_flags.contains(QueueFlags::GRAPHICS)
                    && !queue_flags.contains(QueueFlags::COMPUTE)
                {
                    let _ = supported_features.insert(GraphicsDeviceFeature::AsyncTransfer);
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
                max_sampler_anisotropy: properties.limits.max_sampler_anisotropy,
                supported_msaa_levels,
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
    ) -> Result<()> {
        self.main_thread_only()?;
        if let Some(device) = device {
            // Check whether the `device` is a Vulkan device.
            let _ = resource_ref::<VulkanGraphicsDeviceId>(device.device_id.as_ref())?;
        }
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
                    .ok_or(Error::fail("No active graphics device found."))?
            }
        };

        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;

        // Create logical device and queues.

        let mut enabled_features = HashSet::with_capacity(requested_features.len());
        for requested_feature in requested_features {
            if device
                .supported_features
                .contains(&requested_feature.feature)
            {
                let _ = enabled_features.insert(requested_feature.feature);
            } else if requested_feature.required {
                return Err(Error::UnsupportedRequiredFeature(requested_feature.feature));
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
                Error::invalid_operation("No queue family supports both graphics and presentation.")
            })?;
        request_queue(graphics_family.index, graphics_family.queue_count);

        // Use DMA Transfer family if available. Otherwise, use the main graphics
        // family.
        let transfer_family = if enabled_features.contains(&GraphicsDeviceFeature::AsyncTransfer) {
            queue_families
                .iter()
                .find(|f| {
                    f.queue_flags.contains(QueueFlags::TRANSFER)
                        && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                        && !f.queue_flags.contains(QueueFlags::COMPUTE)
                })
                .ok_or(Error::fail(
                    "Asynchronous transfer is not supported by this device.",
                ))?
        } else {
            graphics_family
        };
        request_queue(transfer_family.index, transfer_family.queue_count);

        // Use the pure compute family if available. Otherwise, use the main graphics
        // family.
        let mut compute_family = None;
        if enabled_features.contains(&GraphicsDeviceFeature::ComputeShaders) {
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
        let mut physical_device_api_version = MIN_API_VERSION;
        let mut max_tessellation_patch_size = 0;
        for pd in physical_devices {
            let properties = unsafe { instance.get_physical_device_properties(pd) };
            if Version::from(VulkanApiVersion(properties.api_version)) < MIN_API_VERSION {
                continue;
            }
            if device.device_id.as_ref() == &self.get_device_id(pd)? as &dyn GraphicsDeviceId {
                physical_device = Some(pd);
                physical_device_api_version = VulkanApiVersion(properties.api_version).into();
                max_tessellation_patch_size = properties.limits.max_tessellation_patch_size;
                break;
            }
        }
        let physical_device = physical_device.ok_or_else(|| {
            Error::InvalidArgument(format!(
                "Graphics device with the id {} not found.",
                device.device_id
            ))
        })?;

        let extension_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device)? };
        let mut device_extension_names = vec![ash::vk::KHR_SWAPCHAIN_NAME.as_ptr()];
        if extension_properties.iter().any(|ext| {
            let name = unsafe { std::ffi::CStr::from_ptr(ext.extension_name.as_ptr()) };
            name == ash::vk::KHR_PORTABILITY_SUBSET_NAME
        }) {
            // This must be enabled whenever the physical device supports it per specification.
            device_extension_names.push(ash::vk::KHR_PORTABILITY_SUBSET_NAME.as_ptr());
        }
        let mut physical_features2 = PhysicalDeviceFeatures2::default();
        if enabled_features.contains(&GraphicsDeviceFeature::AnisotropicFiltering) {
            physical_features2.features.sampler_anisotropy = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::BcTextures) {
            physical_features2.features.texture_compression_bc = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::DepthBiasClamp) {
            physical_features2.features.depth_bias_clamp = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::DepthClamp) {
            physical_features2.features.depth_clamp = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::GeometryShaders) {
            physical_features2.features.geometry_shader = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::SampleRateShading) {
            physical_features2.features.sample_rate_shading = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::TessellationShaders) {
            physical_features2.features.tessellation_shader = ash::vk::TRUE;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::WireframeMode) {
            physical_features2.features.fill_mode_non_solid = ash::vk::TRUE;
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
        const V13: Version = Version::new(1, 3, 0);
        if self.api_version >= V13 && physical_device_api_version >= V13 {
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

        let mut sampling_stages = ash::vk::PipelineStageFlags::VERTEX_SHADER
            | ash::vk::PipelineStageFlags::FRAGMENT_SHADER;
        if enabled_features.contains(&GraphicsDeviceFeature::GeometryShaders) {
            sampling_stages |= ash::vk::PipelineStageFlags::GEOMETRY_SHADER;
        }
        if enabled_features.contains(&GraphicsDeviceFeature::TessellationShaders) {
            sampling_stages |= ash::vk::PipelineStageFlags::TESSELLATION_CONTROL_SHADER
                | ash::vk::PipelineStageFlags::TESSELLATION_EVALUATION_SHADER;
        }

        self.device_context = Some(DeviceContext {
            graphics_device: device.clone(),
            enabled_features,

            vma_allocator,

            graphics_queue_context: QueueContext {
                queue: graphics_queue,
                queue_family_index: graphics_family.index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| GraphicsFrame::default())
                    .collect(),
                frame_sync: None,
            },
            transfer_queue_context: QueueContext {
                queue: transfer_queue,
                queue_family_index: transfer_family.index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| TransferFrame::default())
                    .collect(),
                frame_sync: None,
            },
            compute_queue_context: compute_queue.map(|queue| QueueContext {
                queue,
                queue_family_index: compute_family.unwrap().index,
                frames: (0..self.settings.frames_in_flight)
                    .map(|_| ComputeFrame::default())
                    .collect(),
                frame_sync: None,
            }),

            swapchain_context: SwapchainContext {
                loader: ash::khr::swapchain::Device::new(instance, &logical_device),
                swapchain: SwapchainKHR::null(),
                invalidated: false,
                format: ash::vk::Format::default(),
                extent: ash::vk::Extent2D::default(),
                images: Vec::default(),
                semaphores: Vec::default(),
                current_image_index: 0,
                image_avaliable_semaphore_index: SwapchainContext::INVALID_INDEX,
            },
            rendering: None,
            display_canvas: None,

            physical_device,
            logical_device,
            supports_timeline_semaphore,
            supports_dynamic_rendering,
            sampling_stages,
            max_tessellation_patch_size,

            current_frame_index: 0,
            current_frame_index_sync: Mutex::new(()),
            next_frame_deadline: None,

            thread_context_masks: Mutex::new(std::array::from_fn(|_| ThreadContextMask::default())),
        });

        self.initialize_thread()?;
        self.create_frame_sync()?;
        self.create_swapchain()?;
        self.create_rendering()?;

        Ok(())
    }

    fn create_buffer(&self, size: usize, usage: BufferUsage) -> Result<Box<dyn RendererBuffer>> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;

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

        Ok(Box::new(VulkanBuffer {
            buffer,
            vma_allocation,
            size,
            usage,
        }))
    }

    fn write_buffer(
        &self,
        buffer: &mut dyn RendererBuffer,
        offset: usize,
        src: &[u8],
    ) -> Result<()> {
        let buffer = resource_ref::<VulkanBuffer>(buffer)?;
        if offset >= buffer.size {
            return Err(Error::invalid_argument(
                "Offset is greater than the source buffer size.",
            ));
        }
        if src.is_empty() {
            return Err(Error::invalid_argument("Source buffer cannot be empty."));
        }
        if offset + src.len() > buffer.size {
            return Err(Error::fail("Buffer overflow."));
        }

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                src.as_ptr(),
                (alloc_info.mapped_data as *mut u8).wrapping_byte_add(offset),
                src.len(),
            );
        }

        Ok(())
    }

    fn read_buffer(
        &self,
        buffer: &dyn RendererBuffer,
        offset: usize,
        dest: &mut [u8],
    ) -> Result<()> {
        let buffer = resource_ref::<VulkanBuffer>(buffer)?;
        if offset >= buffer.size {
            return Err(Error::invalid_argument(
                "Offset is greater than the source buffer size.",
            ));
        }
        if dest.is_empty() {
            return Err(Error::invalid_argument(
                "Destination buffer cannot be empty.",
            ));
        }
        if offset + dest.len() > buffer.size {
            return Err(Error::fail("Buffer overflow."));
        }

        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                (alloc_info.mapped_data as *const u8).wrapping_byte_add(offset),
                dest.as_mut_ptr(),
                dest.len(),
            );
        }

        Ok(())
    }

    fn destroy_buffer(&self, buffer: &mut dyn RendererBuffer) -> Result<()> {
        let buffer = resource_mut::<VulkanBuffer>(buffer)?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        unsafe {
            device_context
                .vma_allocator
                .destroy_buffer(buffer.buffer, &mut buffer.vma_allocation);
        }
        buffer.buffer = ash::vk::Buffer::null();
        buffer.size = 0;
        Ok(())
    }

    fn create_texture(
        &mut self,
        options: &TextureOptions,
        data: &[u8],
    ) -> Result<Box<dyn RendererTexture>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        if options.width == 0 || options.height == 0 {
            return Err(Error::invalid_argument(
                "Texture dimensions must be greater than zero.",
            ));
        }
        if options.format.is_bc_compressed()
            && !device_context
                .enabled_features
                .contains(&GraphicsDeviceFeature::BcTextures)
        {
            return Err(Error::invalid_operation(
                "BC textures require the `BcTextures` feature to be enabled.",
            ));
        }
        if options.mip_level_count == 0 {
            return Err(Error::invalid_argument("Mip level count cannot be zero."));
        }

        let max_mip_level_count =
            TextureOptions::max_mip_level_count(options.width, options.height);
        if options.mip_level_count > max_mip_level_count {
            return Err(Error::InvalidArgument(format!(
                "Requested mip level count `{}` is greater than the maximum possible count this texture can have `{}`.",
                options.mip_level_count, max_mip_level_count
            )));
        }

        let vk_format: ash::vk::Format = options.format.into();
        let format_properties = unsafe {
            instance
                .get_physical_device_format_properties(device_context.physical_device, vk_format)
        };
        if !format_properties
            .optimal_tiling_features
            .contains(ash::vk::FormatFeatureFlags::SAMPLED_IMAGE)
        {
            return Err(Error::InvalidOperation(format!(
                "The device does not support `{:?}` textures.",
                options.format
            )));
        }

        let block_extent = options.format.block_extent();
        let mut mip_level_sizes = Vec::with_capacity(options.mip_level_count as usize);
        let mut all_mip_levels_size = 0usize;
        for mip_level in 0..options.mip_level_count {
            let width = (options.width >> mip_level).max(1).div_ceil(block_extent);
            let height = (options.height >> mip_level).max(1).div_ceil(block_extent);
            let size = (width as usize)
                .checked_mul(height as usize)
                .and_then(|size| size.checked_mul(options.format.bytes_per_block()))
                .ok_or(Error::invalid_argument("Texture dimensions are too large."))?;
            all_mip_levels_size = all_mip_levels_size
                .checked_add(size)
                .ok_or(Error::invalid_argument("Texture dimensions are too large."))?;
            mip_level_sizes.push(size);
        }
        let generate_mip_levels = if data.len() == all_mip_levels_size {
            false
        } else if block_extent > 1 {
            return Err(Error::invalid_operation(
                "Cannot generate mip levels of block-compressed textures.",
            ));
        } else if data.len() == mip_level_sizes[0] {
            mip_level_sizes.truncate(1);
            true
        } else {
            return Err(Error::invalid_argument(
                "Texture data size does not match texture dimensions.",
            ));
        };

        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut device_context.transfer_queue_context.frames
            [device_context.current_frame_index]
            .base
            .thread_contexts[thread_context_index];

        if generate_mip_levels
            && !format_properties.optimal_tiling_features.contains(
                ash::vk::FormatFeatureFlags::BLIT_SRC
                    | ash::vk::FormatFeatureFlags::BLIT_DST
                    | ash::vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR,
            )
        {
            return Err(Error::invalid_argument(
                "The texture does not support multiple mip levels.",
            ));
        }

        let mut copy_regions = Vec::with_capacity(mip_level_sizes.len());
        let mut staging_size = 0;
        for (mip_level, size) in mip_level_sizes.iter().enumerate() {
            copy_regions.push(
                ash::vk::BufferImageCopy::default()
                    .buffer_offset(staging_size as u64)
                    .buffer_row_length(0)
                    .buffer_image_height(0)
                    .image_subresource(
                        ash::vk::ImageSubresourceLayers::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .mip_level(mip_level as u32)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .image_offset(ash::vk::Offset3D::default())
                    .image_extent(ash::vk::Extent3D {
                        width: (options.width >> mip_level).max(1),
                        height: (options.height >> mip_level).max(1),
                        depth: 1,
                    }),
            );
            // Transfer-only queues require the buffer offsets to be multiples of four.
            staging_size += size.next_multiple_of(4);
        }

        let staging_buffer_info = ash::vk::BufferCreateInfo::default()
            .size(staging_size as u64)
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
            let mut data_offset = 0;
            for (copy_region, &size) in copy_regions.iter().zip(&mip_level_sizes) {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(data_offset),
                    (allocation_info.mapped_data as *mut u8)
                        .add(copy_region.buffer_offset as usize),
                    size,
                );
                data_offset += size;
            }
        }
        if let Err(error) = device_context.vma_allocator.flush_allocation(
            &staging_allocation,
            0,
            staging_size as u64,
        ) {
            unsafe {
                device_context
                    .vma_allocator
                    .destroy_buffer(staging_buffer, &mut staging_allocation);
            }
            return Err(error.into());
        }

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
            current_frame.base.command_buffer,
            image,
            ash::vk::ImageLayout::UNDEFINED,
            ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ash::vk::AccessFlags::empty(),
            ash::vk::AccessFlags::TRANSFER_WRITE,
            ash::vk::PipelineStageFlags::TOP_OF_PIPE,
            ash::vk::PipelineStageFlags::TRANSFER,
            ash::vk::ImageAspectFlags::COLOR,
            0,
            options.mip_level_count,
            1,
        );

        unsafe {
            device_context.logical_device.cmd_copy_buffer_to_image(
                current_frame.base.command_buffer,
                staging_buffer,
                image,
                ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copy_regions,
            );
        }

        let mut command_buffer = current_frame.base.command_buffer;
        if device_context
            .enabled_features
            .contains(&GraphicsDeviceFeature::AsyncTransfer)
        {
            // Async transfer queues do not support graphics operations such as blits, thus execute
            // them in graphics queue.
            let ownership_barrier = ash::vk::ImageMemoryBarrier::default()
                .old_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(device_context.transfer_queue_context.queue_family_index)
                .dst_queue_family_index(device_context.graphics_queue_context.queue_family_index)
                .image(image)
                .subresource_range(
                    ash::vk::ImageSubresourceRange::default()
                        .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(options.mip_level_count)
                        .base_array_layer(0)
                        .layer_count(1),
                );
            unsafe {
                device_context.logical_device.cmd_pipeline_barrier(
                    command_buffer,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    std::slice::from_ref(
                        &ownership_barrier.src_access_mask(ash::vk::AccessFlags::TRANSFER_WRITE),
                    ),
                );
            }

            let current_frame = &mut device_context.graphics_queue_context.frames
                [device_context.current_frame_index]
                .base
                .thread_contexts[thread_context_index];
            if let Some((render_target, aspect_mask)) = current_frame.render_target.take() {
                let rendering = device_context
                    .rendering
                    .as_mut()
                    .ok_or(Error::invalid_operation("Device is not set."))?;
                Self::end_rendering(
                    &device_context.logical_device,
                    rendering.as_mut(),
                    current_frame.base.command_buffer,
                    render_target,
                    aspect_mask,
                    self.settings.array_layer_count(),
                    device_context.sampling_stages,
                )?;
            }
            command_buffer = current_frame.base.command_buffer;
            unsafe {
                device_context.logical_device.cmd_pipeline_barrier(
                    command_buffer,
                    ash::vk::PipelineStageFlags::TOP_OF_PIPE,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    std::slice::from_ref(&ownership_barrier.dst_access_mask(
                        ash::vk::AccessFlags::TRANSFER_READ | ash::vk::AccessFlags::TRANSFER_WRITE,
                    )),
                );
            }
            current_frame.base.recorded = true;
        }

        if generate_mip_levels {
            for mip_level in 1..options.mip_level_count {
                let previous_mip_level = mip_level - 1;

                Self::transition_image_layout(
                    &device_context.logical_device,
                    command_buffer,
                    image,
                    ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    ash::vk::AccessFlags::TRANSFER_WRITE,
                    ash::vk::AccessFlags::TRANSFER_READ,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::ImageAspectFlags::COLOR,
                    previous_mip_level,
                    1,
                    1,
                );

                let source_width = (options.width >> previous_mip_level).max(1) as i32;
                let source_height = (options.height >> previous_mip_level).max(1) as i32;
                let destination_width = (options.width >> mip_level).max(1) as i32;
                let destination_height = (options.height >> mip_level).max(1) as i32;

                let blit = ash::vk::ImageBlit::default()
                    .src_subresource(
                        ash::vk::ImageSubresourceLayers::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .mip_level(previous_mip_level)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .src_offsets([
                        ash::vk::Offset3D { x: 0, y: 0, z: 0 },
                        ash::vk::Offset3D {
                            x: source_width,
                            y: source_height,
                            z: 1,
                        },
                    ])
                    .dst_subresource(
                        ash::vk::ImageSubresourceLayers::default()
                            .aspect_mask(ash::vk::ImageAspectFlags::COLOR)
                            .mip_level(mip_level)
                            .base_array_layer(0)
                            .layer_count(1),
                    )
                    .dst_offsets([
                        ash::vk::Offset3D { x: 0, y: 0, z: 0 },
                        ash::vk::Offset3D {
                            x: destination_width,
                            y: destination_height,
                            z: 1,
                        },
                    ]);
                unsafe {
                    device_context.logical_device.cmd_blit_image(
                        command_buffer,
                        image,
                        ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        image,
                        ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        std::slice::from_ref(&blit),
                        ash::vk::Filter::LINEAR,
                    );
                }

                Self::transition_image_layout(
                    &device_context.logical_device,
                    command_buffer,
                    image,
                    ash::vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    ash::vk::AccessFlags::TRANSFER_READ,
                    ash::vk::AccessFlags::SHADER_READ,
                    ash::vk::PipelineStageFlags::TRANSFER,
                    ash::vk::PipelineStageFlags::FRAGMENT_SHADER
                        | ash::vk::PipelineStageFlags::COMPUTE_SHADER,
                    ash::vk::ImageAspectFlags::COLOR,
                    previous_mip_level,
                    1,
                    1,
                );
            }
        }
        let base_mip_level = match generate_mip_levels {
            true => options.mip_level_count - 1,
            false => 0,
        };
        Self::transition_image_layout(
            &device_context.logical_device,
            command_buffer,
            image,
            ash::vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ash::vk::AccessFlags::TRANSFER_WRITE,
            ash::vk::AccessFlags::SHADER_READ,
            ash::vk::PipelineStageFlags::TRANSFER,
            ash::vk::PipelineStageFlags::FRAGMENT_SHADER
                | ash::vk::PipelineStageFlags::COMPUTE_SHADER,
            ash::vk::ImageAspectFlags::COLOR,
            base_mip_level,
            options.mip_level_count - base_mip_level,
            1,
        );

        current_frame.transfer_buffers.push(VulkanBuffer {
            buffer: staging_buffer,
            vma_allocation: staging_allocation,
            size: staging_size,
            usage: BufferUsage::Uniform,
        });
        current_frame.base.recorded = true;

        Ok(Box::new(VulkanTexture {
            image,
            image_view,
            vma_allocation: image_allocation,
            texture_options: *options,
        }))
    }

    fn destroy_texture(&self, texture: &mut dyn RendererTexture) -> Result<()> {
        let texture = resource_mut::<VulkanTexture>(texture)?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
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

    fn create_sampler(&self, options: &SamplerOptions) -> Result<Box<dyn RendererSampler>> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let (anisotropy_enable, max_anisotropy) = match options.max_anisotropy {
            Some(max_anisotropy) => {
                if max_anisotropy < 1.0 {
                    return Err(Error::invalid_argument(
                        "Maximum anisotropy must be at least 1.0.",
                    ));
                }
                if !device_context
                    .enabled_features
                    .contains(&GraphicsDeviceFeature::AnisotropicFiltering)
                {
                    return Err(Error::invalid_operation(
                        "Anisotropic filtering requires the `AnisotropicFiltering` feature to be enabled.",
                    ));
                }
                (
                    true,
                    max_anisotropy.min(device_context.graphics_device.max_sampler_anisotropy),
                )
            }
            None => (false, 1.0),
        };
        let create_info = ash::vk::SamplerCreateInfo::default()
            .mag_filter(options.mag_filter.into())
            .min_filter(options.min_filter.into())
            .mipmap_mode(options.mipmap_filter.into())
            .address_mode_u(options.address_mode_u.into())
            .address_mode_v(options.address_mode_v.into())
            .address_mode_w(options.address_mode_w.into())
            .min_lod(0.0)
            .max_lod(ash::vk::LOD_CLAMP_NONE)
            .anisotropy_enable(anisotropy_enable)
            .max_anisotropy(max_anisotropy)
            .compare_enable(options.compare_op.is_some())
            .compare_op(
                options
                    .compare_op
                    .map(|op| op.into())
                    .unwrap_or(ash::vk::CompareOp::LESS),
            );
        let sampler = unsafe {
            device_context
                .logical_device
                .create_sampler(&create_info, None)?
        };
        Ok(Box::new(VulkanSampler { sampler }))
    }

    fn destroy_sampler(&self, sampler: &mut dyn RendererSampler) -> Result<()> {
        let sampler = resource_mut::<VulkanSampler>(sampler)?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        unsafe {
            device_context
                .logical_device
                .destroy_sampler(sampler.sampler, None);
        }
        sampler.sampler = ash::vk::Sampler::null();
        Ok(())
    }

    fn create_compute_pipeline(&self, shader: &Shader) -> Result<Box<dyn RendererComputePipeline>> {
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
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let (prefix, code_u32, suffix) = unsafe { shader.data.align_to::<u32>() };
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(Error::Fail(format!(
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
        let shader_stage = shader.metadata.stage.into();

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

        Ok(Box::new(VulkanComputePipeline {
            pipeline,
            layout,
            descriptor_layouts,
            descriptor_binding_counts: binding_sets.iter().map(Vec::len).collect(),
            descriptor_set_cache: DescriptorSetCache::new(),
        }))
    }

    fn destroy_compute_pipeline(&self, pipeline: &mut dyn RendererComputePipeline) -> Result<()> {
        let pipeline = resource_mut::<VulkanComputePipeline>(pipeline)?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
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
        pipeline.pipeline = ash::vk::Pipeline::null();
        pipeline.layout = ash::vk::PipelineLayout::null();
        pipeline.descriptor_layouts.clear();
        Ok(())
    }

    fn record_compute_command(
        &mut self,
        pipeline: &mut dyn RendererComputePipeline,
        binding_sets: &[&[ResourceBinding<'_>]],
        group_count: (u32, u32, u32),
    ) -> Result<()> {
        let pipeline = resource_mut::<VulkanComputePipeline>(pipeline)?;
        let mapped_range = self.create_compute_resource_sets(pipeline, binding_sets)?;

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let compute_queue_context =
            device_context
                .compute_queue_context
                .as_mut()
                .ok_or(Error::invalid_operation(
                    "Device is not initialized with the `ComputeShaders` feature.",
                ))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut compute_queue_context.frames[device_context.current_frame_index]
            .base
            .thread_contexts[thread_context_index];

        unsafe {
            if current_frame.bound_pipeline != Some(pipeline.pipeline) {
                device_context.logical_device.cmd_bind_pipeline(
                    current_frame.base.command_buffer,
                    PipelineBindPoint::COMPUTE,
                    pipeline.pipeline,
                );
                current_frame.bound_pipeline = Some(pipeline.pipeline);
            }
            if !mapped_range.is_empty() {
                device_context.logical_device.cmd_bind_descriptor_sets(
                    current_frame.base.command_buffer,
                    PipelineBindPoint::COMPUTE,
                    pipeline.layout,
                    0,
                    &pipeline.descriptor_set_cache.descriptor_sets[mapped_range],
                    &[],
                );
            }
            device_context.logical_device.cmd_dispatch(
                current_frame.base.command_buffer,
                group_count.0,
                group_count.1,
                group_count.2,
            );
        }

        current_frame.base.recorded = true;
        Ok(())
    }

    fn create_graphics_pipeline(
        &self,
        shaders: &[&Shader],
        options: &GraphicsPipelineOptions,
    ) -> Result<Box<dyn RendererGraphicsPipeline>> {
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        Ok(Box::new(rendering.create_graphics_pipeline(
            &device_context.logical_device,
            shaders,
            options,
            &device_context.enabled_features,
            device_context.max_tessellation_patch_size,
        )?))
    }

    fn destroy_graphics_pipeline(&self, pipeline: &mut dyn RendererGraphicsPipeline) -> Result<()> {
        let pipeline = resource_mut::<VulkanGraphicsPipeline>(pipeline)?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        rendering.destroy_graphics_pipeline(&device_context.logical_device, pipeline)
    }

    fn record_graphics_command(
        &mut self,
        pipeline: &mut dyn RendererGraphicsPipeline,
        index_buffer: Option<&dyn RendererBuffer>,
        vertex_bindings: &[ResourceBinding<'_>],
        binding_sets: &[&[ResourceBinding<'_>]],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        self.record_draw(
            None,
            pipeline,
            index_buffer,
            vertex_bindings,
            binding_sets,
            draw_count,
            instance_count,
        )
    }

    fn create_canvas(&mut self, options: &CanvasOptions) -> Result<Box<dyn RendererCanvas>> {
        if options.width == 0 || options.height == 0 {
            return Err(Error::invalid_argument(
                "Canvas dimensions must be greater than zero.",
            ));
        }

        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        {
            let state = device_context.graphics_queue_context.frames
                [device_context.current_frame_index]
                .base
                .thread_contexts[thread_context_index]
                .base
                .sync_state
                .0
                .lock()?;
            if *state != FrameState::Started {
                return Err(Error::invalid_operation(
                    "Cannot create a canvas outside of a frame.",
                ));
            }
        }

        let limits = unsafe {
            instance
                .get_physical_device_properties(device_context.physical_device)
                .limits
        };
        let max_width = limits
            .max_image_dimension2_d
            .min(limits.max_framebuffer_width);
        let max_height = limits
            .max_image_dimension2_d
            .min(limits.max_framebuffer_height);
        if options.width > max_width || options.height > max_height {
            return Err(Error::InvalidArgument(format!(
                "Canvas dimensions cannot exceed {max_width}x{max_height}."
            )));
        }

        let layer_count = self.settings.array_layer_count();
        let mut canvas: Box<dyn VulkanCanvas> = match options.depth_only {
            true => Box::new(VulkanDepthCanvas::new(
                device_context,
                options,
                layer_count,
            )?),
            false => Box::new(VulkanColorCanvas::new(
                device_context,
                options,
                self.settings.msaa,
                layer_count,
            )?),
        };

        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let current_frame = &mut device_context.graphics_queue_context.frames
            [device_context.current_frame_index]
            .base
            .thread_contexts[thread_context_index];
        if let Err(error) = Self::clear_new_canvas(
            &device_context.logical_device,
            rendering.as_mut(),
            current_frame,
            canvas.as_ref(),
            device_context.sampling_stages,
        ) {
            canvas.destroy(device_context);
            return Err(error);
        }

        Ok(canvas)
    }

    fn destroy_canvas(&self, canvas: &mut dyn RendererCanvas) -> Result<()> {
        let canvas = vulkan_canvas_mut(canvas)?;
        if canvas.texture().is_allocated() {
            let device_context = self
                .device_context
                .as_ref()
                .ok_or(Error::invalid_operation("Device is not set."))?;
            canvas.destroy(device_context);
        }
        Ok(())
    }

    fn record_canvas_command(
        &mut self,
        canvas: &mut dyn RendererCanvas,
        pipeline: &mut dyn RendererGraphicsPipeline,
        index_buffer: Option<&dyn RendererBuffer>,
        vertex_bindings: &[ResourceBinding<'_>],
        binding_sets: &[&[ResourceBinding<'_>]],
        draw_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        self.record_draw(
            Some(vulkan_canvas_mut(canvas)?),
            pipeline,
            index_buffer,
            vertex_bindings,
            binding_sets,
            draw_count,
            instance_count,
        )
    }

    fn begin_frame(&mut self) -> Result<()> {
        let p_self = self as *mut Self;
        let is_in_main_thread = self.is_in_main_thread();
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;

        if is_in_main_thread {
            if let Some(max_fps) = self.settings.max_fps {
                const SPIN_DURATION: Duration = Duration::from_millis(1);
                let frame_duration = Duration::from_secs(1) / max_fps;
                let now = Instant::now();
                device_context.next_frame_deadline = match device_context.next_frame_deadline {
                    Some(deadline) if deadline > now => {
                        if let Some(sleep_duration) = (deadline - now).checked_sub(SPIN_DURATION) {
                            std::thread::sleep(sleep_duration);
                        }
                        while Instant::now() < deadline {
                            std::hint::spin_loop();
                        }
                        Some(deadline + frame_duration)
                    }
                    _ => Some(now + frame_duration),
                };
            }

            // Wait for current frame to finish.
            fn wait_frame_sync<Frame: VulkanFrame>(
                logical_device: &ash::Device,
                queue_context: &mut QueueContext<Frame>,
                frame_index: usize,
            ) -> Result<()> {
                let frame_sync = queue_context
                    .frame_sync
                    .as_mut()
                    .ok_or(Error::invalid_operation("Device is not set."))?;
                frame_sync.wait(logical_device, frame_index)
            }
            wait_frame_sync(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                device_context.current_frame_index,
            )?;
            wait_frame_sync(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                device_context.current_frame_index,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                wait_frame_sync(
                    &device_context.logical_device,
                    compute_queue_context,
                    device_context.current_frame_index,
                )?;
            }

            Self::acquire_swapchain_image(p_self)?;
        } else {
            // The worker thread might be one frame ahead of the main thread. Wait for main to
            // consume the previous frame, releasing the index lock while waiting so main can
            // finish. Reacquiring the lock ensures `current_frame_index` has also been updated
            // before we wait for main to begin the next frame.
            let mut _mutex_guard = device_context.current_frame_index_sync.lock()?;
            let (state_mutex, cv) = &device_context.graphics_queue_context.frames
                [device_context.current_frame_index]
                .base
                .thread_contexts[thread_context_index]
                .base
                .sync_state;
            let mut state = state_mutex.lock()?;
            if *state == FrameState::Finished {
                drop(_mutex_guard);
                while *state == FrameState::Finished {
                    state = cv.wait(state)?;
                }
                drop(state);
                _mutex_guard = device_context.current_frame_index_sync.lock()?;
            }

            // Wait for the main thread to begin the current frame before we can start recording
            // commands.
            fn wait_main_thread_begin_current_frame<Frame: VulkanFrame>(
                queue_context: &QueueContext<Frame>,
                frame_index: usize,
            ) -> Result<()> {
                let main_frame = &queue_context.frames[frame_index].base().thread_contexts
                    [MAIN_THREAD_CONTEXT_INDEX];
                let (state_mutex, cv) = &main_frame.base().sync_state;
                let mut state = state_mutex.lock()?;
                while *state != FrameState::Started {
                    state = cv.wait(state)?;
                }
                Ok(())
            }
            wait_main_thread_begin_current_frame(
                &device_context.graphics_queue_context,
                device_context.current_frame_index,
            )?;
            wait_main_thread_begin_current_frame(
                &device_context.transfer_queue_context,
                device_context.current_frame_index,
            )?;
            if let Some(compute_queue_context) = &device_context.compute_queue_context {
                wait_main_thread_begin_current_frame(
                    compute_queue_context,
                    device_context.current_frame_index,
                )?;
            }
        }

        fn begin_queue<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            vma_allocator: &vk_mem::Allocator,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
        ) -> Result<()> {
            let current_frame = &mut queue_context.frames[frame_index].base_mut().thread_contexts
                [thread_context_index];
            if *current_frame.base().sync_state.0.lock()? != FrameState::Idle {
                return Err(Error::invalid_operation(
                    "Invalid frame state, did you forget to call `end_frame`?",
                ));
            }
            unsafe {
                logical_device.reset_command_pool(
                    current_frame.base().command_pool,
                    ash::vk::CommandPoolResetFlags::empty(),
                )?;
                if let Some(current_frame) =
                    <dyn Any>::downcast_mut::<TransferThreadContext>(current_frame)
                {
                    for transfer_buffer in &mut current_frame.transfer_buffers {
                        vma_allocator.destroy_buffer(
                            transfer_buffer.buffer,
                            &mut transfer_buffer.vma_allocation,
                        )
                    }
                    current_frame.transfer_buffers.clear();
                }

                let begin_info = CommandBufferBeginInfo::default()
                    .flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
                logical_device
                    .begin_command_buffer(current_frame.base().command_buffer, &begin_info)?;
                if let Some(current_frame) =
                    <dyn Any>::downcast_mut::<GraphicsThreadContext>(current_frame)
                {
                    current_frame.bound_pipeline = None;
                    current_frame.render_target = None;
                } else if let Some(current_frame) =
                    <dyn Any>::downcast_mut::<ComputeThreadContext>(current_frame)
                {
                    current_frame.bound_pipeline = None;
                }
            }
            let (state_mutex, cv) = &current_frame.base().sync_state;
            *state_mutex.lock()? = FrameState::Started;
            cv.notify_all();
            Ok(())
        }
        begin_queue(
            &device_context.logical_device,
            &device_context.vma_allocator,
            &mut device_context.graphics_queue_context,
            device_context.current_frame_index,
            thread_context_index,
        )?;
        begin_queue(
            &device_context.logical_device,
            &device_context.vma_allocator,
            &mut device_context.transfer_queue_context,
            device_context.current_frame_index,
            thread_context_index,
        )?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            begin_queue(
                &device_context.logical_device,
                &device_context.vma_allocator,
                compute_queue_context,
                device_context.current_frame_index,
                thread_context_index,
            )?;
        }

        if is_in_main_thread && device_context.display_canvas.is_none() {
            let extent = device_context.swapchain_context.extent;
            let display_canvas: Box<dyn Any> = self.create_canvas(&CanvasOptions {
                width: extent.width,
                height: extent.height,
                depth_only: false,
            })?;
            let display_canvas = display_canvas
                .downcast::<VulkanColorCanvas>()
                .map_err(|_| Error::fail("Failed to create the display canvas."))?;
            self.device_context
                .as_mut()
                .ok_or(Error::invalid_operation("Device is not set."))?
                .display_canvas = Some(*display_canvas);
        }

        Ok(())
    }

    fn end_frame(&mut self) -> Result<()> {
        let is_in_main_thread = self.is_in_main_thread();
        let device_context = self
            .device_context
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let thread_context_index = Self::thread_context_index()?;
        let image_index = device_context.swapchain_context.current_image_index;
        let rendering = device_context
            .rendering
            .as_mut()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        let _mutex_guard = match is_in_main_thread {
            true => Some(device_context.current_frame_index_sync.lock()?),
            false => None,
        };

        let mut present = false;
        fn end_queue<Frame: VulkanFrame>(
            logical_device: &ash::Device,
            rendering: &mut dyn VulkanRendering,
            display_canvas: Option<&VulkanColorCanvas>,
            swapchain_images: &[ash::vk::Image],
            image_index: usize,
            layer_count: u32,
            sampling_stages: ash::vk::PipelineStageFlags,
            is_in_main_thread: bool,
            queue_context: &mut QueueContext<Frame>,
            frame_index: usize,
            thread_context_index: usize,
            present: &mut bool,
        ) -> Result<()> {
            let current_frame = &mut queue_context.frames[frame_index].base_mut().thread_contexts
                [thread_context_index];
            if *current_frame.base().sync_state.0.lock()? != FrameState::Started {
                return Err(Error::invalid_operation(
                    "Cannot end a frame that hasn't started yet.",
                ));
            }
            unsafe {
                if let Some(current_frame) =
                    <dyn Any>::downcast_mut::<GraphicsThreadContext>(current_frame)
                {
                    if let Some((image, aspect_mask)) = current_frame.render_target.take() {
                        VulkanRenderer::end_rendering(
                            logical_device,
                            rendering,
                            current_frame.base.command_buffer,
                            image,
                            aspect_mask,
                            layer_count,
                            sampling_stages,
                        )?;
                    }
                    if is_in_main_thread {
                        let display_canvas =
                            display_canvas.ok_or(Error::invalid_operation("Device is not set."))?;
                        VulkanRenderer::copy_display_canvas(
                            logical_device,
                            current_frame.base.command_buffer,
                            display_canvas,
                            swapchain_images[image_index],
                            layer_count,
                            sampling_stages,
                        );
                        current_frame.base.recorded = true;
                        *present = true;
                    }
                }
                logical_device.end_command_buffer(current_frame.base().command_buffer)?;
            }
            let (state_mutex, cv) = &current_frame.base().sync_state;
            *state_mutex.lock()? = FrameState::Finished;
            cv.notify_one();
            Ok(())
        }
        end_queue(
            &device_context.logical_device,
            rendering.as_mut(),
            device_context.display_canvas.as_ref(),
            &device_context.swapchain_context.images,
            image_index,
            self.settings.array_layer_count(),
            device_context.sampling_stages,
            is_in_main_thread,
            &mut device_context.graphics_queue_context,
            device_context.current_frame_index,
            thread_context_index,
            &mut present,
        )?;
        end_queue(
            &device_context.logical_device,
            rendering.as_mut(),
            device_context.display_canvas.as_ref(),
            &device_context.swapchain_context.images,
            image_index,
            self.settings.array_layer_count(),
            device_context.sampling_stages,
            is_in_main_thread,
            &mut device_context.transfer_queue_context,
            device_context.current_frame_index,
            thread_context_index,
            &mut present,
        )?;
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            end_queue(
                &device_context.logical_device,
                rendering.as_mut(),
                device_context.display_canvas.as_ref(),
                &device_context.swapchain_context.images,
                image_index,
                self.settings.array_layer_count(),
                device_context.sampling_stages,
                is_in_main_thread,
                compute_queue_context,
                device_context.current_frame_index,
                thread_context_index,
                &mut present,
            )?;
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

            let mut graphics_command_buffers = Vec::new();
            let mut transfer_command_buffers = Vec::new();
            let mut compute_command_buffers = Vec::new();
            let masks = device_context.thread_context_masks.lock()?;
            for thread_context_index in 0..super::thread_context::thread_context_count() {
                if !is_thread_context_active(thread_context_index, &masks) {
                    continue;
                }

                fn add_command_buffer<Frame: VulkanFrame>(
                    queue_context: &mut QueueContext<Frame>,
                    frame_index: usize,
                    thread_context_index: usize,
                    command_buffers: &mut Vec<ash::vk::CommandBuffer>,
                ) -> Result<()> {
                    let current_frame = queue_context.frames[frame_index]
                        .base_mut()
                        .thread_contexts[thread_context_index]
                        .base_mut();

                    // Wait for the worker thread to signal the frame is finished.
                    let (state_mutex, cv) = &current_frame.sync_state;
                    let mut state = state_mutex.lock()?;
                    if *state == FrameState::Idle {
                        return Err(Error::invalid_operation(
                            "Invalid frame state, did you forget to call `begin_frame`?",
                        ));
                    }
                    while *state != FrameState::Finished {
                        state = cv.wait(state)?;
                    }
                    *state = FrameState::Idle;
                    cv.notify_one();

                    if current_frame.recorded {
                        command_buffers.push(current_frame.command_buffer);
                        current_frame.recorded = false;
                    }

                    Ok(())
                }
                add_command_buffer(
                    &mut device_context.graphics_queue_context,
                    device_context.current_frame_index,
                    thread_context_index,
                    &mut graphics_command_buffers,
                )?;
                add_command_buffer(
                    &mut device_context.transfer_queue_context,
                    device_context.current_frame_index,
                    thread_context_index,
                    &mut transfer_command_buffers,
                )?;
                if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                    add_command_buffer(
                        compute_queue_context,
                        device_context.current_frame_index,
                        thread_context_index,
                        &mut compute_command_buffers,
                    )?;
                }
            }

            fn submit_commands<Frame: VulkanFrame>(
                logical_device: &ash::Device,
                queue_context: &mut QueueContext<Frame>,
                frame_index: usize,
                command_buffers: &[ash::vk::CommandBuffer],
                wait_semaphores: &[ash::vk::Semaphore],
                wait_stages: &[ash::vk::PipelineStageFlags],
                signal_semaphores: &[ash::vk::Semaphore],
            ) -> Result<()> {
                // Submit even without commands when there is a wait, so the semaphore is
                // unsignaled.
                if !command_buffers.is_empty() || !wait_semaphores.is_empty() {
                    let submit_info = SubmitInfo::default()
                        .command_buffers(command_buffers)
                        .wait_semaphores(wait_semaphores)
                        .wait_dst_stage_mask(wait_stages)
                        .signal_semaphores(signal_semaphores);
                    let frame_sync = queue_context
                        .frame_sync
                        .as_mut()
                        .ok_or(Error::invalid_operation("Device is not set."))?;
                    frame_sync.submit(
                        logical_device,
                        frame_index,
                        queue_context.queue,
                        &[submit_info],
                    )?;
                }
                Ok(())
            }
            let transfer_semaphore = match transfer_command_buffers.is_empty() {
                true => None,
                false => {
                    device_context.transfer_queue_context.frames[device_context.current_frame_index]
                        .semaphore
                }
            };
            let mut graphics_wait_semaphores = Vec::with_capacity(2);
            let mut graphics_wait_stages = Vec::with_capacity(2);
            if present {
                graphics_wait_semaphores.extend_from_slice(&image_available_semaphores);
                graphics_wait_stages.push(ash::vk::PipelineStageFlags::TRANSFER);
            }
            if let Some(transfer_semaphore) = transfer_semaphore {
                graphics_wait_semaphores.push(transfer_semaphore);
                graphics_wait_stages.push(ash::vk::PipelineStageFlags::ALL_COMMANDS);
            }
            let graphics_signal_semaphores: &[ash::vk::Semaphore] = match present {
                true => &render_finished_semaphores,
                false => &[],
            };

            submit_commands(
                &device_context.logical_device,
                &mut device_context.transfer_queue_context,
                device_context.current_frame_index,
                &transfer_command_buffers,
                &[],
                &[],
                transfer_semaphore.as_slice(),
            )?;
            submit_commands(
                &device_context.logical_device,
                &mut device_context.graphics_queue_context,
                device_context.current_frame_index,
                &graphics_command_buffers,
                &graphics_wait_semaphores,
                &graphics_wait_stages,
                graphics_signal_semaphores,
            )?;
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                submit_commands(
                    &device_context.logical_device,
                    compute_queue_context,
                    device_context.current_frame_index,
                    &compute_command_buffers,
                    &[],
                    &[],
                    &[],
                )?;
            }

            if present {
                let wait_semaphores = [device_context.swapchain_context.semaphores[image_index].1];
                let swapchains = [device_context.swapchain_context.swapchain];
                let image_indices = [image_index as u32];
                let present_info = ash::vk::PresentInfoKHR::default()
                    .wait_semaphores(&wait_semaphores)
                    .swapchains(&swapchains)
                    .image_indices(&image_indices);
                let result = unsafe {
                    device_context
                        .swapchain_context
                        .loader
                        .queue_present(device_context.graphics_queue_context.queue, &present_info)
                };
                match result {
                    Ok(suboptimal) => device_context.swapchain_context.invalidated |= suboptimal,
                    Err(ash::vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        device_context.swapchain_context.invalidated = true
                    }
                    Err(error) => return Err(error.into()),
                }
                device_context
                    .swapchain_context
                    .image_avaliable_semaphore_index = SwapchainContext::INVALID_INDEX;
            }

            device_context.current_frame_index =
                (device_context.current_frame_index + 1) % self.settings.frames_in_flight;
        }

        Ok(())
    }

    fn wait_idle(&self) -> Result<()> {
        self.main_thread_only()?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;

        unsafe {
            device_context.logical_device.device_wait_idle()?;
        }
        Ok(())
    }

    fn clear(&mut self, color: RGB<f32>) -> Result<()> {
        self.record_clear(None, color)
    }

    fn clear_canvas(&mut self, canvas: &mut dyn RendererCanvas, color: RGB<f32>) -> Result<()> {
        self.record_clear(Some(vulkan_canvas_mut(canvas)?), color)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        if let Err(e) = self.uninitialize() {
            eprintln!("Failed to uninitialize renderer on drop: {}", e);
        }
    }
}
