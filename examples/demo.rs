//! Renders a short FM arpeggio to opal-demo.wav in the working directory.
//!
//! Copyright (c) 2026 Bitdancer (github.com/RealBitdancer).
//! SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use opal::Opal;

const SAMPLE_RATE: u32 = 44_100;
const GATE_FRAMES: usize = SAMPLE_RATE as usize * 35 / 100;
const TAIL_FRAMES: usize = SAMPLE_RATE as usize * 15 / 100;
const RELEASE_FRAMES: usize = SAMPLE_RATE as usize / 2;

struct Note {
    block: u8,
    fnum: u16,
}

const MELODY: &[Note] = &[
    Note {
        block: 4,
        fnum: 580,
    },
    Note {
        block: 4,
        fnum: 690,
    },
    Note {
        block: 4,
        fnum: 869,
    },
    Note {
        block: 5,
        fnum: 580,
    },
    Note {
        block: 4,
        fnum: 869,
    },
    Note {
        block: 4,
        fnum: 690,
    },
    Note {
        block: 4,
        fnum: 580,
    },
];

const PATCH: &[(u16, u8)] = &[
    (0x105, 0x01),
    (0x20, 0x21),
    (0x40, 0x1A),
    (0x60, 0xF5),
    (0x80, 0x37),
    (0xE0, 0x00),
    (0x23, 0x21),
    (0x43, 0x00),
    (0x63, 0xF5),
    (0x83, 0x17),
    (0xE3, 0x00),
    (0xC0, 0x38),
];

fn main() -> io::Result<()> {
    let mut chip = Opal::boxed(SAMPLE_RATE);
    chip.write_regs(PATCH.iter().copied());

    let total_frames = MELODY.len() * (GATE_FRAMES + TAIL_FRAMES) + RELEASE_FRAMES;
    let mut frames = vec![0i16; total_frames * 2];

    let mut pos = 0;
    for note in MELODY {
        let freq_hi = ((u16::from(note.block) << 2) | (note.fnum >> 8)) as u8;
        chip.write_reg(0xA0, (note.fnum & 0xFF) as u8);
        chip.write_reg(0xB0, 0x20 | freq_hi);
        chip.render(&mut frames[pos..pos + GATE_FRAMES * 2]);
        pos += GATE_FRAMES * 2;
        chip.write_reg(0xB0, freq_hi);
        chip.render(&mut frames[pos..pos + TAIL_FRAMES * 2]);
        pos += TAIL_FRAMES * 2;
    }
    chip.render(&mut frames[pos..]);

    write_wav("opal-demo.wav", &frames, SAMPLE_RATE)?;

    let peak = frames.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    println!("wrote opal-demo.wav: {total_frames} stereo frames at {SAMPLE_RATE} Hz, peak {peak}");
    Ok(())
}

fn write_wav(path: impl AsRef<Path>, interleaved: &[i16], rate: u32) -> io::Result<()> {
    let data_bytes = u32::try_from(interleaved.len() * 2).expect("wav data fits in u32");
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36 + data_bytes).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes());
    header[22..24].copy_from_slice(&2u16.to_le_bytes());
    header[24..28].copy_from_slice(&rate.to_le_bytes());
    header[28..32].copy_from_slice(&(rate * 4).to_le_bytes());
    header[32..34].copy_from_slice(&4u16.to_le_bytes());
    header[34..36].copy_from_slice(&16u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_bytes.to_le_bytes());

    let mut file = File::create(path)?;
    file.write_all(&header)?;

    let mut bytes = Vec::with_capacity(interleaved.len() * 2);
    for sample in interleaved {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    file.write_all(&bytes)?;
    Ok(())
}
