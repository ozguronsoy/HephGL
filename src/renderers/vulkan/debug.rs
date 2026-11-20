use std::{
    ffi::{CStr, CString, c_void},
    hash::{DefaultHasher, Hash, Hasher},
};

use ash::vk::{
    Bool32, DebugUtilsLabelEXT, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
    DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
};

use crate::{
    Error, Result,
    renderers::{
        debug::{DebugWriter, Debugger, RendererDebugObject, Severity},
        downcast_ref,
        vulkan::{
            VulkanBuffer, VulkanColorCanvas, VulkanComputePipeline, VulkanDepthCanvas,
            VulkanGraphicsPipeline, VulkanRenderer, VulkanSampler, VulkanTexture,
        },
    },
};

pub(super) struct VulkanDebugWriter {
    pub(super) device: ash::ext::debug_utils::Device,
    pub(super) render_pass_rendering: bool,
}

impl DebugWriter for VulkanDebugWriter {
    fn set_object_name(&self, object: &dyn RendererDebugObject, name: &str) -> Result<()> {
        let name = CString::new(name)?;
        fn set<T: ash::vk::Handle + Copy>(
            device: &ash::ext::debug_utils::Device,
            name: &CStr,
            handle: T,
        ) -> Result<()> {
            if handle.is_null() {
                return Err(Error::invalid_argument("Provided object is destroyed."));
            }
            let name_info = ash::vk::DebugUtilsObjectNameInfoEXT::default()
                .object_name(name)
                .object_handle(handle);
            unsafe { device.set_debug_utils_object_name(&name_info)? };
            Ok(())
        }
        if let Ok(buffer) = downcast_ref::<VulkanBuffer>(object) {
            set(&self.device, &name, buffer.buffer)?;
        } else if let Ok(texture) = downcast_ref::<VulkanTexture>(object) {
            set(&self.device, &name, texture.image)?;
            set(&self.device, &name, texture.image_view)?;
        } else if let Ok(sampler) = downcast_ref::<VulkanSampler>(object) {
            set(&self.device, &name, sampler.sampler)?;
        } else if let Ok(pipeline) = downcast_ref::<VulkanComputePipeline>(object) {
            set(&self.device, &name, pipeline.pipeline)?;
            set(&self.device, &name, pipeline.layout)?;
            for descriptor_set_layout in &pipeline.descriptor_layouts {
                set(&self.device, &name, *descriptor_set_layout)?;
            }
        } else if let Ok(pipeline) = downcast_ref::<VulkanGraphicsPipeline>(object) {
            set(&self.device, &name, pipeline.pipeline)?;
            set(&self.device, &name, pipeline.layout)?;
            for descriptor_set_layout in &pipeline.descriptor_layouts {
                set(&self.device, &name, *descriptor_set_layout)?;
            }
        } else if let Ok(canvas) = downcast_ref::<VulkanColorCanvas>(object) {
            if self.render_pass_rendering {
                set(&self.device, &name, canvas.render_pass)?;
                set(&self.device, &name, canvas.framebuffer)?;
            }
            for texture in &canvas.textures {
                set(&self.device, &name, texture.image)?;
                set(&self.device, &name, texture.image_view)?;
            }
            set(&self.device, &name, canvas.depth_image.image)?;
            set(&self.device, &name, canvas.depth_image.view)?;
            for msaa_image in &canvas.msaa_color_images {
                set(&self.device, &name, msaa_image.image)?;
                set(&self.device, &name, msaa_image.view)?;
            }
        } else if let Ok(canvas) = downcast_ref::<VulkanDepthCanvas>(object) {
            if self.render_pass_rendering {
                set(&self.device, &name, canvas.render_pass)?;
                set(&self.device, &name, canvas.framebuffer)?;
            }
            set(&self.device, &name, canvas.texture.image)?;
            set(&self.device, &name, canvas.texture.image_view)?;
        } else {
            return Err(Error::invalid_argument(
                "Provided object is not a Vulkan debug object.",
            ));
        }
        Ok(())
    }
}

