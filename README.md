# opal-rust

[![Test](https://github.com/RealBitdancer/opal-rust/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/RealBitdancer/opal-rust/actions/workflows/test.yml)

This crate binds [opal](https://github.com/RealBitdancer/opal), the OPL3 (Yamaha YMF262) and OPL2 FM synthesis emulator, to Rust. The Cargo build script compiles the vendored C11 core and exposes it as a native crate. Bindgen is not used. The compiled crate links that core statically. There is no Rust runtime dependency. The crate is `no_std` plus `alloc`.

Versions encode the pinned opal release and a packaging revision. This package is 3.0.0-1 and binds opal 3.0.0. Binding-only fixes raise the revision. A newer opal release starts the revision over.

Cargo treats the `-1` revision as a pre-release tag. Dependents must write the full version (`opal = "3.0.0-1"`). A requirement such as `opal = "3"` will not match. Notes on this update are in [doc/3.0.0.md](doc/3.0.0.md).

## Requirements

- Rust **1.97** or newer, edition 2024
- A C11 compiler (whichever Cargo's `cc` crate finds on the host)

## Installation

```toml
[dependencies]
opal = "3.0.0-1"
```

To pin the matching git tag instead:

```toml
[dependencies]
opal = { git = "https://github.com/RealBitdancer/opal-rust", tag = "v3.0.0-1" }
```

## Usage

```rust
use opal::Opal;

let mut chip = Opal::new(44_100);

chip.write_reg(0x105, 0x01); // OPL3 mode
// ... program a voice, key on a note ...

let mut frames = [0i16; 2048]; // interleaved stereo
chip.render(&mut frames);
```

`Opal` is an ordinary value with no internal pointers. You may clone, assign, or move it without restriction. A clone is a full save state. The struct is about 66 KiB, so `Opal::boxed(rate)` returns a `Box<Opal>` without placing the entire chip on the stack.

While NEW (register 105h bit 0) is clear, writes to the second register array land in the first (except 105h itself), E0h writes keep only waveforms 0-3, and C0h writes turn on both speakers. Changing NEW does not touch anything already set, including a waveform 4-7 written earlier. All 18 channels keep playing, and four-operator pairs follow register 104h alone.

To compile a local opal checkout instead of the vendored 3.0.0 sources, set `OPAL_SRC` to a tree that contains `include/opal/opal.h` and `src/opal.c`.

## API

Each of the eight C entry points maps to methods on `opal::Opal`:

| C                      | Rust                                           |
| :--------------------- | :--------------------------------------------- |
| `opalInit`             | `Opal::new`, or `Opal::boxed`                  |
| `opalSetSampleRate`    | `Opal::set_sample_rate`                        |
| `opalWriteReg`         | `Opal::write_reg`, `Opal::write_regs`          |
| `opalWriteRegBuffered` | `Opal::write_reg_buffered`, `write_regs_buffered` |
| `opalFlushWriteBuf`    | `Opal::flush_write_buf`                        |
| `opalPan`              | `Opal::pan`                                    |
| `opalSample`           | `Opal::sample`, `render`, `render_stereo`      |
| `opalReadStatus`       | `Opal::read_status`                            |

Public sample-rate arguments take `u32` hertz. A rate of `0` is remapped by the C core to the native OPL3 rate. Rates above `i32::MAX - 49716` clamp to that limit. A value that does not fit in a C `int` is passed as `c_int::MAX`, which the core then clamps. `set_sample_rate` resets the resampler phase and leaves the buffered-write queue in place. `render` writes interleaved `i16` frames. `render_stereo` writes `[StereoSample]`. Batch helpers take any iterator of `(reg, value)` pairs. The original `opal*` symbols stay available as `opal::ffi::*`.

`opal::ffi::Opal` and the crate-root `opal::Channel`, `opal::Operator`, and `opal::QueuedWrite` are hand-mirrored as `#[repr(C)]` with snake_case fields. `Opal` wraps `ffi::Opal` and derefs to it, so visualizer state such as `chip.op[3].eg_out` reads directly. Safe code cannot write it. The C core uses several fields unchecked as array indices and shift counts, and a bad value would be undefined behavior. The `unsafe` methods `Opal::raw_mut` and `Opal::from_raw` give write access to callers that keep those fields in range. `envelope_stage` and `chan_type` use the transparent newtypes `EnvelopeStage` and `ChannelType`. Both implement `Display` and `From<c_int>`.

Links among the structs are indices (`op_slot`, `mod_source`, `pair_index`, `out_source`) with the sentinels `OP_NONE`, `MOD_OWN_FB`, and `CH_NONE`. A cloned instance stays self-contained because of that. `pair_index` is the physical 4-op partner, pre-wired on capable channels. `chan_type` says whether 4-op is active. `output_enable` bit 0 is the left speaker and bit 1 is the right. `Opal::new` sets both. Mode bits and operator flags are `u8`, matching `uint8_t` in the header. For a melodic channel, `modulator_slot(ch)` and `carrier_slot(ch)` return operator indices as documented in the upstream header.

`VERSION_MAJOR`, `VERSION_MINOR`, `VERSION_PATCH`, and the packed `VERSION` match the `OPAL_VERSION_*` macros in `opal.h`. The test suite checks every field offset and struct size against the C compiler. The mirrors cannot drift without failing tests.

## Demo

```sh
cargo run --example demo
```

writes a short FM arpeggio to `opal-demo.wav` in the working directory.

## Tests

```sh
cargo test
```

runs layout checks against the C compiler along with behavioral coverage: silence after init, four-operator pairing, NEW at write time, tone generation, save-state clones, buffered write equivalence, resampler full-scale step regression, pan (including the clamp above 127), stereo render parity, timer flags, the write queue surviving a sample-rate change, sample-rate clamping (`0` and the upper limit), and a raw copy wrapped back with `from_raw`. A `compile_fail` doctest checks that safe code cannot write chip state.

## Updating the bound opal release

1. Copy `include/opal/opal.h` and `src/opal.c` from the new opal tag into `vendor/opal/`.
2. Set `OPAL_VERSION` in `build.rs` to the new tag.
3. Run `cargo test`.
4. If the layout test fails, update the mirrored structs in `src/lib.rs` and the offset
   tables in `src/abi_check.c` together.
5. Set this package's version in `Cargo.toml` to the new opal version with revision
   `-1` and refresh this README.
6. Add a CHANGELOG.md section for the new version. Pushing the `v<version>` tag
   creates the GitHub release with that section as its notes.

## License

MIT, see [LICENSE](LICENSE). opal is MIT too, including its emulation core. Only its output resampler follows Shayde of Reality's public-domain Opal. See the [upstream repository](https://github.com/RealBitdancer/opal). Vendored copies of the C11 sources and their MIT license sit in `vendor/opal/`.
