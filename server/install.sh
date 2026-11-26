#!/bin/bash
# 在服务器上以 root 运行：把同目录的文件装好、接进 nginx（api.pingyi.me:8443/ls/）、起两个服务。可以重复跑。
# 从开发机：scp -r server py:/tmp/ls-deploy && ssh py "bash /tmp/ls-deploy/install.sh"
# 装完把 CurseForge Key 放进 /etc/leafstarter/cf.env（见 server/README.md），没放之前启动器自动走 Cloudflare Worker。
set -euo pipefail
SRC=$(cd "$(dirname "$0")" && pwd)
STAMP=$(date +%Y%m%d-%H%M%S)
SITE=/etc/nginx/sites-available/api.pingyi.me

id leafstarter >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin leafstarter
install -d -m 755 /opt/leafstarter
install -m 644 "$SRC/ls-api.mjs" "$SRC/ls-guard.mjs" /opt/leafstarter/
install -d -m 700 /etc/leafstarter
[ -f /etc/leafstarter/cf.env ] || install -m 600 /dev/null /etc/leafstarter/cf.env
install -m 644 "$SRC/systemd/leafstarter-api.service" "$SRC/systemd/ls-guard.service" /etc/systemd/system/

# nginx：先备份，改完 nginx -t 不过就原样退回
cp -a /etc/nginx/nginx.conf "/root/nginx.conf.bak-$STAMP"
cp -a "$SITE" "/root/api.pingyi.me.bak-$STAMP"
install -m 644 "$SRC/nginx/leafstarter-http.conf" /etc/nginx/conf.d/leafstarter.conf
install -m 644 "$SRC/nginx/leafstarter-location.conf" /etc/nginx/snippets/leafstarter.conf
grep -q "include snippets/leafstarter.conf;" "$SITE" || sed -i 's#^\(\s*\)include snippets/yuzu-pkg.conf;#&\n\1include snippets/leafstarter.conf;#' "$SITE"
# 每个 nginx 进程能同时开的连接：768 太少，连接洪水一来全站都卡；顺带把文件句柄上限放开
sed -i 's/^\(\s*\)worker_connections 768;/\1worker_connections 4096;/' /etc/nginx/nginx.conf
grep -q "^worker_rlimit_nofile" /etc/nginx/nginx.conf || sed -i '0,/^worker_processes/s//worker_rlimit_nofile 16384;\n&/' /etc/nginx/nginx.conf
touch /var/log/nginx/ls.access.log
chown www-data:adm /var/log/nginx/ls.access.log

systemctl daemon-reload
systemctl enable leafstarter-api ls-guard >/dev/null
systemctl restart leafstarter-api ls-guard
if nginx -t 2>/dev/null; then
    systemctl reload nginx
    echo "装好了；nginx 已重载（备份在 /root/*.bak-$STAMP）"
else
    nginx -t || true
    cp -a "/root/nginx.conf.bak-$STAMP" /etc/nginx/nginx.conf
    cp -a "/root/api.pingyi.me.bak-$STAMP" "$SITE"
    rm -f /etc/nginx/conf.d/leafstarter.conf
    echo "nginx 配置检查没过，已经退回原样" >&2
    exit 1
fi
