// CPUキャッシュメモリ構成
// $ lscpu | grep "L[1-3][di ]"
//   L1d: 192 KiB (4 instances)(48 KiB per core)
//   L1i: 128 KiB (4 instances)(32 KiB per core) 命令用のキャッシュのため、データアクセスの計測には影響しない
//   L2: 5 MiB (4 instances)(1.25 MiB per core)
//   L3: 8 MiB (1 instance)(shared)

use std::time::Instant;
use std::hint::black_box;
use rand::seq::SliceRandom;

fn main() {
    // 1KBから512MBまでのサイズを生成(2のべき乗)
    let sizes = (10..29).map(|exp| 2usize.pow(exp));

    println!("Size(KB)\tSequential(ns)\tRandom(ns)");

    for size in sizes {
        // 対象のサイズをCPUのワードサイズで割った要素数を持つベクタを生成
        // size = 1024(1KB) -> n = 1024 / 8(64bit) = 128要素
        let n = size / std::mem::size_of::<usize>();
        let data: Vec<usize> = (0..n).collect();
        
        // シーケンシャルアクセス
        let start = Instant::now();
        for i in 0..n {
            black_box(data[i]);
        }
        // 1アクセスあたりの平均アクセス時間をナノ秒単位で計算
        let seq_duration = start.elapsed().as_nanos() as f64 / n as f64;

        // ランダムアクセスの計測
        let mut indices: Vec<usize> = (0..n).collect();
        let mut rng = rand::rngs::ThreadRng::default();
        indices.shuffle(&mut rng);

        let start = Instant::now();
        for &idx in &indices {
            black_box(data[idx]);
        }
        let rand_duration = start.elapsed().as_nanos() as f64 / n as f64;

        println!("{}\t\t{:.2}\t\t{:.2}", size / 1024, seq_duration, rand_duration);
    }
}