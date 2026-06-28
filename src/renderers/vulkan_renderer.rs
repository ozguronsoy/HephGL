pub struct VulkanRenderer {}

impl crate::renderers::Renderer for VulkanRenderer {
    fn create() -> Self {
        Self {}
    }

    fn initialize(&mut self) {
        self.clear(renkrs::RGB::<u8> { r: 255, g: 0, b: 0 });
    }

    fn clear(&mut self, color: renkrs::RGB<u8>) {
        let color_f32: renkrs::RGB<f32> = color.into();
        let clear_value = ash::vk::ClearValue {
            color: ash::vk::ClearColorValue {
                float32: [color_f32.r, color_f32.g, color_f32.b, 1.0],
            },
        };
    }
}
