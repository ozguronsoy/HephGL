use std::ffi::{CStr, CString};

use ash::vk::{
    ApplicationInfo, ClearColorValue, ClearValue, InstanceCreateInfo, PhysicalDeviceProperties2,
    PhysicalDeviceType, StructureType,
};
use ash::{Entry, Instance};
use renkrs::RGB;
use winit::window::Window;

use crate::graphics_device::{GraphicsDevice, GraphicsDeviceType};
use crate::renderers::Renderer;
use crate::{
    HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION_MAJOR, HEPHGL_ENGINE_VERSION_MINOR,
    HEPHGL_ENGINE_VERSION_PATCH,
};

pub struct VulkanRenderer {
    entry: Option<Entry>,
    instance: Option<Instance>,
}

impl VulkanRenderer {
    #[inline]
    fn instance(&self) -> &Instance {
        self.instance
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Instance.")
    }

    #[inline]
    fn entry(&self) -> &Entry {
        self.entry
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Entry.")
    }

    #[inline]
    fn vk_version_to_string(vk_version: u32) -> String {
        format!(
            "v{}.{}.{}",
            ash::vk::api_version_major(vk_version),
            ash::vk::api_version_minor(vk_version),
            ash::vk::api_version_patch(vk_version)
        )
    }
}

impl Renderer for VulkanRenderer {
    fn new() -> Self {
        Self {
            entry: None,
            instance: None,
        }
    }

    fn initialize(&mut self, app_name: &str, window: &Window) {
        if self.entry.is_some() {
            panic!("VulkanRenderer is already initialized.");
        }

        let c_app_name = CString::new(app_name).expect("App name cannot contain null bytes");
        let app_info = ApplicationInfo {
            s_type: StructureType::APPLICATION_INFO,
            p_engine_name: HEPHGL_ENGINE_NAME.as_ptr(),
            p_application_name: c_app_name.as_ptr(),
            application_version: ash::vk::make_api_version(
                0,
                HEPHGL_ENGINE_VERSION_MAJOR,
                HEPHGL_ENGINE_VERSION_MINOR,
                HEPHGL_ENGINE_VERSION_PATCH,
            ),
            api_version: ash::vk::make_api_version(0, 1, 4, 0),
            ..Default::default()
        };
        let instance_create_info = InstanceCreateInfo {
            s_type: StructureType::INSTANCE_CREATE_INFO,
            p_application_info: &app_info,
            ..Default::default()
        };

        let active_entry = self.entry.insert(unsafe {
            Entry::load().expect("Failed to load Vulkan graphics driver library")
        });
        self.instance = Some(unsafe {
            active_entry
                .create_instance(&instance_create_info, None)
                .expect("Failed to create Vulkan Instance")
        });

        // TODO: Implement WSI.
    }

    fn enumerate_devices(&mut self) -> Vec<GraphicsDevice> {
        let mut devices = Vec::<GraphicsDevice>::new();
        let physical_devices = unsafe {
            self.instance()
                .enumerate_physical_devices()
                .expect("Failed to enumerate physical Vulkan devices.")
        };
        for physical_device in physical_devices {
            let mut physical_device_properties2 = PhysicalDeviceProperties2::default();
            unsafe {
                self.instance().get_physical_device_properties2(
                    physical_device,
                    &mut physical_device_properties2,
                );
                devices.push(GraphicsDevice {
                    name: CStr::from_ptr(
                        physical_device_properties2.properties.device_name.as_ptr(),
                    )
                    .to_string_lossy()
                    .into_owned(),

                    device_type: match physical_device_properties2.properties.device_type {
                        PhysicalDeviceType::DISCRETE_GPU => GraphicsDeviceType::DiscreteGpu,
                        PhysicalDeviceType::INTEGRATED_GPU => GraphicsDeviceType::IntegratedGpu,
                        PhysicalDeviceType::VIRTUAL_GPU => GraphicsDeviceType::VirtualGpu,
                        PhysicalDeviceType::CPU => GraphicsDeviceType::Cpu,
                        PhysicalDeviceType::OTHER => GraphicsDeviceType::Other,
                        _ => GraphicsDeviceType::Invalid,
                    },

                    vendor_id: physical_device_properties2.properties.vendor_id,
                    device_id: physical_device_properties2.properties.device_id,

                    api_version: VulkanRenderer::vk_version_to_string(
                        physical_device_properties2.properties.api_version,
                    ),
                    driver_version: VulkanRenderer::vk_version_to_string(
                        physical_device_properties2.properties.driver_version,
                    ),
                });
            };
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
