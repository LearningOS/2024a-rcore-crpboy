//! stride info

/// stride const def
const BIG_STRIDE: isize = 1024;
const INIT_PRIO: isize = 16;

/// stride block
pub struct StrideBlock {
    /// priority
    pub prio: isize,
    /// current stride length
    pub stride: isize,
    /// stride += pass when the proc is run
    pub pass: isize,
}

impl StrideBlock {
    /// create a new stride block with INIT_PRIO = 16
    pub fn new() -> Self {
        Self {
            prio: INIT_PRIO,
            stride: 0,
            pass: BIG_STRIDE / INIT_PRIO,
        }
    }
    /// set a new priority
    /// will be called when sys_set_prio is called
    pub fn set_new_prio(&mut self, new_prio: isize) {
        self.prio = new_prio;
        self.pass = BIG_STRIDE / new_prio;
    }
}
