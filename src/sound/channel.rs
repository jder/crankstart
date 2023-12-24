use crate::sound::effect::Effect;
use crate::sound::SoundSource;
use crate::{pd_func_caller, pd_func_caller_log};
use anyhow::{Error, Result};
use core::marker::PhantomData;

pub struct SoundChannel {
    raw_subsystem: *const crankstart_sys::playdate_sound_channel,
    raw_channel: *mut crankstart_sys::SoundChannel,
}

impl SoundChannel {
    pub fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_channel,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem: raw_subsystem,
            raw_channel: pd_func_caller!((*raw_subsystem).newChannel)?,
        })
    }

    pub fn add_effect<E: Effect>(&mut self, effect: &E) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).addEffect,
            self.raw_channel,
            effect.get_sound_effect().effect()
        )
    }

    pub fn add_source<S: SoundSource>(&mut self, source: &S) -> Result<i32> {
        pd_func_caller!(
            (*self.raw_subsystem).addSource,
            self.raw_channel,
            source.get_sound_source().source()
        )
    }
}

impl Drop for SoundChannel {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeChannel, self.raw_channel);
    }
}
