TODO:
- 1KBから512MBまでのサイズを生成し、シーケンシャルアクセスとランダムアクセスの両方でアクセス時間を計測可能にする
- ローカルPCで動かす
- EC2インスタンスで動かす
- 行列演算をブロッキングに変更する

---

CPUキャッシュメモリ構成確認

```bash
$ lscpu | grep "L[1-3][di ]"
  L1d: 192 KiB (4 instances)(48 KiB per core)
  L1i: 128 KiB (4 instances)(32 KiB per core) 命令用のキャッシュのため、データアクセスの計測には影響しない
  L2: 5 MiB (4 instances)(1.25 MiB per core)
  L3: 8 MiB (1 instance)(shared)
```

---

- EC2設定
- name: matomomoplayground-server
- OS: Ubuntu
- インスタンスタイプ: c7a.medium
- ユーザーデータ:
```
#!/bin/bash
apt-get update -y
apt-get install -y git
```