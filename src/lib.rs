//! Rust binding for [opal](https://github.com/RealBitdancer/opal), the OPL3
//! (Yamaha YMF262) and OPL2 FM synthesis emulator.
//!
//! Copyright (c) 2026 Bitdancer (github.com/RealBitdancer).
//! SPDX-License-Identifier: MIT
//!
//! The structs below mirror `include/opal/opal.h` field for field. The test
//! suite verifies every size and offset against the C compiler, so a layout
//! change upstream fails the build loudly instead of corrupting state.
//!
//! # Quick start
//!
//! ```
//! use opal::Opal;
//!
//! let mut chip = Opal::new(44_100);
//! chip.write_reg(0x105, 0x01); // OPL3 mode
//!
//! let mut frames = [0i16; 256];
//! chip.render(&mut frames);
//! ```
//!
//! Prefer [`Opal::boxed`] when the ~70 KiB chip state should stay off the stack.
//!
//! Bank 1, waveforms 4-7, CHA and CHB, and four-operator pairing apply only
//! while NEW (register 105h bit 0) is set.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::boxed::Box;
use core::ffi::c_int;
use core::fmt;
use core::mem::MaybeUninit;

/// Bound opal major version (`OPAL_VERSION_MAJOR`).
pub const VERSION_MAJOR: u32 = 2;
/// Bound opal minor version (`OPAL_VERSION_MINOR`).
pub const VERSION_MINOR: u32 = 0;
/// Bound opal patch version (`OPAL_VERSION_PATCH`).
pub const VERSION_PATCH: u32 = 2;
/// Packed opal version: `major * 10000 + minor * 100 + patch` (`OPAL_VERSION`).
pub const VERSION: u32 = VERSION_MAJOR * 10_000 + VERSION_MINOR * 100 + VERSION_PATCH;

/// Native OPL3 sample rate in hertz.
pub const OPL3_SAMPLE_RATE: u32 = 49_716;
/// Number of melodic/rhythm channels in an OPL3 instance.
pub const CHANNEL_COUNT: usize = 18;
/// Number of operators (slots) in an OPL3 instance.
pub const OPERATOR_COUNT: usize = 36;
/// Capacity of the buffered register-write queue.
pub const WRITE_BUF_SIZE: usize = 4096;
/// Chip-sample spacing applied between buffered register writes.
pub const WRITE_BUF_DELAY: u32 = 2;

/// No operator: an index field with this value reads as silence.
pub const OP_NONE: u8 = 0xFF;
/// [`Operator::mod_source`] only: the operator's own feedback value.
pub const MOD_OWN_FB: u8 = 0xFE;
/// Channel cannot pair. Used on channels 6-8 and 15-17.
pub const CH_NONE: u8 = 0xFF;

/// Status register: IRQ flag (bit 7).
pub const STATUS_IRQ: u8 = 1 << 7;
/// Status register: timer 1 overflow (bit 6).
pub const STATUS_TIMER1: u8 = 1 << 6;
/// Status register: timer 2 overflow (bit 5).
pub const STATUS_TIMER2: u8 = 1 << 5;

/// Envelope generator stage of an operator.
///
/// ABI-identical to a C `int`. Named constants cover the values defined by
/// opal. An unknown value from a future release stays representable.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnvelopeStage(pub c_int);

impl EnvelopeStage {
    /// Envelope is idle / silent.
    ///
    /// Defined by the C header. The current opal core never stores this value.
    /// Idle operators sit in [`Self::RELEASE`] at full attenuation.
    pub const OFF: Self = Self(-1);
    /// Attack phase.
    pub const ATTACK: Self = Self(0);
    /// Decay phase.
    pub const DECAY: Self = Self(1);
    /// Sustain phase.
    pub const SUSTAIN: Self = Self(2);
    /// Release phase.
    pub const RELEASE: Self = Self(3);

    /// Returns a short name for a known stage, or `None` for an unknown value.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            -1 => Some("off"),
            0 => Some("attack"),
            1 => Some("decay"),
            2 => Some("sustain"),
            3 => Some("release"),
            _ => None,
        }
    }
}

impl fmt::Display for EnvelopeStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "EnvelopeStage({})", self.0),
        }
    }
}

impl From<c_int> for EnvelopeStage {
    #[inline]
    fn from(value: c_int) -> Self {
        Self(value)
    }
}

impl From<EnvelopeStage> for c_int {
    #[inline]
    fn from(value: EnvelopeStage) -> Self {
        value.0
    }
}

/// Current role of a channel.
///
/// ABI-identical to a C `int`. Named constants cover the values defined by
/// opal. An unknown value from a future release stays representable.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelType(pub c_int);

