//! A fixed-capacity ring buffer of interleaved samples.
//!
//! Sits between the decoder thread (producer) and the audio callback
//! (consumer). The callback must never block or allocate, so this type is
//! pre-allocated, lock-free by construction (the caller owns the lock), and
//! always gives a partial fill rather than growing.

/// A fixed-capacity circular buffer of `f32` samples.
#[derive(Debug)]
pub struct RingBuffer {
    buf: Vec<f32>,
    /// Index of the oldest sample.
    head: usize,
    /// How many samples are currently held.
    len: usize,
}

impl RingBuffer {
    /// Create a buffer holding exactly `capacity` samples.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity],
            head: 0,
            len: 0,
        }
    }

    /// Total capacity in samples.
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Samples currently queued.
    pub fn available(&self) -> usize {
        self.len
    }

    /// Samples that can still be written.
    pub fn free(&self) -> usize {
        self.buf.len() - self.len
    }

    /// Append as much of `data` as fits, returning how many samples were taken.
    ///
    /// Anything that does not fit is dropped. Callers that care should consult
    /// [`RingBuffer::free`] first and retry later.
    pub fn push(&mut self, data: &[f32]) -> usize {
        let n = data.len().min(self.free());
        if n == 0 {
            return 0;
        }
        let cap = self.buf.len();
        let tail = (self.head + self.len) % cap;
        let first = n.min(cap - tail);
        self.buf[tail..tail + first].copy_from_slice(&data[..first]);
        if first < n {
            self.buf[..n - first].copy_from_slice(&data[first..n]);
        }
        self.len += n;
        n
    }

    /// Copy up to `out.len()` samples into `out`, returning how many were
    /// written. Remaining samples stay queued; the tail of `out` is untouched.
    pub fn pop_into(&mut self, out: &mut [f32]) -> usize {
        let n = out.len().min(self.len);
        if n == 0 {
            return 0;
        }
        let cap = self.buf.len();
        let first = n.min(cap - self.head);
        out[..first].copy_from_slice(&self.buf[self.head..self.head + first]);
        if first < n {
            out[first..n].copy_from_slice(&self.buf[..n - first]);
        }
        self.head = (self.head + n) % cap;
        self.len -= n;
        n
    }

    /// Drop every queued sample.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}
