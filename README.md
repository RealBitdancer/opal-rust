# opal-rust

[![Test](https://github.com/RealBitdancer/opal-rust/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/RealBitdancer/opal-rust/actions/workflows/test.yml)

This crate binds [opal](https://github.com/RealBitdancer/opal), the OPL3 (Yamaha YMF262) and OPL2 FM synthesis emulator, to Rust. The Cargo build script compiles the vendored C11 core and exposes it as a native crate. Bindgen is not used. The compiled crate links that core statically. There is no Rust runtime dependency. The crate is `no_std` plus `alloc`.

Versions encode the pinned opal release and a packaging revision. This package is 2.0.2-2 and binds opal 2.0.2. Binding-only fixes raise the revision. A newer opal release starts the revision over.

Cargo treats the `-2` revision as a pre-release tag. Dependents must write the full version (`opal = "2.0.2-2"`). A requirement such as `opal = "2"` will not match.

## Requirements

- Rust **1.97** (current stable) or newer, edition 2024
- A C11 compiler (whichever Cargo's `cc` crate finds on the host)

## Installation

```toml
[dependencies]
opal = "2.0.2-2"
```

To pin the matching git tag instead:

```toml
[dependencies]
opal = { git = "https://github.com/RealBitdancer/opal-rust", tag = "v2.0.2-2" }
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

`Opal` is an ordinary value with no internal pointers. You may clone, assign, or move it without restriction. A clone is a full save state. The struct is about 70 KiB, so `Opal::boxed(rate)` returns a `Box<Opal>` without placing the entire chip on the stack.

Bank 1, waveforms 4-7, CHA and CHB, and four-operator pairing apply only while NEW (register 105h bit 0) is set.

To compile a local opal checkout instead of the vendored 2.0.2 sources, set `OPAL_SRC` to a tree that contains `include/opal/opal.h` and `src/opal.c`.

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

Public sample-rate arguments take `u32` hertz. A rate of `0` is remapped by the C core to the native OPL3 rate. Values that do not fit in a C `int` are clamped. `set_sample_rate` also resets the resampler and the buffered-write queue. `render` writes interleaved `i16` frames. `render_stereo` writes `[StereoSample]`. Batch helpers take any iterator of `(reg, value)` pairs. The original `opal*` symbols stay available as `opal::ffi::*`.

`Opal`, `Channel`, `Operator`, and `WriteBuf` are hand-mirrored as `#[repr(C)]` with snake_case fields. Visualizer state such as `envelope_stage`, `eg_out`, and `key` is readable directly. `envelope_stage` and `chan_type` use the transparent newtypes `EnvelopeStage` and `ChannelType`. Both implement `Display` and `From<c_int>`.

Links among the structs are indices (`op_slot`, `mod_source`, `pair_index`, `out_source`) with the sentinels `OP_NONE`, `MOD_OWN_FB`, and `CH_NONE`. A cloned instance stays self-contained because of that. `pair_index` is the physical 4-op partner, pre-wired on capable channels. `chan_type` says whether 4-op is active. For a melodic channel, `modulator_slot(ch)` and `carrier_slot(ch)` return operator indices as documented in the upstream header.

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

runs layout checks against the C compiler along with behavioral coverage: silence after init, pair-index wiring, NEW as a live mode bit, tone generation, save-state clones, buffered write equivalence, resampler full-scale step regression, pan, stereo render parity, timer flags, and sample-rate round-trip (including `0` and overflow clamp).

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

MIT, see [LICENSE](LICENSE). The underlying Opal core is public domain, and the C11 port is MIT. Details live in the [upstream repository](https://github.com/RealBitdancer/opal). Vendored copies of the C11 sources and their MIT license sit in `vendor/opal/`.
