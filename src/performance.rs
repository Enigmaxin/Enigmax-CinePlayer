//! Decoder resource policy. Keep quality intact; unsupported acceleration falls back in mpv.
/// Bound frame parallelism: FFmpeg otherwise grows frame buffers with host core count.
pub fn decoder_threads(logical_cpus: usize) -> usize {
    logical_cpus.clamp(1, 8)
}
/// MVC uses slice workers; avoid four workers on one/two-core machines.
pub fn mvc_threads(logical_cpus: usize) -> usize {
    logical_cpus.clamp(1, 4)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workers_fit_small_and_large_hosts_without_unbounded_buffers() {
        for n in [1, 2, 4, 8, 16, 128] {
            assert!(decoder_threads(n) <= n && decoder_threads(n) <= 8);
            assert!(mvc_threads(n) <= n && mvc_threads(n) <= 4);
        }
        assert_eq!(decoder_threads(0), 1);
        assert_eq!(mvc_threads(0), 1);
    }
}