impl ChannelType {
    /// Two-operator melodic channel.
    pub const TWO_OP: Self = Self(0);
    /// Primary half of a four-operator pair.
    pub const FOUR_OP: Self = Self(1);
    /// Secondary half of a four-operator pair.
    pub const FOUR_OP2: Self = Self(2);
    /// Rhythm-mode percussion channel.
    pub const DRUM: Self = Self(3);

    /// Returns a short name for a known type, or `None` for an unknown value.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("2op"),
            1 => Some("4op"),
            2 => Some("4op2"),
            3 => Some("drum"),
            _ => None,
        }
    }
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "ChannelType({})", self.0),
        }
    }
}

impl From<c_int> for ChannelType {
    #[inline]
    fn from(value: c_int) -> Self {
        Self(value)
    }
}

impl From<ChannelType> for c_int {
    #[inline]
    fn from(value: ChannelType) -> Self {
        value.0
    }
}

/// One stereo 16-bit PCM sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StereoSample {
    pub left: i16,
    pub right: i16,
}

impl StereoSample {
    /// Creates a stereo sample from left and right channels.
    #[inline]
    #[must_use]
    pub const fn new(left: i16, right: i16) -> Self {
        Self { left, right }
    }

    /// Returns true when both channels are zero.
    #[inline]
    #[must_use]
    pub const fn is_silent(self) -> bool {
        self.left == 0 && self.right == 0
    }
}

impl From<(i16, i16)> for StereoSample {
    #[inline]
    fn from((left, right): (i16, i16)) -> Self {
        Self { left, right }
    }
}

impl From<StereoSample> for (i16, i16) {
    #[inline]
    fn from(sample: StereoSample) -> Self {
        (sample.left, sample.right)
    }
}

/// Queued register write entry inside the chip write buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WriteBuf {
    pub time: u64,
    pub reg: u16,
    pub data: u8,
}

/// One FM operator, mapping to one OPL3 slot.
///
/// Melodic channel `ch` uses `op[modulator_slot(ch)]` as its modulator and
/// `op[carrier_slot(ch)]` as its carrier. In 4-op mode the paired channel
/// contributes slot + 6 and slot + 9. All cross references are indices into
/// [`Opal::op`] and [`Opal::chan`], so a copied instance is fully self-contained.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operator {
    pub slot_index: u8,
    pub chan_index: u8,
    /// Phase modulation input: an index into [`Opal::op`] whose `out` is read,
    /// [`MOD_OWN_FB`] for the operator's own feedback, or [`OP_NONE`] for none.
    pub mod_source: u8,
    pub phase: u32,
    pub phase_out: u16,
    pub phase_reset: bool,
    pub waveform: u16,
    pub freq_mult_times2: u16,
    pub envelope_stage: EnvelopeStage,
    pub envelope_level: u16,
    /// Current total attenuation (envelope + output level + KSL + tremolo)
    /// in 0.1875 dB steps, where 0 is loudest and 511 or more is silence.
    pub eg_out: u16,
    pub eg_ksl: u8,
    pub output_level: u16,
    pub attack_rate: u16,
    pub decay_rate: u16,
    pub sustain_level: u16,
    pub release_rate: u16,
    pub key_scale_reg: u8,
    /// Nonzero while keyed on: bit 0 is normal key on, bit 1 is rhythm key on.
    pub key: u8,
    pub out: i16,
    pub prev_out: i16,
    pub fb_mod: i16,
    pub key_scale_rate: bool,
    pub sustain_mode: bool,
    pub tremolo_enable: bool,
    pub vibrato_enable: bool,
}

impl Operator {
    /// Returns true when the operator is keyed on by either normal or rhythm key-on.
    #[inline]
    #[must_use]
    pub const fn is_keyed_on(self) -> bool {
        self.key != 0
    }

    /// Returns true when total attenuation is below the silence threshold of 511.
    ///
    /// After init, operators are in [`EnvelopeStage::RELEASE`] with
    /// [`Self::eg_out`] at or above 511, so this is false until a key-on
    /// lowers the envelope. [`EnvelopeStage::OFF`] is not written by the
    /// current core.
    #[inline]
    #[must_use]
    pub const fn is_active(self) -> bool {
        self.eg_out < 511
    }
}

