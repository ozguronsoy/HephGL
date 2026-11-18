use std::ffi::{CStr, CString, c_void};

use ash::vk::{
    Bool32, DebugUtilsLabelEXT, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
    DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
};

use crate::{
    Error, Result,
    renderers::{
        debug::{DebugLabelState, DebugWriter, Debugger, RendererDebugObject, Severity},
        downcast_ref,
        vulkan::{
            VulkanBuffer, VulkanColorCanvas, VulkanComputePipeline, VulkanDepthCanvas,
            VulkanGraphicsPipeline, VulkanRenderer, VulkanSampler, VulkanTexture,
        },
    },
};

pub(super) struct VulkanDebugWriter {
    pub(super) device: ash::ext::debug_utils::Device,
}

impl DebugWriter for VulkanDebugWriter {
    fn set_object_name(&self, object: &dyn RendererDebugObject, name: &str) -> Result<()> {
        let name = CString::new(name)?;
        let mut name_info = ash::vk::DebugUtilsObjectNameInfoEXT::default().object_name(&name);
        if let Ok(buffer) = downcast_ref::<VulkanBuffer>(object) {
            name_info = name_info.object_handle(buffer.buffer);
        } else if let Ok(texture) = downcast_ref::<VulkanTexture>(object) {
            name_info = name_info.object_handle(texture.image);
        } else if let Ok(sampler) = downcast_ref::<VulkanSampler>(object) {
            name_info = name_info.object_handle(sampler.sampler);
        } else if let Ok(pipeline) = downcast_ref::<VulkanComputePipeline>(object) {
            name_info = name_info.object_handle(pipeline.pipeline);
        } else if let Ok(pipeline) = downcast_ref::<VulkanGraphicsPipeline>(object) {
            name_info = name_info.object_handle(pipeline.pipeline);
        } else if let Ok(canvas) = downcast_ref::<VulkanColorCanvas>(object) {
            name_info = name_info.object_handle(canvas.texture.image);
        } else if let Ok(canvas) = downcast_ref::<VulkanDepthCanvas>(object) {
            name_info = name_info.object_handle(canvas.texture.image);
        } else {
            return Err(Error::invalid_argument(
                "Provided object is not a Vulkan debug object.",
            ));
        }
        if name_info.object_handle == 0 {
            return Err(Error::invalid_argument("Provided object is destroyed."));
        }
        unsafe { self.device.set_debug_utils_object_name(&name_info)? };
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
        })));
        Ok(())
    }

    /// Begins a label if a debugger is attached and a label is pushed. Otherwise, does nothing.
    pub(super) fn begin_label(
        debugger: Option<&Debugger>,
        command_buffer: ash::vk::CommandBuffer,
    ) -> Result<bool> {
        // Remove the popped label first if there is one.
        let _ = Self::end_label(debugger, command_buffer)?;
        let mut begun = false;
        if let Some(debugger) = debugger
            && let Some(label) = debugger.label()
            && label.state() == DebugLabelState::Idle
        {
            let writer = debugger.writer()?;
            let Some(writer) = writer.as_ref() else {
                // Writer's lifetime is tied to the device context.
                return Err(Error::invalid_operation("Device is not set."));
            };
            let writer = downcast_ref::<VulkanDebugWriter>(writer.as_ref())?;
            let label = CString::new(label.label())?;
            let label = DebugUtilsLabelEXT::default().label_name(label.as_c_str());
            unsafe {
                writer
                    .device
                    .cmd_begin_debug_utils_label(command_buffer, &label)
            };
            debugger.begin_label();
            begun = true;
        }
        Ok(begun)
    }

    /// Ends a label if a debugger is attached and a label has begun. Otherwise, does nothing.
    pub(super) fn end_label(
        debugger: Option<&Debugger>,
        command_buffer: ash::vk::CommandBuffer,
    ) -> Result<bool> {
        let mut ended = false;
        if let Some(debugger) = debugger
            && let Some(label) = debugger.label()
            && label.state() == DebugLabelState::Ended
        {
            let writer = debugger.writer()?;
            let Some(writer) = writer.as_ref() else {
                // Writer's lifetime is tied to the device context.
                return Err(Error::invalid_operation("Device is not set."));
            };
            let writer = downcast_ref::<VulkanDebugWriter>(writer.as_ref())?;
            unsafe { writer.device.cmd_end_debug_utils_label(command_buffer) };
            debugger.end_label();
            ended = true;
        }
        Ok(ended)
    }

    /// Validates whether all labels have been popped.
    pub(super) fn validate_labels(debugger: Option<&Debugger>) -> Result<()> {
        if let Some(debugger) = debugger
            && debugger.label().is_some()
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
