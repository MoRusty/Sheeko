/// Tracks one producer's RTP-style sequence numbers to detect drops and
/// reordering. We do not retransmit (CLAUDE.md's UDP notes) — this is purely
/// observability over the lossy stream.
#[derive(Debug, Default)]
pub struct SequenceTracker {
    last_seq: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceEvent {
    /// First packet seen, or immediately follows the previous one.
    InOrder,
    /// `count` packets were skipped between the last sequence number and this one.
    Dropped { count: u16 },
    /// This sequence number is not ahead of the last one seen.
    Reordered,
}

impl SequenceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `seq` as the latest packet observed from this producer and
    /// classifies it relative to the previous one. Uses wrapping arithmetic
    /// so this stays correct across the `u16` sequence-number wraparound.
    pub fn observe(&mut self, seq: u16) -> SequenceEvent {
        let event = match self.last_seq {
            None => SequenceEvent::InOrder,
            Some(last) => {
                let diff = seq.wrapping_sub(last) as i16;
                match diff {
                    1 => SequenceEvent::InOrder,
                    d if d > 1 => SequenceEvent::Dropped {
                        count: (d - 1) as u16,
                    },
                    _ => SequenceEvent::Reordered,
                }
            }
        };

        if !matches!(event, SequenceEvent::Reordered) {
            self.last_seq = Some(seq);
        }

        match event {
            SequenceEvent::Dropped { count } => {
                tracing::warn!(seq, count, "dropped packet(s) detected")
            }
            SequenceEvent::Reordered => tracing::warn!(seq, "out-of-order packet detected"),
            SequenceEvent::InOrder => {}
        }

        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_packet_is_in_order() {
        let mut tracker = SequenceTracker::new();
        assert_eq!(tracker.observe(0), SequenceEvent::InOrder);
    }

    #[test]
    fn detects_a_gap() {
        let mut tracker = SequenceTracker::new();
        tracker.observe(10);
        assert_eq!(tracker.observe(13), SequenceEvent::Dropped { count: 2 });
    }

    #[test]
    fn detects_reordering() {
        let mut tracker = SequenceTracker::new();
        tracker.observe(10);
        tracker.observe(11);
        assert_eq!(tracker.observe(9), SequenceEvent::Reordered);
        // the late packet doesn't corrupt tracking of the real latest sequence
        assert_eq!(tracker.observe(12), SequenceEvent::InOrder);
    }

    #[test]
    fn wraps_around_u16_boundary() {
        let mut tracker = SequenceTracker::new();
        tracker.observe(u16::MAX);
        assert_eq!(tracker.observe(0), SequenceEvent::InOrder);
    }
}