/// A single channel, which can contain two or more operators.
///
/// `op_slot` holds the indices of the channel's operators in [`Opal::op`]
/// ([`OP_NONE`] when absent). `pair_index` is the physical 4-op partner
/// (0+3, 1+4, 2+5, and the same on bank 1). Init pre-wires those pairs.
/// Channels that cannot pair (6-8, 15-17) use [`CH_NONE`]. Whether 4-op is
/// active is [`Channel::chan_type`], not `pair_index`. `out_source` lists up
/// to four operator indices whose `out` values sum into the channel output.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub op_slot: [u8; 4],
    pub chan_index: u8,
    /// Physical 4-op partner.
    ///
    /// Pre-wired on capable channels even while [`Self::chan_type`] is
    /// [`ChannelType::TWO_OP`]. [`CH_NONE`] only on channels that cannot
    /// pair. [`Self::is_paired`] tests the active role, not this field.
    pub pair_index: u8,
    pub out_source: [u8; 4],
    pub chan_type: ChannelType,
    pub freq: u16,
    pub octave: u16,
    pub key_scale_number: u16,
    pub feedback_shift: u16,
    pub modulation_type: u16,
    pub alg: u16,
    pub cha: u16,
    pub chb: u16,
    pub chc: u16,
    pub chd: u16,
    pub left_pan: u16,
    pub right_pan: u16,
}

impl Channel {
    /// Returns true when this channel is currently a 4-op primary or secondary.
    #[inline]
    #[must_use]
    pub const fn is_paired(self) -> bool {
        self.chan_type.0 == ChannelType::FOUR_OP.0 || self.chan_type.0 == ChannelType::FOUR_OP2.0
    }
}

const CHANNEL_SLOTS: [usize; CHANNEL_COUNT] = [
    0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20, 24, 25, 26, 30, 31, 32,
];

/// Index into [`Opal::op`] of the modulator slot for melodic channel `ch`
/// (0 to 8 is bank 0, 9 to 17 is bank 1).
///
/// # Panics
///
/// Panics if `ch >= CHANNEL_COUNT`.
#[inline]
#[must_use]
pub const fn modulator_slot(ch: usize) -> usize {
    CHANNEL_SLOTS[ch]
}

/// Index into [`Opal::op`] of the carrier slot for melodic channel `ch`.
///
/// # Panics
///
/// Panics if `ch >= CHANNEL_COUNT`.
#[inline]
#[must_use]
pub const fn carrier_slot(ch: usize) -> usize {
    CHANNEL_SLOTS[ch] + 3
}

/// An emulated OPL3 chip.
///
/// The struct holds no pointers: all internal references are indices, so an
/// instance is a plain value. Clone it, assign it, or relocate it freely. A
/// clone is a complete save state.
///
/// The struct is about 70 KiB. Prefer [`Opal::boxed`] when keeping it off the
/// stack matters.
#[repr(C)]
#[derive(Clone)]
pub struct Opal {
    pub sample_rate: i32,
    pub sample_accum: i32,
    pub last_output: [i16; 2],
    pub curr_output: [i16; 2],
    pub mix_buff_right: i32,
    pub chan: [Channel; CHANNEL_COUNT],
    pub op: [Operator; OPERATOR_COUNT],
    pub chip_timer: u16,
    pub tremolo_pos: u8,
    pub tremolo_level: u8,
    pub tremolo_shift: u8,
    pub vib_pos: u8,
    pub vib_shift: u8,
    pub eg_timer: u64,
    pub eg_state: u8,
    pub eg_add: u8,
    pub eg_timer_lo: u8,
    pub eg_timer_rem: u8,
    pub note_sel: bool,
    pub opl3_enabled: bool,
    pub rhythm_mode: bool,
    pub waveform_select: bool,
    pub composite_sine_wave: bool,
    pub noise: u32,
    pub rm_hh_bit2: u8,
    pub rm_hh_bit3: u8,
    pub rm_hh_bit7: u8,
    pub rm_hh_bit8: u8,
    pub rm_tc_bit3: u8,
    pub rm_tc_bit5: u8,
    pub rhythm_reg: u8,
    pub timer: [u8; 2],
    pub timer_control: u8,
    pub timer_count: [u16; 2],
    pub status: u8,
    pub write_buf: [WriteBuf; WRITE_BUF_SIZE],
    pub write_buf_sample_count: u64,
    pub write_buf_cur: u32,
    pub write_buf_last: u32,
    pub write_buf_last_time: u64,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Opal>();
};

impl Opal {
    /// Returns an initialized instance at the given output sample rate in hertz.
    ///
    /// The chip state is about 70 KiB. For stack-sensitive call sites use
    /// [`Opal::boxed`] instead.
    ///
    /// A rate of `0` is remapped by the C core to [`OPL3_SAMPLE_RATE`]. Values
    /// that do not fit in a C `int` are clamped to `c_int::MAX`.
    #[must_use]
    pub fn new(sample_rate_hz: u32) -> Self {
        let mut chip = MaybeUninit::<Self>::uninit();
        // Safety: opalInit fully initializes the instance.
        unsafe {
            ffi::opalInit(chip.as_mut_ptr(), c_sample_rate(sample_rate_hz));
            chip.assume_init()
        }
    }

