//! Behavioural tests for the sample ring buffer that sits between the decoder
//! thread and the audio callback.
//!
//! The callback must never block and never allocate, so the buffer has to be
//! fixed capacity, hand back a partial fill when it is short, and preserve
//! ordering across wraparound.

use twentyfourbit_player::ring::RingBuffer;

#[test]
fn new_buffer_is_empty() {
    let rb = RingBuffer::new(16);
    assert_eq!(rb.available(), 0);
    assert_eq!(rb.free(), 16);
}

#[test]
fn push_then_pop_preserves_order() {
    let mut rb = RingBuffer::new(8);
    assert_eq!(rb.push(&[1.0, 2.0, 3.0]), 3);

    let mut out = [0.0f32; 4];
    assert_eq!(rb.pop_into(&mut out), 3);
    assert_eq!(&out[..3], &[1.0, 2.0, 3.0]);
    assert_eq!(rb.available(), 0);
}

#[test]
fn order_survives_wraparound() {
    let mut rb = RingBuffer::new(4);
    // Fill, drain half, refill so the write head wraps past the end.
    rb.push(&[1.0, 2.0, 3.0, 4.0]);
    let mut out = [0.0f32; 2];
    rb.pop_into(&mut out);
    assert_eq!(out, [1.0, 2.0]);

    rb.push(&[5.0, 6.0]);
    let mut out2 = [0.0f32; 4];
    assert_eq!(rb.pop_into(&mut out2), 4);
    assert_eq!(out2, [3.0, 4.0, 5.0, 6.0], "wraparound must not reorder");
}

#[test]
fn push_into_a_full_buffer_accepts_only_what_fits() {
    let mut rb = RingBuffer::new(3);
    assert_eq!(rb.push(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3);
    assert_eq!(rb.free(), 0);

    let mut out = [0.0f32; 4];
    assert_eq!(rb.pop_into(&mut out), 3);
    assert_eq!(&out[..3], &[1.0, 2.0, 3.0], "overflow must be dropped, not wrapped over");
}

#[test]
fn pop_into_short_buffer_returns_partial_and_keeps_the_rest() {
    let mut rb = RingBuffer::new(8);
    rb.push(&[1.0, 2.0, 3.0, 4.0]);

    let mut out = [0.0f32; 2];
    assert_eq!(rb.pop_into(&mut out), 2);
    assert_eq!(out, [1.0, 2.0]);
    assert_eq!(rb.available(), 2, "the rest stays queued");

    let mut out2 = [0.0f32; 4];
    assert_eq!(rb.pop_into(&mut out2), 2);
    assert_eq!(&out2[..2], &[3.0, 4.0]);
}

#[test]
fn clear_discards_everything() {
    let mut rb = RingBuffer::new(8);
    rb.push(&[1.0, 2.0, 3.0]);
    rb.clear();
    assert_eq!(rb.available(), 0);
    assert_eq!(rb.free(), 8);
}

#[test]
fn zero_capacity_is_safe() {
    let mut rb = RingBuffer::new(0);
    assert_eq!(rb.push(&[1.0]), 0);
    let mut out = [0.0f32; 1];
    assert_eq!(rb.pop_into(&mut out), 0);
}
