//! Copying to the system clipboard with OSC 52, the terminal escape sequence
//! for clipboard writes. It works locally and over SSH, tmux and herdr
//! without any platform clipboard library.

use std::io::{self, Write};

/// Asks the terminal to put `text` on the clipboard.
pub fn copy(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(osc52(text).as_bytes())?;
    out.flush()
}

/// The OSC 52 sequence that sets the clipboard (`c`) to `text`.
fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in vectors {
            assert_eq!(base64(input.as_bytes()), expected, "{input:?}");
        }
    }

    #[test]
    fn osc52_wraps_the_encoded_text() {
        assert_eq!(osc52("año"), "\x1b]52;c;YcOxbw==\x07");
    }
}
