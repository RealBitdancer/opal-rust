# Changelog

All notable changes to this binding are documented here. The version is the bound opal
release plus a packaging revision. `1.0.2-1` is the first release that binds opal `1.0.2`.

## [3.0.0-1] - 2026-10-24

### Changed

- Binds opal 3.0.0. The emulation core is new. The eight C functions keep
  their names and C signatures. Rendered output is not bit-identical to 2.0.2.
- `Opal` is now a wrapper around the raw mirror, which moved to
  `opal::ffi::Opal`. It derefs to it, so reads such as `chip.op[3].eg_out`
  compile unchanged. Field writes and struct literals no longer compile. The C
  core trusts fields such as `algorithm`, `multiple`, `f_number`,
  `queue_head`, and `sample_rate`, so a bad value written from safe code was
  undefined behavior, in 2.0.2-2 as well. The `unsafe` methods
  `Opal::raw_mut` and `Opal::from_raw` give write access. Their safety
  sections list the fields that must stay in range. The `opal::ffi`
  functions now take `*mut ffi::Opal` instead of `*mut Opal`, so a call that
  passed `&raw mut chip` passes `chip.raw_mut()` instead.
- Struct mirrors follow the new layout. The visualizer fields keep their names
  and meaning. Those are `envelope_stage`, `eg_out`, `key`, `mod_source`, and
  `out` on `Operator`, and `op_slot`, `pair_index`, `out_source`, `chan_type`,
  `left_pan`, and `right_pan` on `Channel`. The core no longer reads
  `mod_source`, `out_source`, or `chan_type`, so writing them no longer
  changes the sound. Other fields may be renamed, retyped, or rescaled.
- `WriteBuf` is now `QueuedWrite`, with fields `due`, `reg_num`, and `value`.
  `output_enable` replaces the old per-speaker flags. `Opal::opl3_enabled` is
  now `new_mode`. `sample_counter` replaces `chip_timer`. `tremolo_depth` and
  `vibrato_depth` hold the BDh bits that `tremolo_shift` and `vib_shift`
  encoded as shifts. `hi_hat_phase` and `cymbal_phase` hold the phases that
  the `rm_*` bits sampled. The write-buffer cursors are `queue`, `queue_head`,
  `queue_count`, `chip_samples`, and `last_due`. Fields that used to be `bool`
  are `u8`, matching `uint8_t` in the header.
- Operator and channel register fields hold raw register values.
  `sustain_level` keeps its name but is now a `u8` from 0 to 15 instead of a
  scaled `u16` level. Reads that don't name its type still compile and get
  different numbers. `total_level`, `multiple`, `feedback`, `f_number`, and
  `block` replace `output_level`, `freq_mult_times2`, `feedback_shift`,
  `freq`, and `octave`. `algorithm` replaces `alg` and uses a new encoding.
  The envelope rates and `waveform` are `u8` too.
- NEW (register 105h bit 0) still gates new writes. While it is clear, bank 1
  writes other than 105h land in bank 0, E0h writes keep only waveforms 0-3,
  and a C0h write turns on both speakers. Changing NEW no longer rewrites
  stored state or how it plays, so a waveform 4-7 written while NEW was set
  keeps playing after NEW is cleared. All 18 channels keep playing, and
  four-operator pairs follow register 104h alone.
- A masked timer sets nothing. Setting a timer's mask bit clears a flag that
  is already set. IRQ stays set only while a flag is set.
- `Opal::new` and `set_sample_rate` treat a rate of 0 as 49716 Hz. Anything
  above `i32::MAX - 49716` clamps to that limit. A `u32` that does not fit in
  a C `int` is passed as `c_int::MAX`, which the core then clamps.
  `set_sample_rate` no longer clears the write queue or the output history, so
  a queued write survives it. It still resets the resampler phase in
  `sample_accum`. `pan` clamps a level above 127.
- `Opal::new` still leaves every operator in release with `eg_out` at 0x1FF.
  It now enables both speakers on every channel, so a voice is audible without
  a C0 write. The instance is about 66 KiB.

### Added

- Tests for the 3.0.0 behavior. NEW leaves stored state alone. Bank 1 writes
  reach bank 1, and waveforms 4-7 are stored, only while NEW is set.
  Four-operator pairing stays after NEW is cleared. Masked timers set nothing. Sample-rate clamping, the
  write queue surviving `set_sample_rate`, and pan clamping are covered too.
  So is a raw copy wrapped back with `from_raw`, and a `compile_fail` doctest
  checks that safe code cannot write chip state.
- Notes on the update are in `doc/3.0.0.md`.

### Removed

- `Operator::slot_index`, `Operator::chan_index`, and `Channel::chan_index`.
  `op_slot`, `modulator_slot`, and `carrier_slot` map channels to operators.
- Internal state that upstream dropped, including `phase_out`, `phase_reset`,
  `eg_ksl`, `fb_mod`, `cha`, `chb`, `chc`, `chd`, `vib_pos`, the `eg_*`
  envelope timer fields, and `rhythm_reg`.

## [2.0.2-2] - 2026-08-14

### Added

- First public release of the Rust binding. Binds opal 2.0.2.
- The `opal` crate compiles vendored C11 sources from `vendor/opal/`. There is no
  bindgen step. Set `OPAL_SRC` to compile a local checkout instead. The crate sets
  `links = "opal"` and is `no_std` plus `alloc`.
- `Opal`, `Channel`, `Operator`, and `WriteBuf` are hand-mirrored as `#[repr(C)]`
  with snake_case field names. Tests check every size and field offset against the
  C compiler.
- Methods for the eight C functions: `new`, `set_sample_rate`, `write_reg`,
  `write_reg_buffered`, `flush_write_buf`, `pan`, `sample`, and `read_status`.
  `boxed` heap-allocates the ~70 KiB state. `write_regs` and `write_regs_buffered`
  take iterator batches. `render` and `render_stereo` fill interleaved or typed
  stereo buffers.
- `EnvelopeStage` and `ChannelType` transparent newtypes (`Display`, `From<c_int>`).
  Index sentinels `OP_NONE`, `MOD_OWN_FB`, and `CH_NONE`. Status bit constants.
  `modulator_slot` and `carrier_slot` match the upstream header table.
- `VERSION_MAJOR`, `VERSION_MINOR`, `VERSION_PATCH`, and packed `VERSION`
  (`major * 10000 + minor * 100 + patch`), matching `OPAL_VERSION_*` in `opal.h`.
- Public sample-rate arguments are `u32` hertz. A rate of `0` becomes the native
  OPL3 rate. Values that do not fit in a C `int` are clamped.
- Edition 2024. MSRV is current stable (1.97).
- NEW (register 105h bit 0) is a live mode bit. Bank 1, waveforms 4-7, CHA and
  CHB, and four-operator pairing apply only while it is set.
- `pair_index` is the physical partner, or `CH_NONE` if the channel cannot pair.
  `chan_type` says whether 4-op is active.
- After `Opal::new`, every operator's `eg_out` is 0x1FF.
- A masked timer overflow sets FT1 or FT2. IRQ (bit 7) is set only when that
  timer is unmasked.
- Demo: `cargo run --example demo` writes a short FM arpeggio to `opal-demo.wav`.
- Tests cover silence after init, pair-index wiring, NEW as a live mode bit, tone
  generation, save-state clones, buffered write equivalence, resampler full-scale
  step regression, pan, stereo render parity, timer flags, and sample-rate
  round-trip (`0` and overflow clamp).
