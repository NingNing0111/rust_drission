//! 启动 Chrome 时的配置（对齐 DrissionPage ChromiumOptions）

/// 启动新 Chrome 时的配置，对齐 DrissionPage 的 ChromiumOptions 行为
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Chrome 可执行文件路径，None 时自动查找（注册表/PATH/默认名）
    chrome_path: Option<String>,
    /// 用户数据目录，None 时使用 {tmp_path}/DrissionPage/userData/{port}
    user_data_dir: Option<String>,
    /// 是否无头模式
    headless: bool,
    /// 额外命令行参数（与 set_argument 一致，不含 --remote-debugging-port / --user-data-dir）
    args: Vec<String>,
    /// 远程调试端口（与 address 二选一，address 优先时从中解析 port）
    remote_debugging_port: u16,
    /// 调试地址 "host:port"，如 "127.0.0.1:9222"。设置后 launch 使用该 host:port
    address: Option<String>,
    /// 临时目录根，用于默认 user_data_dir。None 时用 std::env::temp_dir()
    tmp_path: Option<String>,
    /// 仅连接已有浏览器，不启动新进程
    existing_only: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            chrome_path: None,
            user_data_dir: None,
            headless: false,
            args: Self::default_args(),
            remote_debugging_port: 9222,
            address: None,
            tmp_path: None,
            existing_only: false,
        }
    }
}

/// DrissionPage 默认的 Chrome 启动参数（反检测 + 常用关闭项）
fn default_chrome_args() -> Vec<&'static str> {
    vec![
        "--no-default-browser-check",
        "--disable-suggestions-ui",
        "--no-first-run",
        "--disable-infobars",
        "--disable-popup-blocking",
        "--hide-crash-restore-bubble",
        "--disable-features=PrivacySandboxSettings4",
        "--disable-blink-features=AutomationControlled",
        "--no-sandbox"
    ]
}

