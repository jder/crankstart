use crate::{pd_func_caller, pd_func_caller_log};
use anyhow::{Error, Result};
use core::marker::PhantomData;

pub trait Effect {
    fn get_sound_effect(&self) -> UnsafeSoundEffect;
}

pub struct UnsafeSoundEffect<'a> {
    effect: *mut crankstart_sys::SoundEffect,
    _marker: PhantomData<&'a ()>,
}

impl<'a> UnsafeSoundEffect<'a> {
    /// # Safety
    /// `effect` must be a valid pointer to a `SoundEffect` struct for 'a.
    pub unsafe fn new(effect: *mut crankstart_sys::SoundEffect) -> Self {
        Self {
            effect,
            _marker: PhantomData,
        }
    }

    pub fn effect(&self) -> *mut crankstart_sys::SoundEffect {
        self.effect
    }
}

pub struct Overdrive {
    raw_subsystem: *const crankstart_sys::playdate_sound_effect_overdrive,
    raw_overdrive: *mut crankstart_sys::Overdrive,
}

impl Overdrive {
    pub(crate) fn new(
        raw_overdrive: *const crankstart_sys::playdate_sound_effect_overdrive,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem: raw_overdrive,
            raw_overdrive: pd_func_caller!((*raw_overdrive).newOverdrive)?,
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

impl Effect for Overdrive {
    fn get_sound_effect(&self) -> UnsafeSoundEffect {
        unsafe { UnsafeSoundEffect::new(self.raw_overdrive as *mut crankstart_sys::SoundEffect) }
    }
}
