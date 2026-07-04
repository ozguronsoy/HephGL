use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};

use ash::vk::{
    ApplicationInfo, ClearColorValue, ClearValue, InstanceCreateInfo, MemoryHeapFlags,
    PhysicalDeviceFeatures2, PhysicalDeviceMemoryProperties2, PhysicalDeviceProperties2,
    PhysicalDeviceType, QueueFamilyProperties2, QueueFlags, StructureType, SurfaceKHR,
};
use ash::{Entry, Instance};
use renkrs::RGB;

use crate::graphics_device::GraphicsDevice;
use crate::renderers::{InitializeOptions, Renderer};
use crate::{HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version};

#[derive(Debug, Default)]
struct QueueFamilyIndices {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub transfer: Option<u32>,
    pub video_decode: Option<u32>,
    pub video_encode: Option<u32>,
    pub optical_flow: Option<u32>,
    pub present: Option<u32>,
}

pub struct VulkanRenderer {
    entry: Option<Entry>,
    instance: Option<Instance>,

    window_surface: Option<SurfaceKHR>,
    window_surface_loader: Option<ash::khr::surface::Instance>,

    queue_index_cache: HashMap<u32, QueueFamilyIndices>,

    device: Option<GraphicsDevice>,
}

impl QueueFamilyIndices {
    #[inline]
    pub fn graphics(&self) -> u32 {
        self.graphics.expect("Graphics queue not found.")
    }

    #[inline]
    pub fn compute(&self) -> u32 {
        self.compute.expect("Compute queue not found.")
    }

    #[inline]
    pub fn transfer(&self) -> u32 {
        self.transfer.expect("Transfer queue not found.")
    }

    #[inline]
    pub fn video_decode(&self) -> u32 {
        self.video_decode.expect("Video decode queue not found.")
    }

    #[inline]
    pub fn video_encode(&self) -> u32 {
        self.video_encode.expect("Video encode queue not found.")
    }

    #[inline]
    pub fn optical_flow(&self) -> u32 {
        self.optical_flow.expect("Optical flow queue not found.")
    }

    #[inline]
    pub fn present(&self) -> u32 {
        self.present.expect("Present queue not found.")
    }

    pub fn is_complete(&self) -> bool {
        // We only check graphics, compute, and present for completion,
        // as transfer can implicitly fallback to the graphics queue,
        // and video/optical flow are highly specialized hardware features.
        self.graphics.is_some() && self.compute.is_some() && self.present.is_some()
    }
}

impl VulkanRenderer {
    #[inline]
    fn entry(&self) -> &Entry {
        self.entry
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Entry.")
    }

    #[inline]
    fn instance(&self) -> &Instance {
        self.instance
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Instance.")
    }

    #[inline]
    fn window_surface(&self) -> &SurfaceKHR {
        self.window_surface
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Window Surface.")
    }

    #[inline]
    fn window_surface_loader(&self) -> &ash::khr::surface::Instance {
        self.window_surface_loader
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Window Surface Loader.")
    }

    fn vk_api_version_to_heph_version(vk_api_version: u32) -> Version {
        Version {
            major: ash::vk::api_version_major(vk_api_version),
            minor: ash::vk::api_version_minor(vk_api_version),
            patch: ash::vk::api_version_patch(vk_api_version),
        }
    }

    fn vk_driver_version_to_heph_version(
        vk_vendor: crate::graphics_device::Vendor,
        vk_driver_version: u32,
    ) -> Version {
        match vk_vendor {
            crate::graphics_device::Vendor::Nvidia => Version {
                major: (vk_driver_version >> 22) & 0x3FF,
                minor: (vk_driver_version >> 14) & 0x0FF,
                patch: (vk_driver_version >> 6) & 0x0FF,
            },

            crate::graphics_device::Vendor::Intel => Version {
                major: vk_driver_version >> 14,
                minor: vk_driver_version & 0x3FFF,
                patch: 0,
            },

            _ => Version {
                major: ash::vk::api_version_major(vk_driver_version),
                minor: ash::vk::api_version_minor(vk_driver_version),
                patch: ash::vk::api_version_patch(vk_driver_version),
            },
        }
    }
}

impl Renderer for VulkanRenderer {
    fn new() -> Self {
        Self {
            entry: None,
            instance: None,

            window_surface: None,
            window_surface_loader: None,

            queue_index_cache: HashMap::default(),

            device: None,
        }
    }

    fn initialize(&mut self, options: &InitializeOptions) {
        if self.entry.is_some() || self.instance.is_some() {
            panic!("VulkanRenderer is already initialized.");
        }

        // Create instance.

        let c_app_name =
            CString::new(options.app_name).expect("App name cannot contain null bytes");
        let required_extension_names =
            ash_window::enumerate_required_extensions(options.display_handle)
                .expect("Failed to enumerate WSI extensions");
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
            api_version: ash::vk::make_api_version(0, 1, 4, 0),
            ..Default::default()
        };
        let instance_create_info = InstanceCreateInfo {
            s_type: StructureType::INSTANCE_CREATE_INFO,
            p_application_info: &app_info,
            enabled_extension_count: required_extension_names.len() as u32,
            pp_enabled_extension_names: required_extension_names.as_ptr(),
            ..Default::default()
        };

        self.entry =
            Some(unsafe { Entry::load().expect("Failed to load Vulkan graphics driver library") });
        self.instance = Some(unsafe {
            self.entry()
                .create_instance(&instance_create_info, None)
                .expect("Failed to create Vulkan Instance")
        });

        // Enumerate supported features.

        // WSI for rendering to the native window.

