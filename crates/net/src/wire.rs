//! Length-prefixed `postcard` wire framing with `lz4_flex` compression for chunks.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terravibe_sim::chunk::{Chunk, ChunkPos};
use terravibe_sim::net::{ClientMsg, ServerMsg};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::MAX_FRAME_BYTES;

const TAG_PLAIN: u8 = 0;
const TAG_COMPRESSED_CHUNK: u8 = 1;

#[derive(Debug, Error)]
pub enum WireError {
    #[error("frame length {len} exceeds max {max}")]
    FrameTooLarge { len: usize, max: usize },
    #[error("truncated frame: expected {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
    #[error("trailing bytes in frame: {0} extra bytes")]
    TrailingBytes(usize),
    #[error("invalid wire tag: {0}")]
    InvalidTag(u8),
    #[error("postcard codec error: {0}")]
    Postcard(#[from] postcard::Error),
    #[error("lz4 decompression error: {0}")]
    Decompress(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize)]
struct CompressedChunkPayload {
    pos: ChunkPos,
    lz4_bytes: Vec<u8>,
}

fn wrap_frame(payload: &[u8]) -> Result<Vec<u8>, WireError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(WireError::FrameTooLarge {
            len: payload.len(),
            max: MAX_FRAME_BYTES,
        });
    }
    let len = payload.len() as u32;
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

fn unwrap_frame(frame: &[u8]) -> Result<&[u8], WireError> {
    if frame.len() < 4 {
        return Err(WireError::Truncated {
            expected: 4,
            actual: frame.len(),
        });
    }
    let len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(WireError::FrameTooLarge {
            len,
            max: MAX_FRAME_BYTES,
        });
    }
    let expected = 4 + len;
    if frame.len() < expected {
        return Err(WireError::Truncated {
            expected,
            actual: frame.len(),
        });
    }
    if frame.len() > expected {
        return Err(WireError::TrailingBytes(frame.len() - expected));
    }
    Ok(&frame[4..expected])
}

/// Encode a `ClientMsg` into a 4-byte LE length-prefixed postcard frame.
pub fn encode_client_msg(msg: &ClientMsg) -> Result<Vec<u8>, WireError> {
    let payload = postcard::to_allocvec(msg)?;
    wrap_frame(&payload)
}

/// Decode a `ClientMsg` from a 4-byte LE length-prefixed postcard frame.
pub fn decode_client_msg(frame: &[u8]) -> Result<ClientMsg, WireError> {
    let payload = unwrap_frame(frame)?;
    Ok(postcard::from_bytes(payload)?)
}

/// Encode a `ServerMsg` into a 4-byte LE length-prefixed frame.
/// `ServerMsg::Chunk` payloads are compressed with `lz4_flex`.
pub fn encode_server_msg(msg: &ServerMsg) -> Result<Vec<u8>, WireError> {
    let mut payload = Vec::new();
    match msg {
        ServerMsg::Chunk { pos, chunk } => {
            payload.push(TAG_COMPRESSED_CHUNK);
            let raw_chunk = postcard::to_allocvec(chunk.as_ref())?;
            let lz4_bytes = lz4_flex::compress_prepend_size(&raw_chunk);
            let inner = postcard::to_allocvec(&CompressedChunkPayload {
                pos: *pos,
                lz4_bytes,
            })?;
            payload.extend_from_slice(&inner);
        }
        other => {
            payload.push(TAG_PLAIN);
            let inner = postcard::to_allocvec(other)?;
            payload.extend_from_slice(&inner);
        }
    }
    wrap_frame(&payload)
}

/// Decode a `ServerMsg` from a 4-byte LE length-prefixed frame.
pub fn decode_server_msg(frame: &[u8]) -> Result<ServerMsg, WireError> {
    let payload = unwrap_frame(frame)?;
    let (&tag, rest) = payload
        .split_first()
        .ok_or(WireError::Truncated {
            expected: 5,
            actual: 4,
        })?;
    match tag {
        TAG_PLAIN => Ok(postcard::from_bytes(rest)?),
        TAG_COMPRESSED_CHUNK => {
            let compressed: CompressedChunkPayload = postcard::from_bytes(rest)?;
            if compressed.lz4_bytes.len() < 4 {
                return Err(WireError::Truncated {
                    expected: 4,
                    actual: compressed.lz4_bytes.len(),
                });
            }
            let uncompressed_len = u32::from_le_bytes([
                compressed.lz4_bytes[0],
                compressed.lz4_bytes[1],
                compressed.lz4_bytes[2],
                compressed.lz4_bytes[3],
            ]) as usize;
            if uncompressed_len > MAX_FRAME_BYTES {
                return Err(WireError::FrameTooLarge {
                    len: uncompressed_len,
                    max: MAX_FRAME_BYTES,
                });
            }
            let raw = lz4_flex::block::decompress(&compressed.lz4_bytes[4..], uncompressed_len)
                .map_err(|e| WireError::Decompress(e.to_string()))?;
            let chunk: Chunk = postcard::from_bytes(&raw)?;
            Ok(ServerMsg::Chunk {
                pos: compressed.pos,
                chunk: Arc::new(chunk),
            })
        }
        other => Err(WireError::InvalidTag(other)),
    }
}

/// Write a pre-encoded length-prefixed frame to an async stream.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &[u8],
) -> Result<(), WireError> {
    writer.write_all(frame).await?;
    Ok(())
}

