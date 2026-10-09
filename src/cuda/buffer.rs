pub struct Staging {
    slots: Vec<u64>,
}

impl Staging {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![0u64; capacity.max(1)],
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u64 {
        self.slots.as_mut_ptr()
    }

    pub fn take(&mut self, count: usize) -> Vec<u64> {
        let limit = count.min(self.slots.len());
        let mut out = Vec::with_capacity(limit);
        for index in 0..limit {
            out.push(self.slots[index]);
        }
        out
    }
}

impl Default for Staging {
    fn default() -> Self {
        Self::new(256)
    }
}
