//! 开服剧本（ROADMAP 5.12 ⑤）：固定步骤，每一步都能重复执行（已经做过的会跳过），中途断了重跑能接着做完。
//! 每一步是一段 bash（从 stdin 送进去），失败时把最后几行输出交给界面和 AI。
//! 服务器上的布局：`~/leafstarter/<id>/`，里面 java-<N>/、server/、backups/、start.sh、ctl.sh

use serde::Serialize;

/// 开服要的参数（大部分由本地实例算出来）
#[derive(Debug, Clone)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub mc: String,
    /// vanilla / fabric / forge / neoforge
    pub loader: String,
    pub loader_version: String,
    pub java: u32,
    pub mem_mb: u32,
    pub port: u16,
    pub rcon_port: u16,
    pub rcon_pass: String,
    pub online: bool,
    pub whitelist: bool,
    pub motd: String,
    /// 原版服务端：(镜像地址, 官方地址, sha1)
    pub server_jar: Option<(String, String, String)>,
    /// Fabric 启动器 / Forge、NeoForge 安装器：(镜像地址, 官方地址)
    pub extra_jar: Option<(String, String)>,
    pub keep_backups: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    /// 空的表示这一步由程序自己做（传 Mod、配置）
    #[serde(skip)]
    pub script: String,
}

pub const UPLOAD_MODS: &str = "mods";

/// 服务器上的目录（相对 $HOME，脚本里展开）
pub fn base(id: &str) -> String {
    format!("$HOME/leafstarter/{id}")
}

fn fill(t: &str, p: &Plan) -> String {
    let (jar_m, jar_o, jar_sha) = p.server_jar.clone().unwrap_or_default();
    let (ex_m, ex_o) = p.extra_jar.clone().unwrap_or_default();
    t.replace("@BASE@", &base(&p.id))
        .replace("@SVC@", &format!("leafstarter-{}", p.id))
        .replace("@NAME@", &p.name.replace(['\n', '"', '$', '`', '\\'], ""))
        .replace("@MC@", &p.mc)
        .replace("@LOADER@", &p.loader)
        .replace("@LV@", &p.loader_version)
        .replace("@JAVA@", &p.java.to_string())
        .replace("@MEM@", &p.mem_mb.to_string())
        .replace("@PORT@", &p.port.to_string())
        .replace("@RCON@", &p.rcon_port.to_string())
        .replace("@PASS@", &p.rcon_pass)
        .replace("@ONLINE@", if p.online { "true" } else { "false" })
        .replace("@WL@", if p.whitelist { "true" } else { "false" })
        .replace("@MOTD@", &props_escape(&p.motd))
        .replace("@JAR_M@", &jar_m)
        .replace("@JAR_O@", &jar_o)
        .replace("@JAR_SHA@", &jar_sha)
        .replace("@EX_M@", &ex_m)
        .replace("@EX_O@", &ex_o)
        .replace("@KEEP@", &p.keep_backups.max(1).to_string())
}

/// server.properties 里非 ASCII 字符写成 \uXXXX，新老版本都认
fn props_escape(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\n' | '\r' | '|' | '\\' | '"' | '$' | '`'))
        .map(|c| if c.is_ascii() { c.to_string() } else { c.encode_utf16(&mut [0; 2]).iter().map(|u| format!("\\\\u{u:04x}")).collect() })
        .collect()
}

const COMMON: &str = r#"BASE="@BASE@"; S="$BASE/server"; J="$BASE/java-@JAVA@"; SVC="@SVC@"
SUDO=""; [ "$(id -u)" = 0 ] || SUDO="sudo -n"
mkdir -p "$BASE" "$S"
"#;

const OS: &str = r#"[ "$(uname -s)" = Linux ] || { echo "这台不是 Linux（$(uname -s)），剧本只支持 Linux"; exit 3; }
. /etc/os-release 2>/dev/null || true
echo "系统：${PRETTY_NAME:-没认出来} $(uname -m)"
echo "CPU $(nproc) 核，内存 $(free -m 2>/dev/null | awk '/^Mem/{print $2}') MB，硬盘剩余 $(df -Pm "$HOME" | awk 'NR==2{print $4}') MB"
case "$(uname -m)" in x86_64|aarch64) ;; *) echo "不支持这种 CPU 架构"; exit 3;; esac
[ -z "$SUDO" ] || $SUDO true 2>/dev/null || echo "提示：不是 root、sudo 也要密码，装软件和开机自启这两步可能做不了"
"#;

