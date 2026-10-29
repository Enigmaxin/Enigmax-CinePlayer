//! Persistent request throttling; manual requests still respect server cooldowns.
#[derive(Default, Debug)]
pub struct Cache {
    pub automatic_after: u64,
    pub manual_after: u64,
    pub blocked_until: u64,
    pub status: String,
}
impl Cache {
    pub fn decode(bytes: &[u8]) -> Self {
        if bytes.len() > 8192 {
            return Self::default();
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return Self::default();
        };
        if v["format"] != 1 {
            return Self::default();
        }
        Self {
            automatic_after: v["automatic_after"].as_u64().unwrap_or(0),
            manual_after: v["manual_after"].as_u64().unwrap_or(0),
            blocked_until: v["blocked_until"].as_u64().unwrap_or(0),
            status: v["status"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(1024)
                .collect(),
        }
    }
    pub fn wait(&self, now: u64, manual: bool) -> u64 {
        self.blocked_until
            .max(if manual {
                self.manual_after
            } else {
                self.automatic_after
            })
            .saturating_sub(now)
    }
    pub fn record(&mut self, now: u64, status: &str, retry: u64) {
        self.status = status.into();
        self.blocked_until = retry;
        self.manual_after = now.saturating_add(60).max(retry);
        self.automatic_after = now
            .saturating_add(if matches!(status, "ok" | "no-release") {
                12 * 3600
            } else {
                900
            })
            .max(retry);
    }
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"format":1,"automatic_after":self.automatic_after,"manual_after":self.manual_after,"blocked_until":self.blocked_until,"status":self.status})).unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_cache_and_manual_cooldowns_survive_restarts() {
        let mut c = Cache::default();
        c.record(1000, "ok", 0);
        let c = Cache::decode(&c.encode());
        assert_eq!(c.wait(1001, false), 43199);
        assert_eq!(c.wait(1001, true), 59);
        assert_eq!(c.wait(1060, true), 0);
    }
    #[test]
    fn server_blocks_manual_and_automatic_even_after_restarts() {
        let mut c = Cache::default();
        c.record(1000, "limited", 90000);
        let c = Cache::decode(&c.encode());
        assert_eq!(c.wait(80000, true), 10000);
        assert_eq!(c.wait(80000, false), 10000);
        assert_eq!(c.wait(90001, true), 0);
        assert_eq!(Cache::decode(b"corrupt").wait(0, true), 0);
    }
}
