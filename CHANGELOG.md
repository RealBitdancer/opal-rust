# Changelog

All notable changes to this binding are documented here. The version is the bound opal
release plus a packaging revision. `1.0.2-1` is the first release that binds opal `1.0.2`.

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
