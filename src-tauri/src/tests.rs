use crate::delete::*;
use crate::rules::*;
use crate::tree::*;

fn d(name: &str, files: Vec<RawFile>, dirs: Vec<RawDir>) -> RawDir {
    RawDir { name: name.into(), mtime: 0, attrs: 0x10, flags: 0, files, dirs }
}
fn f(name: &str, size: u64) -> RawFile {
    RawFile { name: name.into(), size, mtime: 0, attrs: 0x20, flags: 0 }
}

/// 按真实环境变量拼一棵模拟的 C 盘
fn fake_tree(rules: &Rules) -> Tree {
    let profile = std::env::var("USERPROFILE").unwrap();
    let user = profile.rsplit('\\').next().unwrap().to_string();
    let raw = d("C:\\", vec![f("pagefile.sys", 1000), f("setup.log", 5)], vec![
        d("Windows", vec![], vec![
            d("System32", vec![f("kernel32.dll", 50)], vec![]),
            d("Temp", vec![f("a.tmp", 7)], vec![d("sub", vec![f("b.bin", 3)], vec![])]),
            d("Fonts", vec![f("x.ttf", 2)], vec![]),
        ]),
        d("Users", vec![], vec![d(&user, vec![f("NTUSER.DAT", 9)], vec![
            d("AppData", vec![], vec![d("Local", vec![], vec![
                d("Temp", vec![f("t1", 10)], vec![]),
                d("SomeApp", vec![f("config.ini", 1)], vec![]),
            ])]),
            d("Documents", vec![], vec![d("proj", vec![f("main.js", 1)], vec![
                d("node_modules", vec![f("x.js", 100)], vec![]),
            ])]),
            d("Downloads", vec![f("big.iso", 500)], vec![]),
        ])]),
        d("Program Files", vec![], vec![d("App", vec![], vec![d("node_modules", vec![f("y.js", 4)], vec![])])]),
        d("Games", vec![f("save.dat", 8)], vec![]),
    ]);
    let mut t = Tree::new(ScanMode::Full, "C:\\");
    t.push_raw_dir(NO_PARENT, raw);
    t.finalize(rules);
    t
}

fn find(t: &Tree, path: &str) -> u32 {
    (0..t.nodes.len() as u32)
        .find(|&i| t.path(i).display().to_string().eq_ignore_ascii_case(path))
        .unwrap_or_else(|| panic!("找不到 {path}"))
}

fn rule_of(t: &Tree, r: &Rules, path: &str) -> (String, u8) {
    let n = &t.nodes[find(t, path) as usize];
    (r.entry(n.rule).id.clone(), n.level)
}

#[test]
fn classification() {
    let r = Rules::load();
    let t = fake_tree(&r);
    let up = std::env::var("USERPROFILE").unwrap();
    assert_eq!(rule_of(&t, &r, "C:\\"), ("drive-root".into(), 3));
    assert_eq!(rule_of(&t, &r, "C:\\pagefile.sys"), ("pagefile".into(), 3));
    assert_eq!(rule_of(&t, &r, "C:\\setup.log"), ("ext-temp".into(), 1));
    assert_eq!(rule_of(&t, &r, "C:\\Windows"), ("windows".into(), 3));
    assert_eq!(rule_of(&t, &r, "C:\\Windows\\Fonts\\x.ttf").1, 3, "Windows 下未识别的内容继承保护");
    assert_eq!(rule_of(&t, &r, "C:\\Windows\\System32\\kernel32.dll").1, 3);
    assert_eq!(rule_of(&t, &r, "C:\\Windows\\Temp"), ("win-temp".into(), 0), "更具体的规则覆盖上级保护");
    assert_eq!(rule_of(&t, &r, "C:\\Windows\\Temp\\sub\\b.bin").1, 0);
    assert_eq!(rule_of(&t, &r, "C:\\Users"), ("profile-root".into(), 3));
    assert_eq!(rule_of(&t, &r, &up).0, "profile-root");
    assert_eq!(rule_of(&t, &r, &format!("{up}\\NTUSER.DAT")).0, "ntuser");
    assert_eq!(rule_of(&t, &r, &format!("{up}\\AppData\\Local\\Temp")), ("user-temp".into(), 0));
    assert_eq!(rule_of(&t, &r, &format!("{up}\\AppData\\Local\\SomeApp")), ("appdata".into(), 2));
    assert_eq!(rule_of(&t, &r, &format!("{up}\\Documents\\proj\\node_modules")), ("node-modules".into(), 1));
    assert_eq!(rule_of(&t, &r, &format!("{up}\\Downloads\\big.iso")), ("downloads".into(), 1));
    assert_eq!(rule_of(&t, &r, "C:\\Program Files\\App\\node_modules").1, 2, "已安装软件里的 node_modules 不能当作可清理");
    assert_eq!(rule_of(&t, &r, "C:\\Games"), ("default".into(), 2), "未识别默认谨慎");

    // 风险向上汇总：AppData\Local 本身是 L2，里面有 L0；C 盘整体含 L3
    let root = &t.nodes[find(&t, "C:\\") as usize];
    assert_eq!(root.max_level, 3);
    assert_eq!(root.size, 1000 + 5 + 50 + 7 + 3 + 2 + 9 + 10 + 1 + 1 + 100 + 500 + 4 + 8);
    // 可清理命中：Windows\Temp、AppData\Local\Temp、node_modules、Downloads 不算（L1 但 quick=false 也算命中）
    let hit_paths: Vec<String> = t.hits.iter().map(|&h| t.path(h).display().to_string()).collect();
    assert!(hit_paths.iter().any(|p| p.ends_with("Windows\\Temp")));
    assert!(hit_paths.iter().any(|p| p.ends_with("node_modules")));
}

