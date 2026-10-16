use heph_gl::Version;

use crate::renderer::flags::RendererTestFlags;

#[derive(Clone, Copy)]
pub struct RendererVersionedTestSuite {
    /// The API version the renderer will use for the current test suite.
    pub api_version: Option<Version>,
    /// The flags of the current test suite.
    pub flags: RendererTestFlags,
}

impl RendererVersionedTestSuite {
    pub fn version_string(&self) -> String {
        if let Some(api_version) = self.api_version {
            api_version.to_string().to_uppercase()
        } else {
            "LATEST VERSION".to_string()
        }
    }
}
