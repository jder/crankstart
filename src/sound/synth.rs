use crate::{pd_func_caller, pd_func_caller_log};
use anyhow::{anyhow, ensure, Error, Result};
use crankstart_sys::{PDSynth, PDSynthSignalValue};

use super::SoundSource;

pub struct Synth {
    raw_subsystem: *const crankstart_sys::playdate_sound_synth,
    raw_synth: *mut PDSynth,
}

impl Synth {
    pub(crate) fn new(
        raw_subsystem: *const crankstart_sys::playdate_sound_synth,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem,
            raw_synth: pd_func_caller!((*raw_subsystem).newSynth)?,
        })
    }

    pub fn set_waveform(&mut self, waveform: crankstart_sys::SoundWaveform) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setWaveform, self.raw_synth, waveform)
    }

    pub fn set_frequency_modulator<S: Signal>(&mut self, frequency_mod: &S) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).setFrequencyModulator,
            self.raw_synth,
            frequency_mod.as_signal_value()
        )
    }

    pub fn set_volume(&mut self, volume_left: f32, volume_right: f32) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).setVolume,
            self.raw_synth,
            volume_left,
            volume_right
        )
    }

    pub fn get_volume(&self) -> Result<(f32, f32)> {
        let mut left = 0.0;
        let mut right = 0.0;
        pd_func_caller!(
            (*self.raw_subsystem).getVolume,
            self.raw_synth,
            &mut left,
            &mut right
        )?;
        Ok((left, right))
    }

    pub fn play_midi_note(
        &mut self,
        note: crankstart_sys::MIDINote,
        velocity: f32,
        duration: f32,
        when: u32,
    ) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).playMIDINote,
            self.raw_synth,
            note,
            velocity,
            duration,
            when
        )
    }

    pub fn note_off(&mut self, when: u32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).noteOff, self.raw_synth, when)
    }
}

impl Drop for Synth {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeSynth, self.raw_synth);
    }
}

/// # Safety
/// This trait must guarantee that the returned pointer is valid for the `self` lifetime.
pub unsafe trait Signal {
    fn as_signal_value(&self) -> *mut PDSynthSignalValue;
}

pub struct LFO {
    raw_subsystem: *const crankstart_sys::playdate_sound_lfo,
    raw_lfo: *mut crankstart_sys::PDSynthLFO,
}

impl LFO {
    pub(crate) fn new(
        raw_lfo: *const crankstart_sys::playdate_sound_lfo,
        lfo_type: crankstart_sys::LFOType,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem: raw_lfo,
            raw_lfo: pd_func_caller!((*raw_lfo).newLFO, lfo_type)?,
        })
    }

    pub fn set_center(&mut self, center: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setCenter, self.raw_lfo, center)
    }

    pub fn set_rate(&mut self, rate: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setRate, self.raw_lfo, rate)
    }

    pub fn set_depth(&mut self, depth: f32) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setDepth, self.raw_lfo, depth)
    }

    pub fn set_retrigger(&mut self, retrigger: bool) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).setRetrigger,
            self.raw_lfo,
            retrigger as i32
        )
    }
}

impl Drop for LFO {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeLFO, self.raw_lfo);
    }
}

unsafe impl Signal for LFO {
    fn as_signal_value(&self) -> *mut PDSynthSignalValue {
        self.raw_lfo as *mut PDSynthSignalValue
    }
}

// SAFETY: Synth is a sound source we keep alive for self's lifetime
unsafe impl SoundSource for Synth {
    fn get_sound_source(&self) -> *mut crankstart_sys::SoundSource {
        self.raw_synth as *mut crankstart_sys::SoundSource
    }
}