/// 非系统盘：任意盘符和任意位置的规则
#[test]
fn other_drive_rules() {
    let r = Rules::load();
    let raw = d("D:\\", vec![], vec![
        d("$RECYCLE.BIN", vec![], vec![d("S-1-5-21-x", vec![f("$R1.zip", 30)], vec![])]),
        d("WindowsApps", vec![f("app.exe", 5)], vec![]),
        d("WpSystem", vec![], vec![]),
        d("WUDownloadCache", vec![f("u.cab", 9)], vec![]),
        d(".pnpm-store", vec![f("p", 4)], vec![]),
        d("SteamLibrary", vec![], vec![d("steamapps", vec![], vec![
            d("common", vec![f("game.exe", 100)], vec![]),
            d("downloading", vec![f("part", 20)], vec![]),
            d("shadercache", vec![f("s", 6)], vec![]),
        ])]),
        d("workspace", vec![], vec![d("proj", vec![], vec![d("node_modules", vec![f("x.js", 3)], vec![])])]),
        d("Backup", vec![], vec![d("WeChat Files", vec![], vec![d("wxid_1", vec![], vec![
            d("Msg", vec![f("MSG0.db", 50)], vec![]),
            d("FileStorage", vec![], vec![d("Cache", vec![f("c", 7)], vec![]), d("Image", vec![f("i", 8)], vec![])]),
        ])])]),
        d("Program Files", vec![], vec![d("App", vec![], vec![d("node_modules", vec![f("y.js", 2)], vec![])])]),
    ]);
    let mut t = Tree::new(ScanMode::Full, "D:\\");
    t.push_raw_dir(NO_PARENT, raw);
    t.finalize(&r);
    assert_eq!(rule_of(&t, &r, "D:\\$RECYCLE.BIN"), ("recycle-bin".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\$RECYCLE.BIN\\S-1-5-21-x").1, 3);
    let rb = find(&t, "D:\\$RECYCLE.BIN");
    assert_eq!(t.delete_mode(&r, rb), (Mode::EmptyRecycleBin, 1), "回收站按“需确认”清空，不因内部结构显示为受保护");
    let plan = build_plan(&t, &r, &[rb]);
    assert_eq!((plan.items.len(), plan.max_level), (1, 1));
    assert_eq!(rule_of(&t, &r, "D:\\WindowsApps"), ("windowsapps".into(), 3));
    assert_eq!(rule_of(&t, &r, "D:\\WpSystem"), ("windowsapps".into(), 3));
    assert_eq!(rule_of(&t, &r, "D:\\WUDownloadCache"), ("wu-relocated".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\.pnpm-store"), ("pnpm-store".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\SteamLibrary\\steamapps\\downloading"), ("steam-cache".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\SteamLibrary\\steamapps\\shadercache"), ("steam-cache".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\SteamLibrary\\steamapps\\common").0, "default", "游戏本体不是缓存");
    assert_eq!(rule_of(&t, &r, "D:\\workspace\\proj\\node_modules"), ("node-modules".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\Backup\\WeChat Files"), ("wechat-data".into(), 2));
    assert_eq!(rule_of(&t, &r, "D:\\Backup\\WeChat Files\\wxid_1\\Msg\\MSG0.db"), ("wechat-data".into(), 2), "聊天记录数据库保持谨慎");
    assert_eq!(rule_of(&t, &r, "D:\\Backup\\WeChat Files\\wxid_1\\FileStorage\\Cache"), ("wechat-cache".into(), 0));
    assert_eq!(rule_of(&t, &r, "D:\\Backup\\WeChat Files\\wxid_1\\FileStorage\\Image"), ("wechat-media".into(), 1));
    assert_eq!(rule_of(&t, &r, "D:\\Program Files"), ("program-files".into(), 2));
    assert_eq!(rule_of(&t, &r, "D:\\Program Files\\App\\node_modules").1, 2, "软件目录里不套用全局规则");

    // 未识别的 Backup 里有微信聊天记录：整体删除要输入名称
    let p = build_plan(&t, &r, &[find(&t, "D:\\Backup")]);
    assert!(p.typing_required);
    // 未识别的 workspace（里面只有 node_modules）：不用输入
    let p = build_plan(&t, &r, &[find(&t, "D:\\workspace")]);
    assert_eq!(p.max_level, 2);
    assert!(!p.typing_required);
}

#[test]
fn plan_blocks_and_confirms() {
    let r = Rules::load();
    let t = fake_tree(&r);
    let up = std::env::var("USERPROFILE").unwrap();
    let ids = |ps: &[&str]| ps.iter().map(|p| find(&t, p)).collect::<Vec<_>>();

    // 受保护项被拦截
    let p = build_plan(&t, &r, &ids(&["C:\\Windows", "C:\\pagefile.sys"]));
    assert!(p.items.is_empty());
    assert_eq!(p.blocked.len(), 2);

    // 含受保护子项的文件夹被拦截
    let p = build_plan(&t, &r, &ids(&["C:\\Users"]));
    assert!(p.blocked[0].blocked.is_some());

    // L0 只删内容、允许永久删除、无需输入
    let temp = format!("{up}\\AppData\\Local\\Temp");
    let p = build_plan(&t, &r, &ids(&[&temp]));
    assert_eq!(p.max_level, 0);
    assert_eq!(p.items[0].mode, Mode::Contents);
    assert!(p.permanent_allowed);
    let ok = DeleteRequest { ids: vec![], permanent: true, ack: false, typed: String::new() };
    assert!(check_request(&p, &ok).is_ok());

    // 父子同时选中只保留父级
    let p = build_plan(&t, &r, &ids(&[&temp, &format!("{temp}\\t1")]));
    assert_eq!(p.items.len(), 1);

    // L2 需要勾选 + 输入名称，且不能永久删除
    let app = format!("{up}\\AppData\\Local\\SomeApp");
    let p = build_plan(&t, &r, &ids(&[&app]));
    assert_eq!(p.max_level, 2);
    assert_eq!(p.expected_text, "SomeApp");
    assert!(!p.permanent_allowed);
    let mk = |permanent, ack, typed: &str| DeleteRequest { ids: vec![], permanent, ack, typed: typed.into() };
    assert!(check_request(&p, &mk(false, false, "SomeApp")).is_err());
    assert!(check_request(&p, &mk(false, true, "someapp")).is_err());
    assert!(check_request(&p, &mk(true, true, "SomeApp")).is_err());
    assert!(check_request(&p, &mk(false, true, "SomeApp")).is_ok());

    assert!(p.typing_required, "规则明确识别的软件数据要输入名称");

    // 混选：按最高等级，输入“确认删除”
    let p = build_plan(&t, &r, &ids(&[&app, &temp]));
    assert_eq!(p.max_level, 2);
    assert_eq!(p.expected_text, multi_confirm_text());

    // 只是未识别的文件夹：仍是谨慎、只进回收站、要勾选已了解，但不用输入名称
    let p = build_plan(&t, &r, &ids(&["C:\\Games"]));
    assert_eq!(p.max_level, 2);
    assert!(!p.typing_required);
    assert!(!p.permanent_allowed);
    assert!(check_request(&p, &mk(false, true, "")).is_ok());
    assert!(check_request(&p, &mk(false, false, "")).is_err());
    assert!(check_request(&p, &mk(true, true, "")).is_err());

    // 未识别 + 明确识别的谨慎项一起删：要输入
    let p = build_plan(&t, &r, &ids(&["C:\\Games", &app]));
    assert!(p.typing_required);
}

#[test]
fn remove_updates_ancestors() {
    let r = Rules::load();
    let mut t = fake_tree(&r);
    let root = find(&t, "C:\\");
    let before = t.nodes[root as usize].size;
    let iso = find(&t, &format!("{}\\Downloads\\big.iso", std::env::var("USERPROFILE").unwrap()));
    t.remove(iso);
    assert_eq!(t.nodes[root as usize].size, before - 500);
}

/// 在真实的临时目录里扫描并永久删除（只动测试自己创建的文件）
#[test]
fn real_scan_and_delete() {
    let base = std::env::temp_dir().join(format!("dc-test-{}", std::process::id()));
    std::fs::create_dir_all(base.join("a\\b")).unwrap();
    std::fs::write(base.join("a\\b\\x.bin"), vec![0u8; 4096]).unwrap();
    std::fs::write(base.join("y.txt"), b"hello").unwrap();

    let r = Rules::load();
    let progress = crate::scanner::Progress::default();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let ctx = crate::scanner::Ctx { progress: &progress, cancel: &cancel };
    let mut t = crate::scanner::scan_full(&base, &r, &ctx);
    let id = find(&t, &base.display().to_string());
    assert_eq!(t.nodes[id as usize].size, 4096 + 5);
    // 位于 %TEMP% 下，继承“用户临时文件”= 安全
    assert_eq!(t.nodes[id as usize].max_level, 0);

    let plan = build_plan(&t, &r, &[id]);
    assert_eq!(plan.items.len(), 1, "{:?}", plan.blocked.iter().map(|b| &b.blocked).collect::<Vec<_>>());
    let req = DeleteRequest { ids: vec![id], permanent: true, ack: false, typed: String::new() };
    check_request(&plan, &req).unwrap();
    let jobs = jobs_from_plan(&t, &plan);
    let errs = run_jobs(&jobs, true, None);
    let out = apply_results(&mut t, &jobs, errs);
    assert!(out.failed.is_empty());
    assert_eq!(out.freed, 4096 + 5);
    assert!(!base.exists());
}

/// 手动运行：cargo test --release real_machine -- --ignored --nocapture
#[test]
#[ignore]
fn real_machine() {
    let r = Rules::load();
    for (mode, drive) in [("quick", "C:\\"), ("quick", "D:\\"), ("full", "C:\\")] {
        let progress = crate::scanner::Progress::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let ctx = crate::scanner::Ctx { progress: &progress, cancel: &cancel };
        let t0 = std::time::Instant::now();
        let t = if mode == "quick" { crate::scanner::scan_quick(&r, drive, &ctx) } else { crate::scanner::scan_full(std::path::Path::new("C:\\"), &r, &ctx) };
        let root = &t.nodes[t.root as usize];
        println!("== {mode} {drive}: {:.1}s, {} 个节点, 总计 {:.2} GB, 无权限 {}", t0.elapsed().as_secs_f64(), t.nodes.len(), root.size as f64 / 1e9, progress.errors.load(std::sync::atomic::Ordering::Relaxed));
        let mut hits = t.hits.clone();
        hits.sort_by(|a, b| t.nodes[*b as usize].size.cmp(&t.nodes[*a as usize].size));
        for h in hits.iter().take(12) {
            let n = &t.nodes[*h as usize];
            println!("  L{} {:>9.1} MB  {:<16} {}", n.level, n.size as f64 / 1e6, r.entry(n.rule).name.get(), t.path(*h).display());
        }
        if mode == "full" {
            let r0 = t.root;
            for &c in t.nodes[r0 as usize].children.iter().take(8) {
                let n = &t.nodes[c as usize];
                println!("  [{}..{}] {:>9.2} GB  {}", n.level, n.max_level, n.size as f64 / 1e9, n.name);
            }
        }
    }
}
