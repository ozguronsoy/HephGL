use ash::vk::{
    DescriptorPool, DescriptorPoolCreateInfo, DescriptorPoolResetFlags, DescriptorPoolSize,
    DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout, DescriptorType,
};

use crate::Result;

/// The number of sets the first descriptor pool can hold.
const INITIAL_MAX_SETS: u32 = 1000;
/// The maximum number of sets a descriptor pool can hold.
const MAX_SET_COUNT: u32 = u32::MAX / 2;
/// The descriptor types the pools can allocate. Each pool has room for as many descriptors of each
/// type as it has sets.
const DESCRIPTOR_TYPES: [DescriptorType; 4] = [
    DescriptorType::UNIFORM_BUFFER,
    DescriptorType::STORAGE_BUFFER,
    DescriptorType::SAMPLED_IMAGE,
    DescriptorType::SAMPLER,
];

/// A growable collection of descriptor pools.
pub struct DescriptorPools {
    pools: Vec<DescriptorPool>,
    /// The index of the pool the sets are currently allocated from.
    current_index: usize,
    /// The number of sets the next created pool can hold.
    next_max_sets: u32,
}

impl Default for DescriptorPools {
    fn default() -> Self {
        Self {
            pools: Vec::new(),
            current_index: 0,
            next_max_sets: INITIAL_MAX_SETS,
        }
    }
}

impl DescriptorPools {
    /// Allocates one descriptor set for each layout. Creates a larger pool if the existing ones
    /// don't have enough space.
    pub fn allocate(
        &mut self,
        device: &ash::Device,
        layouts: &[DescriptorSetLayout],
    ) -> Result<Vec<DescriptorSet>> {
        if layouts.is_empty() {
            return Ok(Vec::new());
        }

        // The pools are reset every frame, so a pool that is too full for this request is skipped
        // until the next reset.
        while let Some(&pool) = self.pools.get(self.current_index) {
            let sets = Self::try_allocate(device, pool, layouts)?;
            if !sets.is_empty() {
                return Ok(sets);
            }
            self.current_index += 1;
        }

        loop {
            let max_sets = self.next_max_sets;
            let pool = Self::create_pool(device, max_sets)?;
            match Self::try_allocate(device, pool, layouts) {
                Ok(sets) => {
                    if sets.is_empty() {
                        // The request doesn't fit in a pool of this size, try a bigger one.
                        unsafe { device.destroy_descriptor_pool(pool, None) };
                        self.next_max_sets = grow(max_sets);
                        continue;
                    }
                    self.pools.push(pool);
                    self.current_index = self.pools.len() - 1;
                    self.next_max_sets = grow(max_sets);
                    return Ok(sets);
                }
                Err(e) => {
                    unsafe { device.destroy_descriptor_pool(pool, None) };
                    return Err(e);
                }
            }
        }
    }

    /// Frees all the sets allocated from the pools, so the pools can be used again.
    pub fn reset(&mut self, device: &ash::Device) -> Result<()> {
        for pool in &self.pools {
            unsafe { device.reset_descriptor_pool(*pool, DescriptorPoolResetFlags::empty())? };
        }
        self.current_index = 0;
        Ok(())
    }

    /// Destroys all pools.
    pub fn destroy(&mut self, device: &ash::Device) {
        for pool in self.pools.drain(..) {
            unsafe { device.destroy_descriptor_pool(pool, None) };
        }
        self.current_index = 0;
        self.next_max_sets = INITIAL_MAX_SETS;
    }

    fn try_allocate(
        device: &ash::Device,
        pool: DescriptorPool,
        layouts: &[DescriptorSetLayout],
    ) -> Result<Vec<DescriptorSet>> {
        let alloc_info = DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(layouts);
        match unsafe { device.allocate_descriptor_sets(&alloc_info) } {
            Ok(sets) => Ok(sets),
            Err(
                ash::vk::Result::ERROR_OUT_OF_POOL_MEMORY | ash::vk::Result::ERROR_FRAGMENTED_POOL,
            ) => Ok(Vec::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn create_pool(device: &ash::Device, max_sets: u32) -> Result<DescriptorPool> {
        let pool_sizes = DESCRIPTOR_TYPES.map(|descriptor_type| {
            DescriptorPoolSize::default()
                .ty(descriptor_type)
                .descriptor_count(max_sets)
        });
        let pool_info = DescriptorPoolCreateInfo::default()
            .max_sets(max_sets)
            .pool_sizes(&pool_sizes);
        Ok(unsafe { device.create_descriptor_pool(&pool_info, None)? })
    }
}

fn grow(max_sets: u32) -> u32 {
    (max_sets * 2).min(MAX_SET_COUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let pools = DescriptorPools::default();
        assert!(pools.pools.is_empty());
        assert_eq!(pools.current_index, 0);
        assert_eq!(pools.next_max_sets, INITIAL_MAX_SETS);
    }

    #[test]
    fn test_grow() {
        assert_eq!(grow(INITIAL_MAX_SETS), INITIAL_MAX_SETS * 2);
        assert_eq!(grow(MAX_SET_COUNT.div_ceil(2) + 1), MAX_SET_COUNT);
    }
}