impl VulkanRenderer {
    pub(super) fn set_debug_writer(&self, debugger: &Debugger) -> Result<()> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(Error::invalid_operation("Renderer is not initialized"))?;
        let device_context = self
            .device_context
            .as_ref()
            .ok_or(Error::invalid_operation("Device is not set."))?;
        let _ = debugger.set_writer(Some(Box::new(VulkanDebugWriter {
            device: ash::ext::debug_utils::Device::new(instance, &device_context.logical_device),
            render_pass_rendering: !device_context.supports_dynamic_rendering,
        })));
        Ok(())
    }

    /// Ends the popped labels that have begun in the command buffer, then begins the labels pushed
    /// since the last command in it, if a debugger is attached. Returns whether any label has
    /// begun.
    pub(super) fn begin_labels(
        debugger: Option<&Debugger>,
        command_buffer: ash::vk::CommandBuffer,
    ) -> Result<bool> {
        let _ = Self::end_labels(debugger, command_buffer)?;
        let Some(debugger) = debugger else {
            return Ok(false);
        };
        let mut hasher = DefaultHasher::new();
        command_buffer.hash(&mut hasher);
        let labels = debugger.begin_labels(hasher.finish());
        if labels.is_empty() {
            return Ok(false);
        }
        let writer = debugger.writer()?;
        let Some(writer) = writer.as_ref() else {
            // Writer's lifetime is tied to the device context.
            return Err(Error::invalid_operation("Device is not set."));
        };
        let writer = downcast_ref::<VulkanDebugWriter>(writer.as_ref())?;
        for label in labels {
            let label = CString::new(label)?;
            let label = DebugUtilsLabelEXT::default().label_name(label.as_c_str());
            unsafe {
                writer
                    .device
                    .cmd_begin_debug_utils_label(command_buffer, &label)
            };
        }
        Ok(true)
    }

    /// Ends the popped labels that have begun in the command buffer if a debugger is attached.
    /// Returns whether any label has ended.
    pub(super) fn end_labels(
        debugger: Option<&Debugger>,
        command_buffer: ash::vk::CommandBuffer,
    ) -> Result<bool> {
        let Some(debugger) = debugger else {
            return Ok(false);
        };
        let mut hasher = DefaultHasher::new();
        command_buffer.hash(&mut hasher);
        let count = debugger.end_labels(hasher.finish());
        if count == 0 {
            return Ok(false);
        }
        let writer = debugger.writer()?;
        let Some(writer) = writer.as_ref() else {
            // Writer's lifetime is tied to the device context.
            return Err(Error::invalid_operation("Device is not set."));
        };
        let writer = downcast_ref::<VulkanDebugWriter>(writer.as_ref())?;
        for _ in 0..count {
            unsafe { writer.device.cmd_end_debug_utils_label(command_buffer) };
        }
        Ok(true)
    }

    /// Validates whether all labels have been popped.
    pub(super) fn validate_labels(debugger: Option<&Debugger>) -> Result<()> {
        if let Some(debugger) = debugger
            && debugger.has_labels()
        {
            Err(Error::fail(
                "All labels should be popped before ending the frame.",
            ))
        } else {
            Ok(())
        }
    }

    /// Creates the messenger that forwards the validation messages to the attached debugger.
    pub(super) fn create_debug_messenger(&self) -> Result<()> {
        let debug_utils_loader =
            self.debug_utils_loader
                .as_ref()
                .ok_or(Error::invalid_operation(
                    "The debug utils extension is not loaded.",
                ))?;
        let debugger = self
            .debugger
            .as_deref()
            .ok_or(Error::invalid_operation("No attached debugger found."))?;

        let create_info = DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | DebugUtilsMessageSeverityFlagsEXT::INFO
                    | DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
            )
            .message_type(
                DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(debug_messenger_callback))
            .user_data(debugger as *const Debugger as *mut c_void);
        let debug_messenger =
            unsafe { debug_utils_loader.create_debug_utils_messenger(&create_info, None)? };
        self.debug_messenger.set(Some(debug_messenger));

        Ok(())
    }

    /// Destroys the debug messenger if there is one.
    pub(super) fn destroy_debug_messenger(&self) {
        if let (Some(debug_utils_loader), Some(debug_messenger)) = (
            self.debug_utils_loader.as_ref(),
            self.debug_messenger.take(),
        ) {
            unsafe {
                debug_utils_loader.destroy_debug_utils_messenger(debug_messenger, None);
            }
        }
    }
}

/// Appends the received message to the debugger.
unsafe extern "system" fn debug_messenger_callback(
    message_severity: DebugUtilsMessageSeverityFlagsEXT,
    _: DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const DebugUtilsMessengerCallbackDataEXT<'_>,
    user_data: *mut c_void,
) -> Bool32 {
    if callback_data.is_null() || user_data.is_null() {
        return ash::vk::FALSE;
    }
    let callback_data = unsafe { &*callback_data };
    if callback_data.p_message.is_null() {
        return ash::vk::FALSE;
    }

    let message = unsafe { CStr::from_ptr(callback_data.p_message) }.to_string_lossy();
    let severity = if message_severity.contains(DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        Severity::Error
    } else if message_severity.contains(DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        Severity::Warning
    } else {
        Severity::Info
    };

    let debugger = unsafe { &*(user_data as *const Debugger) };
    debugger.log(message, severity);

    ash::vk::FALSE
}