const DEPS: &str = r#"need=""
for c in curl tar sha1sum; do command -v $c >/dev/null || need="$need $c"; done
if [ -n "$need" ]; then
  echo "要装：$need"
  if command -v apt-get >/dev/null; then $SUDO apt-get update -y && $SUDO apt-get install -y curl tar coreutils ca-certificates
  elif command -v dnf >/dev/null; then $SUDO dnf install -y curl tar coreutils
  elif command -v yum >/dev/null; then $SUDO yum install -y curl tar coreutils
  elif command -v apk >/dev/null; then $SUDO apk add curl tar coreutils bash
  else echo "不认识这台的包管理器，手动装上：$need"; exit 3; fi
fi
echo "curl、tar 都有了"
"#;

const JAVA: &str = r#"if [ -x "$J/bin/java" ] && "$J/bin/java" -version >/dev/null 2>&1; then echo "Java @JAVA@ 已经装好了"; "$J/bin/java" -version 2>&1 | head -1; exit 0; fi
[ "$(uname -m)" = x86_64 ] && A=x64 || A=aarch64
mkdir -p "$BASE/tmp"; cd "$BASE/tmp"
M="https://mirrors.tuna.tsinghua.edu.cn/Adoptium/@JAVA@/jre/$A/linux/"
F=$(curl -fsSL --max-time 20 "$M" 2>/dev/null | grep -o "OpenJDK@JAVA@U-jre_${A}_linux_hotspot_[^\"<>]*\.tar\.gz" | sort -uV | tail -1 || true)
if [ -n "$F" ]; then URL="$M$F"; else URL="https://api.adoptium.net/v3/binary/latest/@JAVA@/ga/linux/$A/jre/hotspot/normal/eclipse"; fi
echo "下载 Java @JAVA@：$URL"
curl -fsSL --retry 3 --connect-timeout 20 -o java.tar.gz "$URL"
rm -rf "$J.part"; mkdir -p "$J.part"; tar xzf java.tar.gz -C "$J.part" --strip-components=1
rm -rf "$J"; mv "$J.part" "$J"; rm -f java.tar.gz
"$J/bin/java" -version 2>&1 | head -1
"#;

const SERVER: &str = r#"cd "$S"
get() { # get 文件 镜像地址 官方地址
  curl -fsSL --retry 2 --connect-timeout 20 -o "$1.part" "$2" || curl -fsSL --retry 2 --connect-timeout 20 -o "$1.part" "$3"
  mv -f "$1.part" "$1"
}
if [ -n "@JAR_SHA@" ]; then
  if [ ! -f server.jar ] || [ "$(sha1sum server.jar | cut -d' ' -f1)" != "@JAR_SHA@" ]; then echo "下载原版服务端 @MC@"; get server.jar "@JAR_M@" "@JAR_O@"; fi
  [ "$(sha1sum server.jar | cut -d' ' -f1)" = "@JAR_SHA@" ] || { echo "服务端文件校验不对"; exit 4; }
  echo "原版服务端 @MC@ 好了"
fi
LAUNCH=""
case "@LOADER@" in
  vanilla) LAUNCH='java -Xms512M -Xmx@MEM@M -jar server.jar nogui' ;;
  fabric)
    [ -s fabric-server-launch.jar ] && [ -f .ls-fabric-@LV@ ] || { echo "下载 Fabric 服务端启动器 @LV@"; get fabric-server-launch.jar "@EX_M@" "@EX_O@"; rm -f .ls-fabric-*; touch .ls-fabric-@LV@; }
    echo "serverJar=server.jar" > fabric-server-launcher.properties
    LAUNCH='java -Xms512M -Xmx@MEM@M -jar fabric-server-launch.jar nogui' ;;
  forge|neoforge)
    if [ ! -f .ls-@LOADER@-@LV@ ]; then
      echo "下载 @LOADER@ @LV@ 安装器"; get installer.jar "@EX_M@" "@EX_O@"
      echo "装 @LOADER@ 服务端（要下载不少库，几分钟）"
      "$J/bin/java" -jar installer.jar --installServer
      rm -f installer.jar installer.jar.log; rm -f .ls-@LOADER@-*; touch .ls-@LOADER@-@LV@
    fi
    if [ -f run.sh ]; then
      echo "-Xms512M -Xmx@MEM@M" > user_jvm_args.txt
      LAUNCH='bash run.sh nogui'
    else
      F=$(ls -1 forge-*.jar 2>/dev/null | grep -v installer | head -1)
      [ -n "$F" ] || { echo "没找到装好的 Forge 服务端"; exit 4; }
      LAUNCH="java -Xms512M -Xmx@MEM@M -jar $F nogui"
    fi ;;
  *) echo "这个加载器（@LOADER@）的服务端还不支持"; exit 3 ;;
