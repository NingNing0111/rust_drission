use std::time::{Duration, SystemTime};

// 在 [min_ms, max_ms] 区间内随机等待
pub fn sleep_random_ms(min_ms: u64, max_ms: u64) {
    let (lo, hi) = if min_ms <= max_ms {
        (min_ms, max_ms)
    } else {
        (max_ms, min_ms)
    };
    if lo == hi {
        std::thread::sleep(Duration::from_millis(lo));
        return;
    }
    let now_nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let span = hi - lo + 1;
    let wait_ms = lo + (now_nanos % span);
    std::thread::sleep(Duration::from_millis(wait_ms));
}
