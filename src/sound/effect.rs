use crate::{pd_func_caller, pd_func_caller_log};
use alloc::rc::Rc;
use anyhow::{Error, Result};
use core::marker::PhantomData;

/// # Safety
/// This trait must guarantee that the returned pointer is valid for the `self` lifetime.
pub unsafe trait Effect: 'static {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect;
}

#[derive(Clone)]
pub struct Overdrive(Rc<OverdriveInner>);

impl Overdrive {
    pub(crate) fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_effect_overdrive,
    ) -> Result<Self, Error> {
        Ok(Self(Rc::new(OverdriveInner {
            raw_subsystem,
            raw_overdrive: pd_func_caller!((*raw_subsystem).newOverdrive)?,
        })))
    }

    pub fn set_gain(&mut self, gain: f32) -> Result<()> {
        pd_func_caller!((*self.0.raw_subsystem).setGain, self.0.raw_overdrive, gain)
    }

    pub fn set_limit(&mut self, limit: f32) -> Result<()> {
        pd_func_caller!(
            (*self.0.raw_subsystem).setLimit,
            self.0.raw_overdrive,
            limit
        )
    }
}

unsafe impl Effect for Overdrive {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect {
        self.0.raw_overdrive as *mut crankstart_sys::SoundEffect
    }
}

struct OverdriveInner {
    raw_subsystem: *const crankstart_sys::playdate_sound_effect_overdrive,
    raw_overdrive: *mut crankstart_sys::Overdrive,
}

impl Drop for OverdriveInner {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeOverdrive, self.raw_overdrive);
    }
}
