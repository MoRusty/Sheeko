use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Header size in bytes: 2 (sequence) + 4 (timestamp) + 4 (ssrc).
pub const HEADER_LEN: usize = 10;

/// A minimal RTP-like header. The SFU only ever reads this — the Opus
/// payload after it is forwarded raw, never decoded (see CLAUDE.md's SFU
/// model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("packet too short: {0} bytes, need at least {HEADER_LEN}")]
    TooShort(usize),
}

pub fn encode(header: Header, payload: &[u8]) -> Bytes {
    let mut buf = BytesMut::with_capacity(HEADER_LEN + payload.len());
    buf.put_u16(header.sequence);
    buf.put_u32(header.timestamp);
    buf.put_u32(header.ssrc);
    buf.put_slice(payload);
    buf.freeze()
}

/// Splits a raw packet into its header and payload. The payload `Bytes` is a
/// zero-copy view into the same underlying buffer, not a fresh allocation.
pub fn decode(mut packet: Bytes) -> Result<(Header, Bytes), DecodeError> {
    if packet.len() < HEADER_LEN {
        return Err(DecodeError::TooShort(packet.len()));
    }
    let sequence = packet.get_u16();
    let timestamp = packet.get_u32();
    let ssrc = packet.get_u32();
    Ok((
        Header {
            sequence,
            timestamp,
            ssrc,
        },
        packet,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let header = Header {
            sequence: 42,
            timestamp: 123_456,
            ssrc: 0xdead_beef,
        };
        let payload = b"opus bytes go here";

        let packet = encode(header, payload);
        let (decoded_header, decoded_payload) = decode(packet).unwrap();

        assert_eq!(decoded_header, header);
        assert_eq!(&decoded_payload[..], payload);
    }

    #[test]
    fn rejects_short_packets() {
        let too_short = Bytes::from_static(&[0u8; HEADER_LEN - 1]);
        assert_eq!(decode(too_short), Err(DecodeError::TooShort(HEADER_LEN - 1)));
    }
}
