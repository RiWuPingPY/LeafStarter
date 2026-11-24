//! 游戏日志格式。官方的 log4j 配置让游戏把控制台日志输出成 XML（官方启动器靠它解析），
//! 这里把 XML 事件转回平常的一行行日志：`[12:34:56] [Render thread/INFO]: 内容`。
//! 不是 XML 的行（Java 自己的警告、Mod 直接 println 的）原样保留。

#[derive(Default)]
pub struct LogFormatter {
    ev: Option<Event>,
    tz_offset_min: i64,
}

#[derive(Default)]
struct Event {
    ts: u64,
    level: String,
    thread: String,
    msg: String,
    thr: String,
    /// 正在读哪段 CDATA：0 没有，1 消息，2 异常
    reading: u8,
}

impl LogFormatter {
    pub fn new() -> Self {
        LogFormatter { ev: None, tz_offset_min: local_offset_minutes() }
    }

    /// 喂一行原始输出，吐出零到多行整理好的日志
    pub fn feed(&mut self, raw: &str) -> Vec<String> {
        let t = raw.trim();
        if let Some(ev) = self.ev.as_mut() {
            match ev.reading {
                1 | 2 => {
                    let target = if ev.reading == 1 { &mut ev.msg } else { &mut ev.thr };
                    if let Some(pos) = raw.find("]]>") {
                        push_line(target, &raw[..pos]);
                        ev.reading = 0;
                    } else {
                        push_line(target, raw);
                    }
                    return vec![];
                }
                _ => {}
            }
            if let Some(rest) = t.strip_prefix("<log4j:Message><![CDATA[") {
                read_cdata(rest, &mut ev.msg, &mut ev.reading, 1);
            } else if let Some(rest) = t.strip_prefix("<log4j:Throwable><![CDATA[") {
                read_cdata(rest, &mut ev.thr, &mut ev.reading, 2);
            } else if t.starts_with("</log4j:Event>") {
                let ev = self.ev.take().unwrap();
                return self.render(ev);
            }
            return vec![];
        }
        if t.starts_with("<log4j:Event") {
            self.ev = Some(Event {
                ts: attr(t, "timestamp").and_then(|v| v.parse().ok()).unwrap_or(0),
                level: attr(t, "level").unwrap_or_default(),
                thread: attr(t, "thread").unwrap_or_default(),
                ..Default::default()
            });
            return vec![];
        }
        vec![raw.to_string()]
    }

    fn render(&self, ev: Event) -> Vec<String> {
        let secs = (ev.ts / 1000) as i64 + self.tz_offset_min * 60;
        let day = secs.rem_euclid(86_400);
        let head = format!("[{:02}:{:02}:{:02}] [{}/{}]: ", day / 3600, day % 3600 / 60, day % 60, ev.thread, ev.level);
        let mut out = vec![];
        for (k, line) in ev.msg.split('\n').enumerate() {
            out.push(if k == 0 { format!("{}{}", head, line) } else { line.to_string() });
        }
        out.extend(ev.thr.split('\n').filter(|l| !l.trim().is_empty()).map(String::from));
        out
    }
}

fn read_cdata(rest: &str, target: &mut String, reading: &mut u8, which: u8) {
    match rest.find("]]>") {
        Some(pos) => target.push_str(&rest[..pos]),
        None => {
            target.push_str(rest);
            *reading = which;
        }
    }
}

fn push_line(target: &mut String, s: &str) {
    target.push('\n');
    target.push_str(s);
}

/// 取 XML 属性值，顺带解开实体
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{}=\"", name);
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

/// 本地时区和 UTC 差几分钟
fn local_offset_minutes() -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Tzi {
            bias: i32,
            standard_name: [u16; 32],
            standard_date: [u16; 8],
            standard_bias: i32,
            daylight_name: [u16; 32],
            daylight_date: [u16; 8],
            daylight_bias: i32,
        }
        extern "system" {
            fn GetTimeZoneInformation(tzi: *mut Tzi) -> u32;
        }
        let mut z = Tzi { bias: 0, standard_name: [0; 32], standard_date: [0; 8], standard_bias: 0, daylight_name: [0; 32], daylight_date: [0; 8], daylight_bias: 0 };
        // SAFETY: 结构体按 TIME_ZONE_INFORMATION 的布局声明
        let r = unsafe { GetTimeZoneInformation(&mut z) };
        if r != u32::MAX {
            let extra = if r == 2 { z.daylight_bias } else { z.standard_bias };
            return -((z.bias + extra) as i64);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_xml_events_into_lines() {
        let mut f = LogFormatter { ev: None, tz_offset_min: 480 };
        let mut out = vec![];
        for l in [
            r#"<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1700000000000" level="INFO" thread="Render thread">"#,
            "  <log4j:Message><![CDATA[Backend library: LWJGL version 3.3.3]]></log4j:Message>",
            "</log4j:Event>",
            "WARNING: A terminally deprecated method in sun.misc.Unsafe has been called",
            r#"<log4j:Event logger="x" timestamp="1700000001000" level="ERROR" thread="IO-Worker-1">"#,
            "  <log4j:Message><![CDATA[Couldn't connect to realms]]></log4j:Message>",
            "  <log4j:Throwable><![CDATA[java.lang.RuntimeException: boom",
            "\tat a.b.C.d(C.java:1)",
            "]]></log4j:Throwable>",
            "</log4j:Event>",
        ] {
            out.extend(f.feed(l));
        }
        assert_eq!(out[0], "[06:13:20] [Render thread/INFO]: Backend library: LWJGL version 3.3.3");
        assert_eq!(out[1], "WARNING: A terminally deprecated method in sun.misc.Unsafe has been called");
        assert_eq!(out[2], "[06:13:21] [IO-Worker-1/ERROR]: Couldn't connect to realms");
        assert_eq!(out[3], "java.lang.RuntimeException: boom");
        assert_eq!(out[4], "\tat a.b.C.d(C.java:1)");
        assert_eq!(out.len(), 5);
    }
}
