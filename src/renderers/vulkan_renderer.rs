use std::ffi::{CStr, CString};

use ash::vk::{
    ApplicationInfo, ClearColorValue, ClearValue, InstanceCreateInfo, MemoryHeapFlags,
    PhysicalDeviceMemoryProperties2, PhysicalDeviceProperties2, PhysicalDeviceType, StructureType,
    SurfaceKHR,
};
use ash::{Entry, Instance};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::graphics_device::{GraphicsDevice, GraphicsDeviceType, GraphicsDeviceVendor};
use crate::renderers::Renderer;
use crate::{HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version};

pub struct VulkanRenderer {
    entry: Option<Entry>,
    instance: Option<Instance>,
    window_surface: Option<SurfaceKHR>,
    window_surface_loader: Option<ash::khr::surface::Instance>,
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
        vk_vendor: GraphicsDeviceVendor,
        vk_driver_version: u32,
    ) -> Version {
        match vk_vendor {
            GraphicsDeviceVendor::Nvidia => Version {
                major: (vk_driver_version >> 22) & 0x3FF,
                minor: (vk_driver_version >> 14) & 0x0FF,
                patch: (vk_driver_version >> 6) & 0x0FF,
            },

            GraphicsDeviceVendor::Intel => Version {
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
        }
    }

    fn initialize(
        &mut self,
        app_name: &str,
        window_handle: RawWindowHandle,
        display_handle: RawDisplayHandle,
    ) {
        if self.entry.is_some() || self.instance.is_some() {
            panic!("VulkanRenderer is already initialized.");
        }

        let c_app_name = CString::new(app_name).expect("App name cannot contain null bytes");
        let required_extension_names = ash_window::enumerate_required_extensions(display_handle)
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

        self.window_surface = Some(unsafe {
            ash_window::create_surface(
                self.entry(),
                self.instance(),
                display_handle,
                window_handle,
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
            unsafe {
                self.instance()
                    .get_physical_device_properties2(physical_device, &mut properties2);
                self.instance().get_physical_device_memory_properties2(
                    physical_device,
                    &mut memory_properties2,
                );
            };

            let device_name = unsafe {
                CStr::from_ptr(properties2.properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };

            let device_type = match properties2.properties.device_type {
                PhysicalDeviceType::DISCRETE_GPU => GraphicsDeviceType::DiscreteGpu,
                PhysicalDeviceType::INTEGRATED_GPU => GraphicsDeviceType::IntegratedGpu,
                PhysicalDeviceType::VIRTUAL_GPU => GraphicsDeviceType::VirtualGpu,
                PhysicalDeviceType::CPU => GraphicsDeviceType::Cpu,
                PhysicalDeviceType::OTHER => GraphicsDeviceType::Other,
                _ => GraphicsDeviceType::Invalid,
            };

            let device_vendor_id = properties2.properties.vendor_id;
            let device_id = properties2.properties.device_id;

            let device_api_version =
                VulkanRenderer::vk_api_version_to_heph_version(properties2.properties.api_version);
            let device_driver_version = VulkanRenderer::vk_driver_version_to_heph_version(
                GraphicsDevice::vendor_from_id(properties2.properties.vendor_id),
                properties2.properties.driver_version,
            );

            // VRAM is the sum of the sizes of all DEVICE_LOCAL heaps
            let mut device_vram: u64 = 0;
            for i in 0..memory_properties2.memory_properties.memory_heap_count as usize {
                let heap = memory_properties2.memory_properties.memory_heaps[i];
                if heap.flags.contains(MemoryHeapFlags::DEVICE_LOCAL) {
                    device_vram += heap.size;
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
            });
        }

        devices
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
