use std::ffi::{CStr, c_void};

use ash::vk::{
    Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
    DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerCreateInfoEXT,
};

use crate::{
    Error, Result,
    renderers::{
        debug::{Debugger, Severity},
        vulkan::VulkanRenderer,
    },
};

impl VulkanRenderer {
    /// Creates the messenger that forwards the validation messages to the attached debugger.
    pub(super) fn create_debug_messenger(&self) -> Result<()> {
        let debug_utils_loader =
            self.debug_utils_loader
                .as_ref()
                .ok_or(Error::invalid_operation(
                    "The debug utils extension is not loaded.",
                ))?;
        let debugger = self.debugger.borrow();
        let debugger = debugger
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
