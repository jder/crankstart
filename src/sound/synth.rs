use crate::{pd_func_caller, pd_func_caller_log};
use anyhow::{anyhow, ensure, Error, Result};
use crankstart_sys::{PDSynth, PDSynthSignalValue};

pub struct Synth {
    raw_subsystem: *const crankstart_sys::playdate_sound_synth,
    raw_synth: *mut PDSynth,
}

impl Synth {
    pub(crate) fn new(
        raw_synth: *const crankstart_sys::playdate_sound_synth,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw_subsystem: raw_synth,
            raw_synth: pd_func_caller!((*raw_synth).newSynth)?,
        })
    }

    pub fn set_waveform(&mut self, waveform: crankstart_sys::SoundWaveform) -> Result<()> {
        pd_func_caller!((*self.raw_subsystem).setWaveform, self.raw_synth, waveform)
    }

    pub fn set_frequency_modulator<S: Signal>(&mut self, frequency_mod: &S) -> Result<()> {
        pd_func_caller!(
            (*self.raw_subsystem).setFrequencyModulator,
            self.raw_synth,
            frequency_mod.as_signal_value().value
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

/// Wrapper for known-good PDSynthSignalValue pointers.
pub struct SignalValue<'a> {
    value: *mut PDSynthSignalValue,
    _marker: core::marker::PhantomData<&'a ()>,
}

pub trait Signal {
    fn as_signal_value(&self) -> SignalValue;
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
}

impl Drop for LFO {
    fn drop(&mut self) {
        pd_func_caller_log!((*self.raw_subsystem).freeLFO, self.raw_lfo);
    }
}

impl Signal for LFO {
    fn as_signal_value(&self) -> SignalValue {
        SignalValue {
            value: self.raw_lfo as *mut PDSynthSignalValue,
            _marker: core::marker::PhantomData,
        }
    }
}