    /// Allocates and initializes an instance on the heap.
    ///
    /// Prefer this over [`Opal::new`] when the ~70 KiB chip state should not
    /// live on the stack.
    #[must_use]
    pub fn boxed(sample_rate_hz: u32) -> Box<Self> {
        let mut chip = Box::<Self>::new_uninit();
        // Safety: opalInit fully initializes the instance before assume_init.
        unsafe {
            ffi::opalInit(chip.as_mut_ptr(), c_sample_rate(sample_rate_hz));
            chip.assume_init()
        }
    }

    /// Changes the output sample rate of an initialized instance.
    ///
    /// Also resets the output resampler and the buffered-write queue. A rate
    /// of `0` is remapped by the C core to [`OPL3_SAMPLE_RATE`]. Values that
    /// do not fit in a C `int` are clamped to `c_int::MAX`.
    #[inline]
    pub fn set_sample_rate(&mut self, sample_rate_hz: u32) {
        // Safety: self is a valid initialized Opal instance.
        unsafe { ffi::opalSetSampleRate(&raw mut *self, c_sample_rate(sample_rate_hz)) }
    }

    /// Writes a register immediately, on the same sample as the call.
    #[inline]
    pub fn write_reg(&mut self, reg: u16, value: u8) {
        // Safety: self is a valid initialized Opal instance.
        unsafe { ffi::opalWriteReg(&raw mut *self, reg, value) }
    }

    /// Writes many registers immediately, in iterator order.
    #[inline]
    pub fn write_regs(&mut self, regs: impl IntoIterator<Item = (u16, u8)>) {
        for (reg, value) in regs {
            self.write_reg(reg, value);
        }
    }

    /// Queues a register write.
    ///
    /// Queued writes apply [`WRITE_BUF_DELAY`] chip samples apart, matching
    /// real chip timing.
    #[inline]
    pub fn write_reg_buffered(&mut self, reg: u16, value: u8) {
        // Safety: self is a valid initialized Opal instance.
        unsafe { ffi::opalWriteRegBuffered(&raw mut *self, reg, value) }
    }

    /// Queues many register writes, in iterator order.
    #[inline]
    pub fn write_regs_buffered(&mut self, regs: impl IntoIterator<Item = (u16, u8)>) {
        for (reg, value) in regs {
            self.write_reg_buffered(reg, value);
        }
    }

    /// Applies any queued register writes immediately.
    #[inline]
    pub fn flush_write_buf(&mut self) {
        // Safety: self is a valid initialized Opal instance.
        unsafe { ffi::opalFlushWriteBuf(&raw mut *self) }
    }

    /// Sets the panning level for a channel: 0 left, 64 middle, 127 right.
    ///
    /// `ch` is 0 to 17, or 0 to 8 with bit 8 set for the second bank.
    #[inline]
    pub fn pan(&mut self, ch: u16, level: u8) {
        // Safety: self is a valid initialized Opal instance.
        unsafe { ffi::opalPan(&raw mut *self, ch, level) }
    }

    /// Generates one stereo 16-bit PCM sample.
    #[inline]
    #[must_use]
    pub fn sample(&mut self) -> StereoSample {
        let mut out = StereoSample::default();
        // Safety: self is valid. Out pointers refer to local storage.
        unsafe {
            ffi::opalSample(&raw mut *self, &raw mut out.left, &raw mut out.right);
        }
        out
    }

    /// Fills a buffer with interleaved stereo frames (`L, R, L, R, ...`).
    ///
    /// # Panics
    ///
    /// Panics if `interleaved.len()` is not a multiple of 2.
    pub fn render(&mut self, interleaved: &mut [i16]) {
        let (frames, remainder) = interleaved.as_chunks_mut::<2>();
        assert!(
            remainder.is_empty(),
            "interleaved buffer length must be a multiple of 2"
        );
        for frame in frames {
            // Safety: self is valid. Frame elements are distinct mutable slots.
            unsafe {
                ffi::opalSample(&raw mut *self, &raw mut frame[0], &raw mut frame[1]);
            }
        }
    }

    /// Fills `frames` with consecutive stereo samples.
    pub fn render_stereo(&mut self, frames: &mut [StereoSample]) {
        for frame in frames {
            *frame = self.sample();
        }
    }

    /// Reads the chip status register.
    ///
    /// Bit 7 is IRQ, bit 6 is timer 1 overflow, bit 5 is timer 2 overflow.
    /// Flags stay set until a reset write (bit 7 of register 4), as on real
    /// hardware. See [`STATUS_IRQ`], [`STATUS_TIMER1`], and [`STATUS_TIMER2`].
    #[inline]
    #[must_use]
    pub fn read_status(&self) -> u8 {
        // Safety: opalReadStatus only reads the status byte.
        unsafe { ffi::opalReadStatus((&raw const *self).cast_mut()) }
    }
}