esac
cat > "$BASE/start.sh" <<EOF
#!/bin/bash
cd "$S"
export PATH="$J/bin:\$PATH"
exec $LAUNCH
EOF
chmod +x "$BASE/start.sh"
echo "启动命令：$LAUNCH"
"#;

const CONFIG: &str = r#"cd "$S"
echo "eula=true" > eula.txt
touch server.properties
setp() { if grep -q "^$1=" server.properties; then sed -i "s|^$1=.*|$1=$2|" server.properties; else echo "$1=$2" >> server.properties; fi; }
setp server-port @PORT@
setp enable-rcon true
setp rcon.port @RCON@
setp rcon.password @PASS@
setp online-mode @ONLINE@
setp white-list @WL@
setp enforce-whitelist @WL@
grep -q "^motd=" server.properties || setp motd "@MOTD@"
echo "同意了 EULA；端口 @PORT@，正版验证 @ONLINE@，白名单 @WL@，RCON 只给本机用"
"#;

const CTL: &str = r#"cat > "$BASE/ctl.sh" <<'CTL'
#!/bin/bash
BASE="$(cd "$(dirname "$0")" && pwd)"; S="$BASE/server"; SVC="@SVC@"; KEEP=@KEEP@
SUDO=""; [ "$(id -u)" = 0 ] || SUDO="sudo -n"
has_sd() { command -v systemctl >/dev/null && [ -f "/etc/systemd/system/$SVC.service" ]; }
pid() { cat "$BASE/server.pid" 2>/dev/null; }
running() { if has_sd; then systemctl is-active --quiet "$SVC"; else [ -n "$(pid)" ] && kill -0 "$(pid)" 2>/dev/null; fi; }
case "$1" in
  start) if has_sd; then $SUDO systemctl start "$SVC"; else running || { cd "$S"; nohup "$BASE/start.sh" >> "$BASE/console.log" 2>&1 < /dev/null & echo $! > "$BASE/server.pid"; }; fi ;;
  stop) if has_sd; then $SUDO systemctl stop "$SVC"; else if running; then kill "$(pid)"; for i in $(seq 90); do running || break; sleep 1; done; fi; fi ;;
  restart) "$0" stop; "$0" start ;;
  status) if running; then echo active; else echo inactive; fi ;;
  log) tail -n "${2:-200}" "$S/logs/latest.log" 2>/dev/null ;;
  world) W=$(grep '^level-name=' "$S/server.properties" 2>/dev/null | cut -d= -f2-); echo "${W:-world}" ;;
  backup)
    mkdir -p "$BASE/backups"; W=$("$0" world); [ -d "$S/$W" ] || { echo "还没有世界"; exit 0; }
    F="$BASE/backups/$W-$(date +%Y%m%d-%H%M%S).tar.gz"; tar czf "$F" --exclude=session.lock -C "$S" "$W"
    ls -1t "$BASE"/backups/*.tar.gz 2>/dev/null | tail -n +$((KEEP+1)) | xargs -r rm -f
    echo "$F" ;;
  backups) ls -1t "$BASE"/backups/*.tar.gz 2>/dev/null | while read f; do echo "$(basename "$f") $(stat -c %s "$f")"; done ;;
  *) echo "用法：ctl.sh start|stop|restart|status|log|world|backup|backups"; exit 2 ;;
esac
CTL
chmod +x "$BASE/ctl.sh"
echo "控制脚本：$BASE/ctl.sh"
"#;

const FIREWALL: &str = r#"done=""
if command -v ufw >/dev/null && $SUDO ufw status 2>/dev/null | grep -q "Status: active"; then $SUDO ufw allow @PORT@/tcp && done="ufw"; fi
if command -v firewall-cmd >/dev/null && $SUDO firewall-cmd --state >/dev/null 2>&1; then $SUDO firewall-cmd --permanent --add-port=@PORT@/tcp && $SUDO firewall-cmd --reload && done="$done firewalld"; fi
echo "系统防火墙：${done:-没开着，不用管}。云服务商控制台里的防火墙另外要放行 TCP @PORT@"
"#;

const SERVICE: &str = r#"if command -v systemctl >/dev/null && [ -d /run/systemd/system ] && { [ -z "$SUDO" ] || $SUDO true 2>/dev/null; }; then
$SUDO tee "/etc/systemd/system/$SVC.service" >/dev/null <<EOF
[Unit]
Description=Minecraft 服务器（LeafStarter：@NAME@）
After=network-online.target
Wants=network-online.target

[Service]
User=$(id -un)
WorkingDirectory=$S
ExecStart=$BASE/start.sh
Restart=on-failure
RestartSec=10
SuccessExitStatus=0 1 143
TimeoutStopSec=90

[Install]
WantedBy=multi-user.target
EOF
$SUDO systemctl daemon-reload
$SUDO systemctl enable "$SVC" >/dev/null 2>&1
echo "装成了系统服务 $SVC：开机自启，崩了 10 秒后自动重启"
else
echo "没有 systemd 或没有 root：用后台进程代替（不会开机自启）"
fi
if command -v crontab >/dev/null; then
  ( crontab -l 2>/dev/null | grep -v "$BASE/ctl.sh backup"; echo "30 4 * * * bash $BASE/ctl.sh backup >/dev/null 2>&1" ) | crontab -
  echo "每天 4:30 自动备份世界，留最近 @KEEP@ 份"
else
  echo "这台没有 crontab，自动备份没开（可以在启动器里手动备份）"
fi
"#;

const START: &str = r#"T0=$(date +%s)
if [ "$(bash "$BASE/ctl.sh" status)" = active ]; then echo "服务器已经在运行"; exit 0; fi
bash "$BASE/ctl.sh" start
echo "等服务器起来（第一次要生成世界，Forge 会更久）"
for i in $(seq 900); do
  L="$S/logs/latest.log"
  if [ -f "$L" ] && [ "$(stat -c %Y "$L")" -ge "$T0" ] && grep -q "Done (" "$L"; then echo "服务器起来了"; grep "Done (" "$L" | tail -1; exit 0; fi
  if [ "$i" -gt 20 ] && [ "$(bash "$BASE/ctl.sh" status)" != active ]; then echo "服务器自己退出了，最后的日志："; tail -n 30 "$L" 2>/dev/null; exit 4; fi
  sleep 1
done
echo "15 分钟还没起来，最后的日志："; tail -n 30 "$S/logs/latest.log" 2>/dev/null; exit 4
"#;

/// 整个剧本
pub fn steps(p: &Plan) -> Vec<Step> {
    let s = |id, title, body: &str| Step { id, title, script: fill(&format!("{COMMON}{body}"), p) };
    vec![
        s("os", "识别系统", OS),
        s("deps", "装基础工具", DEPS),
        s("java", "装 Java", JAVA),
        s("server", "装服务端", SERVER),
        Step { id: UPLOAD_MODS, title: "同步 Mod 和配置", script: String::new() },
        s("config", "同意 EULA、写服务器设置", CONFIG),
        s("ctl", "写控制脚本", CTL),
        s("firewall", "放行端口", FIREWALL),
        s("service", "开机自启和定时备份", SERVICE),
        s("start", "启动服务器", START),
    ]
}

/// 日常操作：跑 ctl.sh
pub fn ctl(id: &str, action: &str) -> String {
    format!("bash \"{}/ctl.sh\" {action}", base(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            id: "abc".into(), name: "生存服".into(), mc: "1.20.1".into(), loader: "fabric".into(), loader_version: "0.16.9".into(), java: 17, mem_mb: 3072,
            port: 25565, rcon_port: 25575, rcon_pass: "pw123".into(), online: true, whitelist: false, motd: "一起玩".into(),
            server_jar: Some(("https://m/server.jar".into(), "https://o/server.jar".into(), "deadbeef".into())), extra_jar: Some(("https://m/f.jar".into(), "https://o/f.jar".into())), keep_backups: 7,
        }
    }

    #[test]
    fn fills_everything() {
        for st in steps(&plan()) {
            let left = st.script.split('@').skip(1).step_by(2).find(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
            assert!(left.is_none(), "{} 还有没替换的：@{}@", st.id, left.unwrap_or(""));
        }
        let all: String = steps(&plan()).iter().map(|s| s.script.clone()).collect();
        assert!(all.contains("-Xmx3072M -jar fabric-server-launch.jar"));
        assert!(all.contains("leafstarter-abc"));
        assert!(all.contains(r#"setp motd "\\u4e00\\u8d77\\u73a9""#));
    }

    #[test]
    fn escapes_motd() {
        assert_eq!(props_escape("A$`b"), "Ab");
        assert_eq!(props_escape("中"), r"\\u4e2d");
    }
}