impl BrowserConfig {
    fn default_args() -> Vec<String> {
        default_chrome_args().into_iter().map(String::from).collect()
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// 设置浏览器可执行文件路径（与 DrissionPage set_browser_path 一致）
    pub fn chrome_path(mut self, path: impl Into<String>) -> Self {
        self.chrome_path = Some(path.into());
        self
    }

    /// 设置用户数据目录（与 DrissionPage set_user_data_path 一致）
    pub fn user_data_dir(mut self, dir: impl Into<String>) -> Self {
        self.user_data_dir = Some(dir.into());
        self
    }

    /// 无头模式，默认 --headless=new（与 DrissionPage headless 一致）
    pub fn headless(mut self, on: bool) -> Self {
        self.headless = on;
        self
    }

    /// 设置调试端口，同时设置 address 为 127.0.0.1:port（与 DrissionPage set_local_port 一致）
    pub fn set_local_port(mut self, port: u16) -> Self {
        self.remote_debugging_port = port;
        self.address = Some(format!("127.0.0.1:{}", port));
        self
    }

    /// 设置调试地址，如 "127.0.0.1:9222" 或 "http://127.0.0.1:9222"（与 DrissionPage set_address 一致）
    pub fn set_address(mut self, address: impl Into<String>) -> Self {
        let a = address.into();
        let a = a.replace("localhost", "127.0.0.1");
        let a = a
            .strip_prefix("http://")
            .or_else(|| a.strip_prefix("https://"))
            .unwrap_or(&a)
            .trim_start_matches('/')
            .to_string();
        if let Some((_, port_str)) = a.split_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                self.remote_debugging_port = port;
            }
        }
        self.address = Some(a);
        self
    }

    /// 设置临时目录根（与 DrissionPage set_tmp_path 一致，用于默认 user_data_dir）
    pub fn tmp_path(mut self, path: impl Into<String>) -> Self {
        self.tmp_path = Some(path.into());
        self
    }

    /// 仅连接已有浏览器，不启动（与 DrissionPage existing_only 一致）
    pub fn existing_only(mut self, on: bool) -> Self {
        self.existing_only = on;
        self
    }

    /// 添加或覆盖单条启动参数（与 DrissionPage set_argument 一致）。value 为 None 表示无值参数
    pub fn set_argument(mut self, arg: impl AsRef<str>, value: Option<impl Into<String>>) -> Self {
        let arg = arg.as_ref().to_string();
        self = self.remove_argument(&arg);
        if let Some(v) = value {
            self.args.push(format!("{}={}", arg, v.into()));
        } else {
            self.args.push(arg);
        }
        self
    }

    /// 移除已存在的某条参数（按前缀匹配，如 --headless 会移除 --headless=new）
    pub fn remove_argument(mut self, arg_prefix: impl AsRef<str>) -> Self {
        let prefix = arg_prefix.as_ref();
        self.args.retain(|a| a != prefix && !a.starts_with(&format!("{}=", prefix)));
        self
    }

    /// 额外命令行参数（会与默认参数合并，重复的以最后一次 set_argument 为准）
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn remote_debugging_port(mut self, port: u16) -> Self {
        self.remote_debugging_port = port;
        if self.address.is_none() {
            self.address = Some(format!("127.0.0.1:{}", port));
        }
        self
    }

    // ---------- 便捷方法（对齐 DrissionPage ChromiumOptions）----------

    /// 禁用图片加载
    pub fn no_imgs(self, on: bool) -> Self {
        if on {
            self.set_argument("--blink-settings=imagesEnabled", Some("false"))
        } else {
            self.remove_argument("--blink-settings=imagesEnabled")
        }
    }

    /// 静音
    pub fn mute(self, on: bool) -> Self {
        if on {
            self.set_argument("--mute-audio", None::<&str>)
        } else {
            self.remove_argument("--mute-audio")
        }
    }

    /// 无痕/隐私模式
    pub fn incognito(self, on: bool) -> Self {
        if on {
            self.set_argument("--incognito", None::<&str>)
        } else {
            self.remove_argument("--incognito")
        }
    }

    /// 忽略证书错误
    pub fn ignore_certificate_errors(self, on: bool) -> Self {
        if on {
            self.set_argument("--ignore-certificate-errors", None::<&str>)
        } else {
            self.remove_argument("--ignore-certificate-errors")
        }
    }

    /// 设置 User-Agent
    pub fn set_user_agent(self, ua: impl Into<String>) -> Self {
        self.set_argument("--user-agent", Some(ua))
    }

    /// 设置代理，如 "http://127.0.0.1:7890"
    pub fn set_proxy(self, proxy: impl Into<String>) -> Self {
        self.set_argument("--proxy-server", Some(proxy))
    }

    /// 设置磁盘缓存目录（--disk-cache-dir）
    pub fn set_cache_path(self, path: impl Into<String>) -> Self {
        self.set_argument("--disk-cache-dir", Some(path))
    }

    /// 设置配置文件目录（--profile-directory），默认 "Default"
    pub fn set_profile_directory(self, profile: impl Into<String>) -> Self {
        self.set_argument("--profile-directory", Some(profile))
    }

    // ---------- 内部 getter ----------

    pub(crate) fn get_chrome_path(&self) -> Option<&str> {
        self.chrome_path.as_deref()
    }

    pub(crate) fn get_user_data_dir(&self) -> Option<&str> {
        self.user_data_dir.as_deref()
    }

    pub(crate) fn get_headless(&self) -> bool {
        self.headless
    }

    pub(crate) fn get_args(&self) -> &[String] {
        &self.args
    }

    pub(crate) fn get_remote_debugging_port(&self) -> u16 {
        self.remote_debugging_port
    }

    pub(crate) fn get_address(&self) -> Option<&str> {
        self.address.as_deref()
    }

    pub(crate) fn get_tmp_path(&self) -> Option<&str> {
        self.tmp_path.as_deref()
    }

    pub(crate) fn get_existing_only(&self) -> bool {
        self.existing_only
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let c = BrowserConfig::new();
        assert_eq!(c.get_remote_debugging_port(), 9222);
        assert!(c.get_chrome_path().is_none());
        assert!(c.get_user_data_dir().is_none());
        assert!(!c.get_headless());
        assert!(!c.get_args().is_empty()); // 含默认参数
        assert!(c.get_address().is_none());
        assert!(!c.get_existing_only());
    }

    #[test]
    fn config_builder() {
        let c = BrowserConfig::new()
            .chrome_path("/usr/bin/chrome")
            .user_data_dir("/tmp/profile")
            .headless(true)
            .remote_debugging_port(9333)
            .args(vec!["--foo".into(), "--bar".into()]);
        assert_eq!(c.get_chrome_path(), Some("/usr/bin/chrome"));
        assert_eq!(c.get_user_data_dir(), Some("/tmp/profile"));
        assert!(c.get_headless());
        assert_eq!(c.get_remote_debugging_port(), 9333);
        assert_eq!(c.get_args(), &["--foo", "--bar"]);
        assert_eq!(c.get_address(), Some("127.0.0.1:9333"));
    }

    #[test]
    fn config_set_local_port_and_address() {
        let c = BrowserConfig::new().set_local_port(9223);
        assert_eq!(c.get_remote_debugging_port(), 9223);
        assert_eq!(c.get_address(), Some("127.0.0.1:9223"));
        let c = BrowserConfig::new().set_address("127.0.0.1:9224");
        assert_eq!(c.get_remote_debugging_port(), 9224);
        assert_eq!(c.get_address(), Some("127.0.0.1:9224"));
    }

    #[test]
    fn config_default_impl() {
        let c = BrowserConfig::default();
        assert!(c.get_chrome_path().is_none());
        assert!(!c.get_headless());
    }
}