fn c_sample_rate(sample_rate_hz: u32) -> c_int {
    c_int::try_from(sample_rate_hz).unwrap_or(c_int::MAX)
}

impl fmt::Debug for Opal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opal")
            .field("sample_rate", &self.sample_rate)
            .field("opl3_enabled", &self.opl3_enabled)
            .field("rhythm_mode", &self.rhythm_mode)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Raw C API, matching `include/opal/opal.h` one for one.
///
/// Prefer the methods on [`Opal`] unless you need the C surface directly.
pub mod ffi {
    use super::Opal;
    use core::ffi::c_int;

    unsafe extern "C" {
        /// Fully initializes `*opal` at `sample_rate` hertz.
        ///
        /// # Safety
        ///
        /// `opal` must be valid for writes of `size_of::<Opal>()` bytes.
        pub fn opalInit(opal: *mut Opal, sample_rate: c_int);

        /// Changes the sample rate of an initialized instance.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalSetSampleRate(opal: *mut Opal, sample_rate: c_int);

        /// Writes a register immediately.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalWriteReg(opal: *mut Opal, reg_num: u16, val: u8);

        /// Queues a register write.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalWriteRegBuffered(opal: *mut Opal, reg_num: u16, val: u8);

        /// Applies any queued register writes immediately.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalFlushWriteBuf(opal: *mut Opal);

        /// Sets per-channel panning.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalPan(opal: *mut Opal, channel: u16, pan: u8);

        /// Writes one stereo sample to `left` and `right`.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`]. `left` and `right`
        /// must be valid for writes.
        pub fn opalSample(opal: *mut Opal, left: *mut i16, right: *mut i16);

        /// Reads the status register. The C function takes `*mut` but does not write.
        ///
        /// # Safety
        ///
        /// `opal` must point to an initialized [`Opal`].
        pub fn opalReadStatus(opal: *mut Opal) -> u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    #[link(name = "opal_abi", kind = "static")]
    unsafe extern "C" {
        static opal_abi_sizes: [usize; 4];
        static opal_abi_counts: [usize; 4];
        static opal_abi_version: [c_int; 4];
        static opal_abi_write_buf_offsets: [usize; 3];
        static opal_abi_operator_offsets: [usize; 26];
        static opal_abi_channel_offsets: [usize; 17];
        static opal_abi_opal_offsets: [usize; 40];
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn struct_layouts_match_the_c_compiler() {
        // Safety: symbols are defined by abi_check.c, linked for tests only.
        unsafe {
            assert_eq!(opal_abi_counts[0], 3);
            assert_eq!(opal_abi_counts[1], 26);
            assert_eq!(opal_abi_counts[2], 17);
            assert_eq!(opal_abi_counts[3], 40);

            assert_eq!(size_of::<WriteBuf>(), opal_abi_sizes[0]);
            assert_eq!(size_of::<Operator>(), opal_abi_sizes[1]);
            assert_eq!(size_of::<Channel>(), opal_abi_sizes[2]);
            assert_eq!(size_of::<Opal>(), opal_abi_sizes[3]);

            let write_buf = [
                offset_of!(WriteBuf, time),
                offset_of!(WriteBuf, reg),
                offset_of!(WriteBuf, data),
            ];
            assert_eq!(write_buf, opal_abi_write_buf_offsets);

            let operator = [
                offset_of!(Operator, slot_index),
                offset_of!(Operator, chan_index),
                offset_of!(Operator, mod_source),
                offset_of!(Operator, phase),
                offset_of!(Operator, phase_out),
                offset_of!(Operator, phase_reset),
                offset_of!(Operator, waveform),
                offset_of!(Operator, freq_mult_times2),
                offset_of!(Operator, envelope_stage),
                offset_of!(Operator, envelope_level),
                offset_of!(Operator, eg_out),
                offset_of!(Operator, eg_ksl),
                offset_of!(Operator, output_level),
                offset_of!(Operator, attack_rate),
                offset_of!(Operator, decay_rate),
                offset_of!(Operator, sustain_level),
                offset_of!(Operator, release_rate),
                offset_of!(Operator, key_scale_reg),
                offset_of!(Operator, key),
                offset_of!(Operator, out),
                offset_of!(Operator, prev_out),
                offset_of!(Operator, fb_mod),
                offset_of!(Operator, key_scale_rate),
                offset_of!(Operator, sustain_mode),
                offset_of!(Operator, tremolo_enable),
                offset_of!(Operator, vibrato_enable),
            ];
            assert_eq!(operator, opal_abi_operator_offsets);

            let channel = [
                offset_of!(Channel, op_slot),
                offset_of!(Channel, chan_index),
                offset_of!(Channel, pair_index),
                offset_of!(Channel, out_source),
                offset_of!(Channel, chan_type),
                offset_of!(Channel, freq),
                offset_of!(Channel, octave),
                offset_of!(Channel, key_scale_number),
                offset_of!(Channel, feedback_shift),
                offset_of!(Channel, modulation_type),
                offset_of!(Channel, alg),
                offset_of!(Channel, cha),
                offset_of!(Channel, chb),
                offset_of!(Channel, chc),
                offset_of!(Channel, chd),
                offset_of!(Channel, left_pan),
                offset_of!(Channel, right_pan),
            ];
            assert_eq!(channel, opal_abi_channel_offsets);

            let opal = [
                offset_of!(Opal, sample_rate),
                offset_of!(Opal, sample_accum),
                offset_of!(Opal, last_output),
                offset_of!(Opal, curr_output),
                offset_of!(Opal, mix_buff_right),
                offset_of!(Opal, chan),
                offset_of!(Opal, op),
                offset_of!(Opal, chip_timer),
                offset_of!(Opal, tremolo_pos),
                offset_of!(Opal, tremolo_level),
                offset_of!(Opal, tremolo_shift),
                offset_of!(Opal, vib_pos),
                offset_of!(Opal, vib_shift),
                offset_of!(Opal, eg_timer),
                offset_of!(Opal, eg_state),
                offset_of!(Opal, eg_add),
                offset_of!(Opal, eg_timer_lo),
                offset_of!(Opal, eg_timer_rem),
                offset_of!(Opal, note_sel),
                offset_of!(Opal, opl3_enabled),
                offset_of!(Opal, rhythm_mode),
                offset_of!(Opal, waveform_select),
                offset_of!(Opal, composite_sine_wave),
                offset_of!(Opal, noise),
                offset_of!(Opal, rm_hh_bit2),
                offset_of!(Opal, rm_hh_bit3),
                offset_of!(Opal, rm_hh_bit7),
                offset_of!(Opal, rm_hh_bit8),
                offset_of!(Opal, rm_tc_bit3),
                offset_of!(Opal, rm_tc_bit5),
                offset_of!(Opal, rhythm_reg),
                offset_of!(Opal, timer),
                offset_of!(Opal, timer_control),
                offset_of!(Opal, timer_count),
                offset_of!(Opal, status),
                offset_of!(Opal, write_buf),
                offset_of!(Opal, write_buf_sample_count),
                offset_of!(Opal, write_buf_cur),
                offset_of!(Opal, write_buf_last),
                offset_of!(Opal, write_buf_last_time),
            ];
            assert_eq!(opal, opal_abi_opal_offsets);
        }
    }

    #[test]
    fn version_macros_match_the_c_header() {
        // Safety: symbols are defined by abi_check.c, linked for tests only.
        unsafe {
            assert_eq!(opal_abi_version[0], VERSION_MAJOR as c_int);
            assert_eq!(opal_abi_version[1], VERSION_MINOR as c_int);
            assert_eq!(opal_abi_version[2], VERSION_PATCH as c_int);
            assert_eq!(opal_abi_version[3], VERSION as c_int);
        }
        assert_eq!(VERSION_MAJOR, 2);
        assert_eq!(VERSION_MINOR, 0);
        assert_eq!(VERSION_PATCH, 2);
        assert_eq!(VERSION, 20_002);
    }

    const TEST_VOICE: &[(u16, u8)] = &[
        (0x105, 0x01),
        (0x20, 0x21),
        (0x40, 0x3F),
        (0x60, 0xF0),
        (0x80, 0x0F),
        (0x23, 0x21),
        (0x43, 0x00),
        (0x63, 0xF0),
        (0x83, 0x0F),
        (0xC0, 0x31),
        (0xA0, 0x44),
        (0xB0, 0x32),
    ];

    fn channel_energy(frames: &[i16]) -> (u64, u64) {
        frames.chunks_exact(2).fold((0, 0), |(left, right), frame| {
            (
                left + u64::from(frame[0].unsigned_abs()),
                right + u64::from(frame[1].unsigned_abs()),
            )
        })
    }

    #[test]
    fn initialized_chip_outputs_silence() {
        let mut chip = Opal::new(OPL3_SAMPLE_RATE);
        for _ in 0..256 {
            assert!(chip.sample().is_silent());
        }
        for ch in &chip.chan {
            assert_eq!(ch.chan_type, ChannelType::TWO_OP);
            assert!(!ch.is_paired());
        }
        for op in &chip.op {
            assert!(!op.is_keyed_on());
            assert!(!op.is_active());
            assert_eq!(op.envelope_stage, EnvelopeStage::RELEASE);
            assert_eq!(op.eg_out, 0x1FF);
        }
        for (i, ch) in chip.chan.iter().enumerate() {
            let bank_pos = i % 9;
            if bank_pos < 3 {
                assert_eq!(ch.pair_index, (i + 3) as u8);
            } else if bank_pos < 6 {
                assert_eq!(ch.pair_index, (i - 3) as u8);
            } else {
                assert_eq!(ch.pair_index, CH_NONE);
            }
        }
    }

    #[test]
    fn disabling_4op_leaves_pair_index_in_place() {
        let mut chip = Opal::new(OPL3_SAMPLE_RATE);
        chip.write_reg(0x105, 0x01);
        chip.write_reg(0x104, 0x01);

        assert_eq!(chip.chan[0].chan_type, ChannelType::FOUR_OP);
        assert_eq!(chip.chan[3].chan_type, ChannelType::FOUR_OP2);
        assert_eq!(chip.chan[0].pair_index, 3);
        assert_eq!(chip.chan[3].pair_index, 0);
        assert!(chip.chan[0].is_paired());
        assert!(chip.chan[3].is_paired());
        assert!(!chip.chan[1].is_paired());

        chip.write_reg(0x104, 0x00);
        assert_eq!(chip.chan[0].chan_type, ChannelType::TWO_OP);
        assert_eq!(chip.chan[3].chan_type, ChannelType::TWO_OP);
        assert_eq!(chip.chan[0].pair_index, 3);
        assert_eq!(chip.chan[3].pair_index, 0);
        assert!(!chip.chan[0].is_paired());
        assert!(!chip.chan[3].is_paired());
    }

    #[test]
    fn new_off_keeps_bank_1_silent_and_preserves_cha() {
        let mut chip = Opal::boxed(OPL3_SAMPLE_RATE);
        let bank1_voice = [
            (0x105, 0x01),
            (0x120, 0x21),
            (0x140, 0x3F),
            (0x160, 0xF0),
            (0x180, 0x0F),
            (0x123, 0x21),
            (0x143, 0x00),
            (0x163, 0xF0),
            (0x183, 0x0F),
            (0x1C0, 0x31),
            (0x1A0, 0x44),
            (0x1B0, 0x32),
        ];
        chip.write_regs(bank1_voice);

        assert!(chip.opl3_enabled);
        assert_eq!(chip.chan[9].cha, 0xFFFF);
        assert_eq!(chip.chan[9].chb, 0xFFFF);

        let mut frames = [0i16; 4096];
        chip.render(&mut frames);
        let peak_on = frames.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(peak_on > 1000, "peak_on was {peak_on}");

        chip.write_reg(0x105, 0x00);
        assert!(!chip.opl3_enabled);
        assert_eq!(chip.chan[9].cha, 0xFFFF);
        assert_eq!(chip.chan[9].chb, 0xFFFF);

        chip.render(&mut frames);
        chip.render(&mut frames);
        let peak_off = frames.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert_eq!(peak_off, 0);

        chip.write_reg(0x105, 0x01);
        chip.render(&mut frames);
        let peak_back = frames.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(peak_back > 1000, "peak_back was {peak_back}");
    }

    #[test]
    fn enabling_opl3_without_writing_c0_is_silent() {
        let mut chip = Opal::boxed(OPL3_SAMPLE_RATE);
        chip.write_regs(TEST_VOICE.iter().copied().filter(|&(reg, _)| reg != 0xC0));

        assert!(chip.opl3_enabled);
        assert_eq!(chip.chan[0].cha, 0);
        assert_eq!(chip.chan[0].chb, 0);

        let mut frames = [0i16; 4096];
        chip.render(&mut frames);
        assert!(frames.iter().all(|&s| s == 0));
    }

    #[test]
    fn waveform_4_is_stored_while_new_is_clear() {
        let mut chip = Opal::new(OPL3_SAMPLE_RATE);
        chip.write_reg(0xE0, 0x04);
        assert_eq!(chip.op[0].waveform, 4);
        assert!(!chip.opl3_enabled);
    }

    #[test]
    fn masked_timer_overflow_still_sets_the_timer_flag() {
        let mut chip = Opal::new(OPL3_SAMPLE_RATE);
        chip.write_reg(0x02, 0xFF);
        chip.write_reg(0x04, 0x41);

        let mut frames = [0i16; 32];
        chip.render(&mut frames);

        let status = chip.read_status();
        assert_eq!(status & 0x40, 0x40);
        assert_eq!(status & 0x80, 0);
    }

    #[test]
    fn keyed_on_voice_produces_audio_and_exposes_operator_state() {
        let mut chip = Opal::boxed(OPL3_SAMPLE_RATE);
        chip.write_regs(TEST_VOICE.iter().copied());

        let mut frames = [0i16; 8192];
        chip.render(&mut frames);

        let peak = frames.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(peak > 1000, "peak was {peak}");

        let carrier = chip.op[carrier_slot(0)];
        assert!(carrier.is_keyed_on());
        assert!(carrier.is_active());
        assert_eq!(chip.chan[0].op_slot[0], modulator_slot(0) as u8);
        assert_eq!(chip.chan[0].op_slot[1], carrier_slot(0) as u8);
        assert_eq!(chip.chan[0].chan_type, ChannelType::TWO_OP);
        assert_eq!(chip.chan[0].chan_type.name(), Some("2op"));
        assert!(carrier.envelope_stage.name().is_some());
        assert!(!chip.chan[0].is_paired());
    }

    #[test]
    fn a_clone_is_a_complete_save_state() {
        let mut chip = Opal::boxed(OPL3_SAMPLE_RATE);
        chip.write_regs(TEST_VOICE.iter().copied());
        let mut warmup = [0i16; 2048];
        chip.render(&mut warmup);

        let mut copy = chip.as_ref().clone();
        let mut original_frames = [0i16; 2048];
        let mut copy_frames = [0i16; 2048];
        chip.render(&mut original_frames);
        copy.render(&mut copy_frames);
        assert_eq!(original_frames, copy_frames);
    }

    #[test]
    fn buffered_writes_match_immediate_writes_after_flush() {
        let mut immediate = Opal::boxed(OPL3_SAMPLE_RATE);
        let mut buffered = Opal::boxed(OPL3_SAMPLE_RATE);

        immediate.write_regs(TEST_VOICE.iter().copied());
        buffered.write_regs_buffered(TEST_VOICE.iter().copied());
        buffered.flush_write_buf();

        let mut immediate_frames = [0i16; 2048];
        let mut buffered_frames = [0i16; 2048];
        immediate.render(&mut immediate_frames);
        buffered.render(&mut buffered_frames);
        assert_eq!(immediate_frames, buffered_frames);
    }

    #[test]
    fn resampler_interpolates_a_full_scale_step_without_overflowing() {
        let mut chip = Opal::boxed(44_100);
        chip.sample_accum = 35281;
        chip.last_output = [8734, -8734];
        chip.curr_output = [-32768, 32767];

        let s = chip.sample();
        assert_eq!(s.left, -24468);
        assert_eq!(s.right, 24467);
        assert!(s.left > chip.curr_output[0] && s.left < chip.last_output[0]);
        assert!(s.right > chip.last_output[1] && s.right < chip.curr_output[1]);
    }

    #[test]
    fn pan_moves_output_between_stereo_channels() {
        let mut chip = Opal::boxed(OPL3_SAMPLE_RATE);
        chip.write_regs(TEST_VOICE.iter().copied());
        let mut frames = [0i16; 4096];

        chip.pan(0, 0);
        chip.render(&mut frames);
        let (hard_left_l, hard_left_r) = channel_energy(&frames);
        assert!(
            hard_left_l > hard_left_r * 4,
            "left={hard_left_l} right={hard_left_r}"
        );

        chip.pan(0, 127);
        chip.render(&mut frames);
        let (hard_right_l, hard_right_r) = channel_energy(&frames);
        assert!(
            hard_right_r > hard_right_l * 4,
            "left={hard_right_l} right={hard_right_r}"
        );
    }

    #[test]
    fn render_stereo_matches_interleaved_render() {
        let mut a = Opal::boxed(OPL3_SAMPLE_RATE);
        let mut b = Opal::boxed(OPL3_SAMPLE_RATE);
        a.write_regs(TEST_VOICE.iter().copied());
        b.write_regs(TEST_VOICE.iter().copied());

        let mut interleaved = [0i16; 512];
        let mut stereo = [StereoSample::default(); 256];
        a.render(&mut interleaved);
        b.render_stereo(&mut stereo);

        for (i, frame) in stereo.iter().enumerate() {
            assert_eq!(frame.left, interleaved[i * 2]);
            assert_eq!(frame.right, interleaved[i * 2 + 1]);
        }
    }

    #[test]
    fn sample_rate_round_trips_as_u32() {
        let mut chip = Opal::new(48_000);
        assert_eq!(chip.sample_rate, 48_000);
        chip.set_sample_rate(44_100);
        assert_eq!(chip.sample_rate, 44_100);
    }

    #[test]
    fn sample_rate_zero_uses_native_opl3_rate() {
        let chip = Opal::new(0);
        assert_eq!(chip.sample_rate, OPL3_SAMPLE_RATE as i32);
    }

    #[test]
    fn sample_rate_too_large_does_not_wrap_negative() {
        let chip = Opal::new(u32::MAX);
        assert!(chip.sample_rate > 0);
        assert_eq!(chip.sample_rate, i32::MAX);
    }
}
