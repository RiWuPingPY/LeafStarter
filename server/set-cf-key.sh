#!/bin/bash
# 把 CurseForge Key 放进 /etc/leafstarter/cf.env（只有 root 能读），重启源站代理，再自查一次。
# 用法（在自己电脑的终端里，PowerShell 也行，不带引号就不会被转坏）：ssh -t py /opt/leafstarter/set-cf-key.sh
set -e
read -rsp 'CurseForge Key（输入时不显示，输完按回车）: ' k
echo
if [ -z "$k" ]; then
    echo '没输入，什么都没改'
    exit 1
fi
umask 077
printf 'CF_API_KEY=%s\n' "$k" > /etc/leafstarter/cf.env
chmod 600 /etc/leafstarter/cf.env
unset k
systemctl restart leafstarter-api
sleep 1
H='Origin: https://leafstarter.pingyi.me'
R='api.pingyi.me:8443:127.0.0.1'
echo "源站状态：$(curl -s -H "$H" --resolve "$R" https://api.pingyi.me:8443/ls/health)"
code=$(curl -s -o /tmp/ls-key-check.json -w '%{http_code}' -H "$H" --resolve "$R" 'https://api.pingyi.me:8443/ls/cf/v1/mods/search?classId=6&searchFilter=jei&pageSize=1')
if [ "$code" = 200 ]; then
    echo "试搜 jei：成功（$(grep -o '"name":"[^"]*"' /tmp/ls-key-check.json | head -1)）"
else
    echo "试搜 jei：失败，状态码 $code。Key 可能不对，重新运行这个脚本再输一次"
fi
rm -f /tmp/ls-key-check.json