/// Read a 4-byte LE length-prefixed frame from an async stream, rejecting frames
/// larger than `MAX_FRAME_BYTES` before allocating the payload buffer.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, WireError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(WireError::FrameTooLarge {
            len,
            max: MAX_FRAME_BYTES,
        });
    }
    let mut frame = vec![0u8; 4 + len];
    frame[..4].copy_from_slice(&len_buf);
    reader.read_exact(&mut frame[4..]).await?;
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::block::{AIR, DIRT, STONE};
    use terravibe_sim::net::PROTOCOL_VERSION;
    use terravibe_sim::world_bounds::WorldSize;

    #[test]
    fn chunk_roundtrip_and_oversized_frame_rejected() {
        // 1. Chunk roundtrip (compressed with lz4_flex)
        let mut chunk = Chunk::filled(AIR);
        chunk.set(1, 2, 3, STONE);
        chunk.set(4, 5, 6, DIRT);
        let pos = ChunkPos { x: 2, y: 1, z: -3 };
        let msg = ServerMsg::Chunk {
            pos,
            chunk: Arc::new(chunk),
        };
        let encoded = encode_server_msg(&msg).expect("encode chunk");
        // Compressed frame should be much smaller than raw 32x32x32 chunk
        assert!(encoded.len() < 4096, "expected compressed chunk, got {} bytes", encoded.len());

        let decoded = decode_server_msg(&encoded).expect("decode chunk");
        match decoded {
            ServerMsg::Chunk {
                pos: dpos,
                chunk: dchunk,
            } => {
                assert_eq!(dpos, pos);
                assert_eq!(dchunk.get(1, 2, 3), STONE);
                assert_eq!(dchunk.get(4, 5, 6), DIRT);
            }
            other => panic!("expected Chunk, got {other:?}"),
        }

        // 2. Plain ServerMsg & ClientMsg roundtrip
        let welcome = ServerMsg::Welcome {
            id: 7,
            spawn: [1.5, 64.0, -2.5],
            size: WorldSize::Small,
        };
        let w_enc = encode_server_msg(&welcome).unwrap();
        assert!(matches!(
            decode_server_msg(&w_enc).unwrap(),
            ServerMsg::Welcome { id: 7, .. }
        ));

        let hello = ClientMsg::Hello {
            seed: 42,
            name: Some("Alice".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: Some("pw".into()),
        };
        let h_enc = encode_client_msg(&hello).unwrap();
        assert!(matches!(
            decode_client_msg(&h_enc).unwrap(),
            ClientMsg::Hello {
                seed: 42,
                protocol_version: PROTOCOL_VERSION,
                ..
            }
        ));

        // 3. Oversized length prefix (> MAX_FRAME_BYTES) rejected immediately without OOM
        let huge_len = ((MAX_FRAME_BYTES as u32) + 1).to_le_bytes();
        let err = decode_server_msg(&huge_len).unwrap_err();
        assert!(matches!(err, WireError::FrameTooLarge { .. }));
        let err = decode_client_msg(&huge_len).unwrap_err();
        assert!(matches!(err, WireError::FrameTooLarge { .. }));

        // 4. u32::MAX length prefix rejected without OOM
        let max_len = u32::MAX.to_le_bytes();
        assert!(matches!(
            decode_server_msg(&max_len).unwrap_err(),
            WireError::FrameTooLarge { .. }
        ));

        // 5. Truncated header and truncated body rejected
        assert!(matches!(
            decode_server_msg(&[1, 2]).unwrap_err(),
            WireError::Truncated { .. }
        ));
        assert!(matches!(
            decode_server_msg(&encoded[..encoded.len() - 3]).unwrap_err(),
            WireError::Truncated { .. }
        ));
        assert!(matches!(
            decode_client_msg(&h_enc[..h_enc.len() - 1]).unwrap_err(),
            WireError::Truncated { .. }
        ));

        // 6. Corrupted tag / corrupted compressed payload rejected without panic
        let mut bad_tag = encoded.clone();
        bad_tag[4] = 99;
        assert!(matches!(
            decode_server_msg(&bad_tag).unwrap_err(),
            WireError::InvalidTag(99)
        ));

        // 7. LZ4 zip-bomb (uncompressed length > MAX_FRAME_BYTES) rejected without OOM
        let mut bomb_lz4 = Vec::new();
        bomb_lz4.extend_from_slice(&((MAX_FRAME_BYTES as u32) + 100).to_le_bytes());
        bomb_lz4.extend_from_slice(&[0u8; 8]);
        let mut bomb_payload = vec![TAG_COMPRESSED_CHUNK];
        bomb_payload.extend_from_slice(
            &postcard::to_allocvec(&CompressedChunkPayload {
                pos,
                lz4_bytes: bomb_lz4,
            })
            .unwrap(),
        );
        let bomb_frame = wrap_frame(&bomb_payload).unwrap();
        assert!(matches!(
            decode_server_msg(&bomb_frame).unwrap_err(),
            WireError::FrameTooLarge { .. }
        ));
    }

    #[tokio::test]
    async fn async_read_frame_rejects_oversized_before_allocating() {
        let huge = u32::MAX.to_le_bytes();
        let mut cursor = std::io::Cursor::new(huge.as_slice());
        let err = read_frame(&mut cursor).await.unwrap_err();
        assert!(matches!(err, WireError::FrameTooLarge { .. }));
    }
}
