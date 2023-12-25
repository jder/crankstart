use crate::sound::effect::Effect;
use crate::sound::SoundSource;
use crate::{pd_func_caller, pd_func_caller_log};
use alloc::boxed::Box;
use alloc::vec::Vec;
use anyhow::{Error, Result};
use core::marker::PhantomData;

pub struct SoundChannel {
    raw_subsystem: *const crankstart_sys::playdate_sound_channel,
    raw_channel: *mut crankstart_sys::SoundChannel,
    effects: Vec<Box<dyn Effect>>,
    sources: Vec<Box<dyn SoundSource>>,
}

impl SoundChannel {
    pub fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_channel,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem,
            raw_channel: pd_func_caller!((*raw_subsystem).newChannel)?,
            effects: Vec::new(),
            sources: Vec::new(),
        })
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setVolume, self.raw_channel, volume)
    }

    pub fn add_effect<E: Effect>(&mut self, effect: E) -> Result<()> {
        let result = pd_func_caller!(
            (*self.raw_subsystem).addEffect,
            self.raw_channel,
            effect.get_sound_effect()
        );
        self.effects.push(Box::new(effect));
        result
    }

    pub fn add_source<S: SoundSource>(&mut self, source: S) -> Result<i32> {
        let result = pd_func_caller!(
            (*self.raw_subsystem).addSource,
            self.raw_channel,
            source.get_sound_source()
        );
        self.sources.push(Box::new(source));
        result
    }
}

impl Drop for SoundChannel {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeChannel, self.raw_channel);
    }
}
