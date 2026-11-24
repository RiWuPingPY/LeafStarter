//! 二分查冲突（ROADMAP 5.3、M3）：一个整合包启动就崩、规则又认不出原因时，自动开一部分 Mod 试，找出是哪个。
//!
//! 只管「启动到主菜单之前就崩」的情况。步骤：
//! 1. 全开再试一次：不崩就是偶发，不查了
//! 2. 全关试一次：也崩就不是 Mod 的问题
//! 3. 按文件名排好，二分找最短的会崩的前缀 → 最后加进去的那个 x 有份
//! 4. 只开 x：崩了就是它自己的问题
//! 5. 不崩就是和别人冲突：x 一直开着，在它前面再二分一次找到搭档 y
//!
//! 每次「开」都会带上必需的前置（见 `modmeta::closure`），不然缺前置的崩溃会混进来。

use std::future::Future;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Outcome {
    /// 全开也没崩
    NoCrash,
    /// 一个 Mod 都不开也崩
    NotMods,
    /// 找到了：x 自己会崩，或者 x 和 partner 一起会崩
    Found { culprit: usize, partner: Option<usize> },
}

/// 估计要启动几次（界面上显示「大约 N 次」）
pub fn estimate(n: usize) -> usize {
    let log = |k: usize| if k <= 1 { 0 } else { (usize::BITS - (k - 1).leading_zeros()) as usize };
    2 + log(n) + 1 + log(n.saturating_sub(1))
}

/// `crashes(开哪些)` 返回这样开会不会崩（true = 崩了）
pub async fn search<F, Fut>(n: usize, mut crashes: F) -> Result<Outcome>
where
    F: FnMut(Vec<usize>) -> Fut,
    Fut: Future<Output = Result<bool>>,
{
    if !crashes((0..n).collect()).await? {
        return Ok(Outcome::NoCrash);
    }
    if crashes(vec![]).await? {
        return Ok(Outcome::NotMods);
    }
    // 前缀 lo 个不崩、前缀 hi 个崩
    let (mut lo, mut hi) = (0, n);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if crashes((0..mid).collect()).await? {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let x = hi - 1;
    if x == 0 || crashes(vec![x]).await? {
        return Ok(Outcome::Found { culprit: x, partner: None });
    }
    // 搭档在 0..x 里：前缀 lo 个 + x 不崩，前缀 hi 个 + x 崩
    let (mut lo, mut hi) = (0, x);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        let mut set: Vec<usize> = (0..mid).collect();
        set.push(x);
        if crashes(set).await? {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(Outcome::Found { culprit: x, partner: Some(hi - 1) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn run(n: usize, bad: impl Fn(&[usize]) -> bool) -> (Outcome, usize) {
        let count = Cell::new(0);
        let out = tauri::async_runtime::block_on(search(n, |set| {
            count.set(count.get() + 1);
            let r = bad(&set);
            async move { Ok(r) }
        }))
        .unwrap();
        (out, count.get())
    }

    #[test]
    fn finds_single_culprit() {
        for n in [1, 2, 7, 50] {
            for c in 0..n {
                let (o, tries) = run(n, |s| s.contains(&c));
                assert_eq!(o, Outcome::Found { culprit: c, partner: None }, "n={n} c={c}");
                assert!(tries <= estimate(n), "n={n} tries={tries}");
            }
        }
    }

    #[test]
    fn finds_conflicting_pair() {
        let n = 50;
        for (a, b) in [(3, 41), (0, 49), (20, 21), (48, 2)] {
            let (o, tries) = run(n, |s| s.contains(&a) && s.contains(&b));
            let (x, y) = (a.max(b), a.min(b));
            assert_eq!(o, Outcome::Found { culprit: x, partner: Some(y) }, "{a} {b}");
            assert!(tries <= estimate(n));
        }
        assert_eq!(estimate(50), 15);
    }

    #[test]
    fn gives_up_sensibly() {
        assert_eq!(run(10, |_| false).0, Outcome::NoCrash);
        assert_eq!(run(10, |_| true).0, Outcome::NotMods);
    }
}
