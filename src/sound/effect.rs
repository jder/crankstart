use crate::{pd_func_caller, pd_func_caller_log};
use anyhow::{Error, Result};
use core::marker::PhantomData;

/// # Safety
/// This trait must guarantee that the returned pointer is valid for the `self` lifetime.
/// TODO: This is currently not sound -- dropping the original `Effect` will invalidate the
/// returned pointer -- ie seems like there's no reference counting internally?
/// (This might also be true for the other unsafe traits, we might need to do our own reference counting.)
pub unsafe trait Effect {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect;
}

pub struct Overdrive {
    raw_subsystem: *const crankstart_sys::playdate_sound_effect_overdrive,
    raw_overdrive: *mut crankstart_sys::Overdrive,
}

impl Overdrive {
    pub(crate) fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_effect_overdrive,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem,
            raw_overdrive: pd_func_caller!((*raw_subsystem).newOverdrive)?,
        })
    }

    pub fn set_gain(&mut self, gain: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setGain, self.raw_overdrive, gain)
    }

    pub fn set_limit(&mut self, limit: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setLimit, self.raw_overdrive, limit)
    }
}

impl Drop for Overdrive {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeOverdrive, self.raw_overdrive);
    }
}

unsafe impl Effect for Overdrive {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect {
        self.raw_overdrive as *mut crankstart_sys::SoundEffect
    }
}