        self.window_surface = Some(unsafe {
            ash_window::create_surface(
                self.entry(),
                self.instance(),
                options.display_handle,
                options.window_handle,
                None,
            )
            .expect("Failed to create WSI Surface")
        });
        self.window_surface_loader = Some(ash::khr::surface::Instance::new(
            self.entry(),
            self.instance(),
        ));
    }

    fn uninitialize(&mut self) {
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
    }

    fn enumerate_devices(&mut self) -> Vec<GraphicsDevice> {
        let mut devices = Vec::<GraphicsDevice>::new();
        let physical_devices = unsafe {
            self.instance()
                .enumerate_physical_devices()
                .expect("Failed to enumerate physical Vulkan devices.")
        };

        for physical_device in physical_devices {
            let mut properties2 = PhysicalDeviceProperties2::default();
            let mut memory_properties2 = PhysicalDeviceMemoryProperties2::default();
            let mut physical_features2 = PhysicalDeviceFeatures2::default();
            let mut queue_family_properties2 = Vec::<QueueFamilyProperties2>::default();
            unsafe {
                self.instance()
                    .get_physical_device_properties2(physical_device, &mut properties2);
                self.instance().get_physical_device_memory_properties2(
                    physical_device,
                    &mut memory_properties2,
                );
                self.instance()
                    .get_physical_device_features2(physical_device, &mut physical_features2);

                let queue_family_properties2_size = self
                    .instance()
                    .get_physical_device_queue_family_properties2_len(physical_device);
                queue_family_properties2.resize(
                    queue_family_properties2_size,
                    QueueFamilyProperties2::default(),
                );
                self.instance()
                    .get_physical_device_queue_family_properties2(
                        physical_device,
                        &mut queue_family_properties2,
                    );
            };

            let device_name = unsafe {
                CStr::from_ptr(properties2.properties.device_name.as_ptr())
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
            self.queue_index_cache
                .insert(device_id, QueueFamilyIndices::default());

            let device_api_version =
                VulkanRenderer::vk_api_version_to_heph_version(properties2.properties.api_version);
            let device_driver_version = VulkanRenderer::vk_driver_version_to_heph_version(
                GraphicsDevice::vendor_from_id(properties2.properties.vendor_id),
                properties2.properties.driver_version,
            );

            // VRAM is the sum of the sizes of all DEVICE_LOCAL heaps
            let mut device_vram: u64 = 0;
            let heap_count = memory_properties2.memory_properties.memory_heap_count as usize;
            for heap in memory_properties2.memory_properties.memory_heaps[..heap_count].iter() {
                if heap.flags.contains(MemoryHeapFlags::DEVICE_LOCAL) {
                    device_vram += heap.size;
                }
            }

            let mut supported_features = HashSet::<crate::graphics_device::Feature>::default();
            let extension_properties = unsafe {
                self.instance()
                    .enumerate_device_extension_properties(physical_device)
                    .expect("Failed to enumerate the supported device features.")
            };
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
                let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
                if name.to_string_lossy() == "VK_KHR_ray_tracing_pipeline" {
                    supported_features.insert(crate::graphics_device::Feature::RayTracing);
                }
            }
            for (index, queue_family) in queue_family_properties2.iter().enumerate() {
                let queue_flags = queue_family.queue_family_properties.queue_flags;
                if queue_flags.contains(QueueFlags::COMPUTE) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.compute = Some(index as u32)
                        });
                    supported_features.insert(crate::graphics_device::Feature::ComputeShaders);
                }
                if queue_flags.contains(QueueFlags::VIDEO_DECODE_KHR) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.video_decode = Some(index as u32)
                        });
                    supported_features.insert(crate::graphics_device::Feature::VideoDecoding);
                }
                if queue_flags.contains(QueueFlags::VIDEO_ENCODE_KHR) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.video_encode = Some(index as u32)
                        });
                    supported_features.insert(crate::graphics_device::Feature::VideoEncoding);
                }
                if queue_flags.contains(QueueFlags::OPTICAL_FLOW_NV) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.optical_flow = Some(index as u32)
                        });
                    supported_features.insert(crate::graphics_device::Feature::OpticalFlow);
                }

                if queue_flags.contains(QueueFlags::GRAPHICS) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.graphics = Some(index as u32)
                        });
                }
                if queue_flags.contains(QueueFlags::TRANSFER) {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.transfer = Some(index as u32)
                        });
                }

                let is_present_supported = unsafe {
                    self.window_surface_loader()
                        .get_physical_device_surface_support(
                            physical_device,
                            index as u32,
                            *self.window_surface(),
                        )
                        .expect("Failed to query presentation support for queue family")
                };
                if is_present_supported {
                    self.queue_index_cache
                        .entry(device_id)
                        .and_modify(|queue_family_indices| {
                            queue_family_indices.present = Some(index as u32);
                        });
                }
            }

            devices.push(GraphicsDevice {
                name: device_name,
                device_type: device_type,
                vendor_id: device_vendor_id,
                device_id: device_id,
                api_version: device_api_version,
                driver_version: device_driver_version,
                vram: device_vram,
                supported_features: supported_features,
            });
        }

        devices
    }

    fn get_device(&self) -> &Option<GraphicsDevice> {
        &self.device
    }

    fn set_device(&mut self, device: &GraphicsDevice) {
        if self.device.is_some() {
            // TODO: uninitialize old device.
        }

        self.device = Some(device.clone());
    }

    fn clear(&mut self, color: RGB<f32>) {
        let clear_value = ClearValue {
            color: ClearColorValue {
                float32: [color.r, color.g, color.b, 1.0],
            },
        };

        // TODO
    }
}
