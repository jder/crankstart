use crate::{pd_func_caller, pd_func_caller_log, sound::SAMPLES_PER_SECOND};
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

#[derive(Clone)]
pub struct OnePoleFilter(Rc<OnePoleFilterInner>);

impl OnePoleFilter {
    pub(crate) fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_effect_onepolefilter,
    ) -> Result<Self, Error> {
        Ok(Self(Rc::new(OnePoleFilterInner {
            raw_subsystem,
            raw_one_pole_filter: pd_func_caller!((*raw_subsystem).newFilter)?,
        })))
    }

    pub fn set_parameter(&mut self, parameter: f32) -> Result<()> {
        pd_func_caller!(
            (*self.0.raw_subsystem).setParameter,
            self.0.raw_one_pole_filter,
            parameter
        )
    }
}

unsafe impl Effect for OnePoleFilter {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect {
        self.0.raw_one_pole_filter as *mut crankstart_sys::SoundEffect
    }
}

struct OnePoleFilterInner {
    raw_subsystem: *const crankstart_sys::playdate_sound_effect_onepolefilter,
    raw_one_pole_filter: *mut crankstart_sys::OnePoleFilter,
}

impl Drop for OnePoleFilterInner {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeFilter, self.raw_one_pole_filter);
    }
}

#[derive(Clone)]
pub struct DelayLine(Rc<DelayLineInner>);

impl DelayLine {
    pub(crate) fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_effect_delayline,
        length_seconds: f32,
        stereo: bool,
    ) -> Result<Self, Error> {
        Ok(Self(Rc::new(DelayLineInner {
            raw_subsystem,
            raw_delay_line: pd_func_caller!(
                (*raw_subsystem).newDelayLine,
                (length_seconds * (SAMPLES_PER_SECOND as f32)) as i32,
                stereo as i32
            )?,
        })))
    }

    pub fn set_feedback(&mut self, feedback: f32) -> Result<()> {
        pd_func_caller!(
            (*self.0.raw_subsystem).setFeedback,
            self.0.raw_delay_line,
            feedback
        )
    }
}

unsafe impl Effect for DelayLine {
    fn get_sound_effect(&self) -> *mut crankstart_sys::SoundEffect {
        self.0.raw_delay_line as *mut crankstart_sys::SoundEffect
    }
}

struct DelayLineInner {
    raw_subsystem: *const crankstart_sys::playdate_sound_effect_delayline,
    raw_delay_line: *mut crankstart_sys::DelayLine,
}

impl Drop for DelayLineInner {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeDelayLine, self.raw_delay_line);
    }
}
