//! At most one submitted seek and one coalesced request. No unbounded UI queue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    pub seconds: f64,
    pub absolute: bool,
}
#[derive(Default)]
pub struct Queue {
    serial: u64,
    active: Option<u64>,
    pending: Option<Request>,
}
impl Queue {
    fn begin(&mut self, request: Request) -> (u64, Request) {
        self.serial = self.serial.wrapping_add(1).max(1);
        let id = (1_u64 << 63) | self.serial;
        self.active = Some(id);
        (id, request)
    }
    pub fn push(&mut self, request: Request) -> Option<(u64, Request)> {
        if !request.seconds.is_finite() {
            return None;
        }
        if self.active.is_none() {
            return Some(self.begin(request));
        }
        self.pending = Some(match (self.pending, request.absolute) {
            (Some(old), false) => Request {
                seconds: (old.seconds + request.seconds)
                    .clamp(if old.absolute { 0.0 } else { -1e8 }, 1e8),
                absolute: old.absolute,
            },
            _ => request,
        });
        None
    }
    pub fn complete(&mut self, id: u64) -> Option<(u64, Request)> {
        if self.active != Some(id) {
            return None;
        }
        self.active = None;
        self.pending.take().map(|request| self.begin(request))
    }
    pub fn clear(&mut self) {
        self.active = None;
        self.pending = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_seeks_keep_only_last_absolute_and_sum_relative_steps() {
        let mut q = Queue::default();
        let (id, _) = q
            .push(Request {
                seconds: 1.0,
                absolute: true,
            })
            .unwrap();
        for i in 2..10000 {
            assert!(
                q.push(Request {
                    seconds: i as f64,
                    absolute: true
                })
                .is_none()
            );
        }
        let (next, r) = q.complete(id).unwrap();
        assert_eq!(r.seconds, 9999.0);
        q.push(Request {
            seconds: 10.0,
            absolute: false,
        });
        q.push(Request {
            seconds: 10.0,
            absolute: false,
        });
        assert_eq!(
            q.complete(next).unwrap().1,
            Request {
                seconds: 20.0,
                absolute: false
            }
        );
    }
    #[test]
    fn old_file_replies_cannot_dispatch_a_seek_in_the_new_file() {
        let mut q = Queue::default();
        let (old, _) = q
            .push(Request {
                seconds: 1.0,
                absolute: true,
            })
            .unwrap();
        q.push(Request {
            seconds: 2.0,
            absolute: true,
        });
        q.clear();
        let (new, _) = q
            .push(Request {
                seconds: 3.0,
                absolute: true,
            })
            .unwrap();
        assert_ne!(old, new);
        assert!(q.complete(old).is_none());
        assert!(q.complete(new).is_none());
        assert!(
            q.push(Request {
                seconds: f64::NAN,
                absolute: true
            })
            .is_none()
        );
    }
}
