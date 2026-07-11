//! Minimal 16-bit PCM WAV encoder for audio-stem export.
//!
//! Pure and dependency-free: the mixed timeline audio (see [`crate::audio_mixer`])
//! can be written to a standard `.wav` without pulling in ffmpeg. Encoding is
//! split from file I/O so the byte layout is unit-tested directly.

/// Encode interleaved f32 PCM (`-1.0..=1.0`) as a 16-bit PCM WAV byte stream.
/// Samples are clamped and rounded to `i16`.
pub fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let block_align = channels as u32 * bytes_per_sample;
    let byte_rate = sample_rate * block_align;
    let data_len = samples.len() as u32 * bytes_per_sample;

    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&(block_align as u16).to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Write interleaved f32 PCM to `path` as a 16-bit PCM WAV.
pub fn write_wav(
    path: &std::path::Path,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let bytes = encode_wav(samples, sample_rate, channels);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, bytes).map_err(|e| format!("write wav: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read back (sample_rate, channels, i16 samples) from a PCM WAV — test only.
    fn parse_wav(bytes: &[u8]) -> (u32, u16, Vec<i16>) {
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        assert_eq!(&bytes[36..40], b"data");
        let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
        let mut samples = Vec::with_capacity(data_len / 2);
        for chunk in bytes[44..44 + data_len].chunks_exact(2) {
            samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
        }
        (sample_rate, channels, samples)
    }

    #[test]
    fn header_declares_rate_and_channels() {
        let wav = encode_wav(&[0.0, 0.0], 48_000, 2);
        let (rate, channels, samples) = parse_wav(&wav);
        assert_eq!(rate, 48_000);
        assert_eq!(channels, 2);
        assert_eq!(samples, vec![0, 0]);
    }

    #[test]
    fn full_scale_and_clamping() {
        let wav = encode_wav(&[1.0, -1.0, 2.0, -2.0], 44_100, 1);
        let (_, _, samples) = parse_wav(&wav);
        // +1.0 → 32767, -1.0 → -32767, over-range clamps to the same.
        assert_eq!(samples, vec![32767, -32767, 32767, -32767]);
    }

    #[test]
    fn midscale_rounds() {
        let wav = encode_wav(&[0.5], 8_000, 1);
        let (_, _, samples) = parse_wav(&wav);
        assert_eq!(samples[0], (0.5 * 32767.0f32).round() as i16); // 16384
    }

    #[test]
    fn data_length_matches_sample_count() {
        let wav = encode_wav(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 48_000, 2);
        // 44-byte header + 6 samples * 2 bytes.
        assert_eq!(wav.len(), 44 + 12);
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join("fronda-wav-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stem.wav");
        write_wav(&path, &[0.25, -0.25], 22_050, 2).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let (rate, channels, samples) = parse_wav(&bytes);
        assert_eq!(rate, 22_050);
        assert_eq!(channels, 2);
        assert_eq!(samples.len(), 2);
    }
}
