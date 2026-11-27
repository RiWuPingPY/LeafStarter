# 源站接口（api.pingyi.me:8443/ls/）

启动器搜 CurseForge 优先走这里，国内直连最快；这里出问题时自动改走官网 Worker（`leafstarter.pingyi.me/cf`）。

```
启动器 ──直连──▶ nginx :8443 /ls/ ──▶ ls-api（127.0.0.1:8790）──▶ CurseForge
   │              ├ 单 IP 限速：每秒 10、突发 40、同时 30
   │              ├ 防护模式时直接断开
   │              └ 没有来源头的裸请求断开（防扫描）
   └── 源站连不上 / 429 / 5xx / 防护模式 ──▶ 官网 Worker（Cloudflare）──▶ CurseForge
```

| 文件 | 装到 | 做什么 |
|---|---|---|
| `ls-api.mjs` | `/opt/leafstarter/` | CurseForge 代理：只放行启动器用到的接口，裁掉用不到的字段（一次搜索约 3.6 KB），每 IP 每分钟 300 次、全部每分钟 1500 次 |
| `ls-guard.mjs` | `/opt/leafstarter/` | 防刷守护，见下 |
| `nginx/leafstarter-http.conf` | `/etc/nginx/conf.d/leafstarter.conf` | 限速区、日志格式、上游 |
| `nginx/leafstarter-location.conf` | `/etc/nginx/snippets/leafstarter.conf` | `/ls/` 这一段，include 进 api.pingyi.me 的 server 块 |
| `systemd/*.service` | `/etc/systemd/system/` | 两个服务 |
| `install.sh` | — | 一键装（可重复跑）；改 nginx 前备份到 `/root/*.bak-时间`，检查不过自动退回 |

安装：`scp -r server py:/tmp/ls-deploy && ssh py "bash /tmp/ls-deploy/install.sh"`

## 放 CurseForge Key

Key 只放在服务器上，只有 root 能读。在自己电脑的终端里运行（PowerShell 也行；命令里没有引号，不会被转坏）：

```
ssh -t py /opt/leafstarter/set-cf-key.sh
```

按提示输入 Key（输入时不显示），脚本会存好、重启 ls-api，再用这个 Key 试搜一次 jei，告诉你成没成。没放之前启动器自动走 Worker。

## 防刷（ls-guard）

管不了的：流量型 DDoS。几个 G 的流量在到这台机器之前就把带宽占满了，腾讯云会把 IP 拉进黑洞，机器上的程序无能为力。也**不做关机**：关机等于自己把自己打挂，还会带走同机的抽卡、叶脉号、MC。

管的是刷接口和连接洪水，并且优先保住同机别的项目：

| 情况 | 处理 | 代价 |
|---|---|---|
| 单个 IP 一分钟打 /ls/ 超过 1200 次（含被限速的） | 防火墙里封它 10 分钟；6 小时内再犯 1 小时、24 小时 | 被封的 IP 这段时间连不上 8443（同端口别的项目也连不上），所以门槛定得很高，学校机房共用出口也到不了 |
| /ls/ 一分钟总共超过 6000 次（分布式刷），或出口带宽连续 30 秒超过 85% 且 LeafStarter 占四成以上 | **防护模式**：只把 /ls/ 断开 15 分钟（再触发 1 小时、4 小时），到点且流量降下来自动解除 | 玩家搜 CurseForge 改走 Worker，慢一点（约 1.7 秒对 1 秒）；同端口别的项目不受影响 |
| 每个 IP 每秒新建连接超过 50 个（突发 100） | 多出来的连接直接丢掉 | 正常浏览器重试一次就连上，几乎无感 |

防火墙规则放在单独的表 `inet ls_guard` 里：默认放行，只管 8443，不碰 SSH 和别的端口；重启机器后由守护程序重建，封禁名单不保留。

阈值改 `/etc/leafstarter/guard.json`（只写要改的项，比如 `{"linkMbps": 6, "allow": ["你家的IP"]}`），然后 `systemctl restart ls-guard`。`linkMbps` 填腾讯云控制台里这台机器的出口带宽。

常用命令（在服务器上）：

```
node /opt/leafstarter/ls-guard.mjs status        # 最近一分钟的量、防护模式、封了谁
node /opt/leafstarter/ls-guard.mjs unban 1.2.3.4 # 解封
node /opt/leafstarter/ls-guard.mjs shield on|off # 手动开关防护模式
journalctl -u ls-guard -f                        # 守护的记录
```

## 拆掉

```
systemctl disable --now ls-guard leafstarter-api
nft delete table inet ls_guard
sed -i '/include snippets\/leafstarter.conf;/d' /etc/nginx/sites-available/api.pingyi.me
rm /etc/nginx/conf.d/leafstarter.conf /etc/nginx/snippets/leafstarter.conf
nginx -t && systemctl reload nginx
```

拆掉以后启动器自动只走 Worker。
