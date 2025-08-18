use crate::{
    add_files_to_git_repo_index, commit_index_to_git_repo, config_git_repo_user,
    lookup_entry_from_git_repo_commit_tree_by_path, read_git_repo_blob_content,
    upsert_tag_to_git_repo, upsert_branch_to_git_repo, switch_git_repo_branch, open_or_init_git_repo,
    reset_git_repo_head, clean_git_repo_index, traverse_git_repo_commit_tree_recorder, restore_git_repo_head_to_workdir
};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

// 生成随机内容的1KB文件
fn generate_random_file_content() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    let seed = hasher.finish();

    // 生成大约1KB的随机内容
    let mut content = String::new();
    for i in 0..1024 {
        content.push((((seed + i as u64) % 94) + 33) as u8 as char);
    }
    content
}

// 创建测试文件
fn create_test_file(
    repo_dir: &str,
    filename: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = Path::new(repo_dir).join(filename);
    fs::write(file_path, content)?;
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BenchmarkResult {
    pub total_runs: usize,
    pub successful_runs: usize,
    pub failed_runs: usize,
    pub durations: Vec<Duration>,
    pub avg_duration: Duration,
    pub pct50_duration: Duration,
    pub pct90_duration: Duration,
    pub pct95_duration: Duration,
}

impl BenchmarkResult {
    pub fn new(mut durations: Vec<Duration>) -> Self {
        let total_runs = durations.len();
        let successful_runs = total_runs;
        let failed_runs = 0;

        if durations.is_empty() {
            return Self {
                total_runs,
                successful_runs,
                failed_runs,
                durations,
                avg_duration: Duration::from_nanos(0),
                pct50_duration: Duration::from_nanos(0),
                pct90_duration: Duration::from_nanos(0),
                pct95_duration: Duration::from_nanos(0),
            };
        }

        // 排序以计算百分位数
        durations.sort();

        // 计算平均值
        let total_nanos: u128 = durations.iter().map(|d| d.as_nanos()).sum();
        let avg_duration = Duration::from_nanos((total_nanos / total_runs as u128) as u64);

        // 计算百分位数
        let pct50_idx = (total_runs as f64 * 0.50) as usize;
        let pct90_idx = (total_runs as f64 * 0.90) as usize;
        let pct95_idx = (total_runs as f64 * 0.95) as usize;

        let pct50_duration = durations[pct50_idx.min(total_runs - 1)];
        let pct90_duration = durations[pct90_idx.min(total_runs - 1)];
        let pct95_duration = durations[pct95_idx.min(total_runs - 1)];

        Self {
            total_runs,
            successful_runs,
            failed_runs,
            durations,
            avg_duration,
            pct50_duration,
            pct90_duration,
            pct95_duration,
        }
    }

    pub fn print_summary(&self) {
        println!("\n=== 性能测试结果 ===");
        println!("总运行次数: {}", self.total_runs);
        println!("成功次数: {}", self.successful_runs);
        println!("失败次数: {}", self.failed_runs);
        println!(
            "平均耗时: {:.2}ms",
            self.avg_duration.as_secs_f64() * 1000.0
        );
        println!(
            "PCT50 耗时: {:.2}ms",
            self.pct50_duration.as_secs_f64() * 1000.0
        );
        println!(
            "PCT90 耗时: {:.2}ms",
            self.pct90_duration.as_secs_f64() * 1000.0
        );
        println!(
            "PCT95 耗时: {:.2}ms",
            self.pct95_duration.as_secs_f64() * 1000.0
        );

        if !self.durations.is_empty() {
            let min_duration = self.durations.first().unwrap();
            let max_duration = self.durations.last().unwrap();
            println!("最小耗时: {:.2}ms", min_duration.as_secs_f64() * 1000.0);
            println!("最大耗时: {:.2}ms", max_duration.as_secs_f64() * 1000.0);
        }
    }
}

#[allow(dead_code)]
fn benchmark_open_or_init_git_repo_new_scenario(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: open_or_init_git_repo 新建场景，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_test_repo";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在（新建场景）
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数
        match open_or_init_git_repo(&test_dir) {
            Ok(_repo) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

fn benchmark_lookup_and_read_git_repo_blob() -> BenchmarkResult {
    println!(
        "开始性能测试: lookup_entry_from_git_repo_commit_tree_by_path 和 read_git_repo_blob_content，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_lookup_read_blob";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 创建 10 个嵌套文件并提交
        let repo_path = Path::new(&test_dir);
        let nested_files = match create_nested_test_files(repo_path) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("第 {} 次测试创建嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let nested_file_refs: Vec<&str> = nested_files.iter().map(|s| s.as_str()).collect();
        let index1 = match add_files_to_git_repo_index(&mut repo, nested_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加嵌套文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let commit_oid = match commit_index_to_git_repo(&mut repo, index1, "Add nested files") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 找到目录层级最深的文件（通常是 dir4/subdir8/subdir9/subdir10/subdir11/subdir12/file10.txt）
        let deepest_file_path = "dir4/subdir8/subdir9/subdir10/subdir11/subdir12/file10.txt";

        // 步骤2和3: 开始计时 - 仅测试 lookup 和 read 的耗时
        let start = Instant::now();

        // 步骤2：查找文件 entry
        let entry_option = match lookup_entry_from_git_repo_commit_tree_by_path(&repo, Some(commit_oid), deepest_file_path) {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("第 {} 次测试查找文件 entry 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let entry = match entry_option {
            Some(entry) => entry,
            None => {
                eprintln!("第 {} 次测试未找到文件 entry: {}", i + 1, deepest_file_path);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤3：读取文件内容
        match read_git_repo_blob_content(&repo, entry.oid) {
            Ok(_content) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试读取文件内容失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_open_or_init_git_repo_existing_scenario(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: open_or_init_git_repo 打开已存在仓库场景，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let test_dir = format!("bench_existing_repo_{}", std::process::id());

    // 预先创建一个 Git 仓库
    if Path::new(&test_dir).exists() {
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    // 创建测试仓库
    match open_or_init_git_repo(&test_dir) {
        Ok(_) => println!("预创建测试仓库成功: {}", test_dir),
        Err(e) => {
            eprintln!("预创建测试仓库失败: {}", e);
            return BenchmarkResult::new(vec![]);
        }
    }

    for i in 0..iterations {
        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（打开已存在的仓库）
        match open_or_init_git_repo(&test_dir) {
            Ok(_repo) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试失败: {}", i + 1, e);
            }
        }
    }

    // 清理测试目录
    if Path::new(&test_dir).exists() {
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_config_git_repo_user(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: config_git_repo_user 配置用户信息，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let test_dir = format!("bench_config_repo_{}", std::process::id());

    // 预先创建一个 Git 仓库
    if Path::new(&test_dir).exists() {
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    let mut repo = match open_or_init_git_repo(&test_dir) {
        Ok(repo) => {
            println!("预创建测试仓库成功: {}", test_dir);
            repo
        }
        Err(e) => {
            eprintln!("预创建测试仓库失败: {}", e);
            return BenchmarkResult::new(vec![]);
        }
    };

    for i in 0..iterations {
        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（配置用户信息）
        let name = format!("test_user_{}", i);
        let email = format!("test_user_{}@example.com", i);

        match config_git_repo_user(&mut repo, &name, &email) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试失败: {}", i + 1, e);
            }
        }
    }

    // 清理测试目录
    if Path::new(&test_dir).exists() {
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在空仓库中添加单个文件
#[allow(dead_code)]
fn benchmark_add_single_file_empty_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: add_files_to_git_repo_index 在空仓库中添加单个1KB文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_add_single_file";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建测试文件
        let content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "test_file.txt", &content) {
            eprintln!("第 {} 次测试创建文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（添加文件到索引）
        match add_files_to_git_repo_index(&mut repo, vec!["test_file.txt"]) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试添加文件失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在已有10个文件的仓库中添加新文件
#[allow(dead_code)]
fn benchmark_add_single_file_existing_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: add_files_to_git_repo_index 在已有10个文件的仓库中添加新文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_add_file_existing";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建10个初始文件并提交
        let mut initial_files = Vec::new();
        for j in 0..10 {
            let filename = format!("initial_file_{}.txt", j);
            let content = generate_random_file_content();
            if let Err(e) = create_test_file(&test_dir, &filename, &content) {
                eprintln!("第 {} 次测试创建初始文件失败: {}", i + 1, e);
                break;
            }
            initial_files.push(filename);
        }

        if initial_files.len() != 10 {
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 添加初始文件到索引
        let initial_file_refs: Vec<&str> = initial_files.iter().map(|s| s.as_str()).collect();
        let index = match add_files_to_git_repo_index(&mut repo, initial_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加初始文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 提交初始文件
        if let Err(e) = commit_index_to_git_repo(&mut repo, index, "Initial commit with 10 files")
        {
            eprintln!("第 {} 次测试提交初始文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建新的测试文件
        let content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "new_file.txt", &content) {
            eprintln!("第 {} 次测试创建新文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（添加新文件到索引）
        match add_files_to_git_repo_index(&mut repo, vec!["new_file.txt"]) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试添加新文件失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在已有10个文件的仓库中修改现有文件
#[allow(dead_code)]
fn benchmark_modify_single_file_existing_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: add_files_to_git_repo_index 在已有10个文件的仓库中修改现有文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_modify_file_existing";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建10个初始文件并提交
        let mut initial_files = Vec::new();
        for j in 0..10 {
            let filename = format!("initial_file_{}.txt", j);
            let content = generate_random_file_content();
            if let Err(e) = create_test_file(&test_dir, &filename, &content) {
                eprintln!("第 {} 次测试创建初始文件失败: {}", i + 1, e);
                break;
            }
            initial_files.push(filename);
        }

        if initial_files.len() != 10 {
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 添加初始文件到索引
        let initial_file_refs: Vec<&str> = initial_files.iter().map(|s| s.as_str()).collect();
        let index = match add_files_to_git_repo_index(&mut repo, initial_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加初始文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 提交初始文件
        if let Err(e) = commit_index_to_git_repo(&mut repo, index, "Initial commit with 10 files")
        {
            eprintln!("第 {} 次测试提交初始文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 修改第一个文件的内容
        let modified_content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "initial_file_0.txt", &modified_content) {
            eprintln!("第 {} 次测试修改文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（修改文件并添加到索引）
        match add_files_to_git_repo_index(&mut repo, vec!["initial_file_0.txt"]) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试修改文件失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在空仓库中提交单个文件
#[allow(dead_code)]
fn benchmark_commit_single_file_empty_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: commit_index_to_git_repo 在空仓库中提交单个文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_commit_single_file";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建测试文件并添加到索引
        let content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "test_file.txt", &content) {
            eprintln!("第 {} 次测试创建文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index = match add_files_to_git_repo_index(&mut repo, vec!["test_file.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（提交索引）
        match commit_index_to_git_repo(&mut repo, index, "Add single file to empty repo") {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试提交失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在已有10个文件的仓库中提交新文件
#[allow(dead_code)]
fn benchmark_commit_new_file_existing_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: commit_index_to_git_repo 在已有10个文件的仓库中提交新文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_commit_new_file";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建10个初始文件并提交
        let mut initial_files = Vec::new();
        for j in 0..10 {
            let filename = format!("initial_file_{}.txt", j);
            let content = generate_random_file_content();
            if let Err(e) = create_test_file(&test_dir, &filename, &content) {
                eprintln!("第 {} 次测试创建初始文件失败: {}", i + 1, e);
                break;
            }
            initial_files.push(filename);
        }

        if initial_files.len() != 10 {
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 添加初始文件到索引并提交
        let initial_file_refs: Vec<&str> = initial_files.iter().map(|s| s.as_str()).collect();
        let initial_index = match add_files_to_git_repo_index(&mut repo, initial_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加初始文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        if let Err(e) =
            commit_index_to_git_repo(&mut repo, initial_index, "Initial commit with 10 files")
        {
            eprintln!("第 {} 次测试提交初始文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建新的测试文件并添加到索引
        let content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "new_file.txt", &content) {
            eprintln!("第 {} 次测试创建新文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index = match add_files_to_git_repo_index(&mut repo, vec!["new_file.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加新文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（提交新文件）
        match commit_index_to_git_repo(&mut repo, index, "Add new file to existing repo") {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试提交新文件失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在已有10个文件的仓库中提交修改的文件
#[allow(dead_code)]
fn benchmark_commit_modified_file_existing_repo(iterations: usize) -> BenchmarkResult {
    println!(
        "开始性能测试: commit_index_to_git_repo 在已有10个文件的仓库中提交修改的文件，测试 {} 次",
        iterations
    );

    let mut durations = Vec::with_capacity(iterations);
    let base_dir = "bench_commit_modified_file";

    for i in 0..iterations {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建10个初始文件并提交
        let mut initial_files = Vec::new();
        for j in 0..10 {
            let filename = format!("initial_file_{}.txt", j);
            let content = generate_random_file_content();
            if let Err(e) = create_test_file(&test_dir, &filename, &content) {
                eprintln!("第 {} 次测试创建初始文件失败: {}", i + 1, e);
                break;
            }
            initial_files.push(filename);
        }

        if initial_files.len() != 10 {
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 添加初始文件到索引并提交
        let initial_file_refs: Vec<&str> = initial_files.iter().map(|s| s.as_str()).collect();
        let initial_index = match add_files_to_git_repo_index(&mut repo, initial_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加初始文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        if let Err(e) =
            commit_index_to_git_repo(&mut repo, initial_index, "Initial commit with 10 files")
        {
            eprintln!("第 {} 次测试提交初始文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 修改第一个文件的内容并添加到索引
        let modified_content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "initial_file_0.txt", &modified_content) {
            eprintln!("第 {} 次测试修改文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index = match add_files_to_git_repo_index(&mut repo, vec!["initial_file_0.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加修改文件到索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 开始计时
        let start = Instant::now();

        // 执行被测试的函数（提交修改的文件）
        match commit_index_to_git_repo(&mut repo, index, "Modify existing file in repo") {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试提交修改文件失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

// 创建具有多层目录结构的测试文件
fn create_nested_test_files(
    repo_path: &std::path::Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file_paths = vec![
        "file1.txt".to_string(),
        "dir1/file2.txt".to_string(),
        "dir1/file3.txt".to_string(),
        "dir1/subdir1/file4.txt".to_string(),
        "dir2/file5.txt".to_string(),
        "dir2/subdir2/file6.txt".to_string(),
        "dir2/subdir2/subdir3/file7.txt".to_string(),
        "dir3/subdir4/subdir5/subdir6/file8.txt".to_string(),
        "dir3/subdir4/subdir5/subdir6/subdir7/file9.txt".to_string(),
        "dir4/subdir8/subdir9/subdir10/subdir11/subdir12/file10.txt".to_string(),
    ];

    for file_path in &file_paths {
        let full_path = repo_path.join(file_path);
        // 创建父目录
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        create_test_file(
            repo_path.to_str().unwrap(),
            file_path,
            &generate_random_file_content(),
        )?;
    }

    Ok(file_paths)
}

// 测试在空仓库中一次性提交10个具有多层目录结构的文件
#[allow(dead_code)]
fn benchmark_add_commit_multiple_files_empty_repo() -> BenchmarkResult {
    let mut durations = Vec::new();

    for _ in 0..1000 {
        let start = Instant::now();

        // 创建临时目录
        let temp_dir = std::env::temp_dir().join(format!("bench_test_{}", std::process::id()));
        let repo_path = &temp_dir;

        // 创建并配置仓库
        let mut repo = open_or_init_git_repo(repo_path.to_str().unwrap()).unwrap();
        config_git_repo_user(&mut repo, "Test User", "test@example.com").unwrap();

        // 创建10个具有多层目录结构的文件
        let file_paths = create_nested_test_files(repo_path).unwrap();

        // 开始计时：添加所有文件到索引并提交
        let index =
            add_files_to_git_repo_index(&mut repo, file_paths.iter().map(|s| s.as_str()).collect())
                .unwrap();
        commit_index_to_git_repo(
            &mut repo,
            index,
            "Add and commit 10 files with nested directory structure",
        )
        .unwrap();

        let duration = start.elapsed();
        durations.push(duration);

        // 清理
        drop(repo);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    BenchmarkResult::new(durations)
}

// 性能测试：在空仓库中创建提交并打标签
#[allow(dead_code)]
fn benchmark_create_tag_empty_repo() -> BenchmarkResult {
    let mut durations = Vec::new();
    
    for _ in 0..1000 {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join(format!("bench_tag_test_{}", std::process::id()));
        let repo_path = &temp_dir;
        
        // 创建并配置仓库
        let mut repo = open_or_init_git_repo(repo_path.to_str().unwrap()).unwrap();
        config_git_repo_user(&mut repo, "Test User", "test@example.com").unwrap();
        
        // 创建一个测试文件
        create_test_file(repo_path.to_str().unwrap(), "test_file.txt", &generate_random_file_content()).unwrap();
        
        // 添加文件到索引并提交
        let index = add_files_to_git_repo_index(&mut repo, vec!["test_file.txt"]).unwrap();
        commit_index_to_git_repo(&mut repo, index, "Initial commit for tag test").unwrap();
        
        // 开始计时：创建标签
        let start = Instant::now();
        upsert_tag_to_git_repo(&mut repo, "test_tag", "Test tag message", None).unwrap();
        let duration = start.elapsed();
        durations.push(duration);
        
        // 清理
        drop(repo);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_upsert_branch_empty_repo() -> BenchmarkResult {
    println!(
        "开始性能测试: upsert_branch_to_git_repo 创建分支，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_upsert_branch";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 创建测试文件并提交
        let content = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "test_file.txt", &content) {
            eprintln!("第 {} 次测试创建文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 添加文件到 index
        let index = match add_files_to_git_repo_index(&mut repo, vec!["test_file.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 提交文件
        let _commit_id = match commit_index_to_git_repo(&mut repo, index, "Initial commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 开始计时 - 只测试 upsert_branch_to_git_repo 函数的耗时
        let start = Instant::now();

        // 执行被测试的函数（创建分支）
        match upsert_branch_to_git_repo(&mut repo, "test_branch", None) {
            Ok(_branch_ref) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试创建分支失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_switch_git_repo_branch() -> BenchmarkResult {
    println!(
        "开始性能测试: switch_git_repo_branch 切换分支，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_switch_branch";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 在空仓库上添加一个文件并提交
        let content1 = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "file1.txt", &content1) {
            eprintln!("第 {} 次测试创建文件1失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index1 = match add_files_to_git_repo_index(&mut repo, vec!["file1.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件1到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let _commit_id1 = match commit_index_to_git_repo(&mut repo, index1, "First commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交1失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 创建分支 test_branch_1
        if let Err(e) = upsert_branch_to_git_repo(&mut repo, "test_branch_1", None) {
            eprintln!("第 {} 次测试创建分支失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤2: 继续创建一个文件并提交
        let content2 = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "file2.txt", &content2) {
            eprintln!("第 {} 次测试创建文件2失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index2 = match add_files_to_git_repo_index(&mut repo, vec!["file2.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件2到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let _commit_id2 = match commit_index_to_git_repo(&mut repo, index2, "Second commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交2失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤3: 开始计时 - 只测试 switch_git_repo_branch 函数的耗时
        let start = Instant::now();

        // 执行被测试的函数（切换到 test_branch_1，need_restore_to_workdir 为 true）
        match switch_git_repo_branch(&mut repo, "test_branch_1", true) {
            Ok(_branch_ref) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试切换分支失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_reset_git_repo_head() -> BenchmarkResult {
    println!(
        "开始性能测试: reset_git_repo_head 重置到指定提交，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_reset_head";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 在空仓库中添加文件并提交作为 commit1
        let content1 = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "initial_file.txt", &content1) {
            eprintln!("第 {} 次测试创建初始文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index1 = match add_files_to_git_repo_index(&mut repo, vec!["initial_file.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加初始文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let commit1_oid = match commit_index_to_git_repo(&mut repo, index1, "Initial commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交初始文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤2: 创建 10 个嵌套文件并提交
        let repo_path = Path::new(&test_dir);
        let nested_files = match create_nested_test_files(repo_path) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("第 {} 次测试创建嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let nested_file_refs: Vec<&str> = nested_files.iter().map(|s| s.as_str()).collect();
        let index2 = match add_files_to_git_repo_index(&mut repo, nested_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加嵌套文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        if let Err(e) = commit_index_to_git_repo(&mut repo, index2, "Add nested files") {
            eprintln!("第 {} 次测试提交嵌套文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤3: 开始计时 - 只测试 reset_git_repo_head 函数的耗时
        let start = Instant::now();

        // 执行被测试的函数（重置到 commit1）
        match reset_git_repo_head(&mut repo, commit1_oid) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试重置失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_clean_git_repo_index() -> BenchmarkResult {
    println!(
        "开始性能测试: clean_git_repo_index 清理索引并提交，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_clean_index";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 创建 10 个嵌套文件并提交
        let repo_path = Path::new(&test_dir);
        let nested_files = match create_nested_test_files(repo_path) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("第 {} 次测试创建嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let nested_file_refs: Vec<&str> = nested_files.iter().map(|s| s.as_str()).collect();
        let index1 = match add_files_to_git_repo_index(&mut repo, nested_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加嵌套文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        if let Err(e) = commit_index_to_git_repo(&mut repo, index1, "Add nested files") {
            eprintln!("第 {} 次测试提交嵌套文件失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤2 & 3 & 4 : 开始计时 - 测试 clean_git_repo_index 和 commit_index_to_git_repo 的耗时
        let start = Instant::now();

        // 步骤2: 清理索引
        let clean_index = match clean_git_repo_index(&mut repo) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试清理索引失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤3: 提交清理后的索引
        match commit_index_to_git_repo(&mut repo, clean_index, "清空所有文件") {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试提交清理索引失败: {}", i + 1, e);
            }
        }

        // 步骤4: 恢复工作目录到 HEAD
        if let Err(e) = restore_git_repo_head_to_workdir(&mut repo) {
            eprintln!("第 {} 次测试恢复工作目录失败: {}", i + 1, e);
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}

#[allow(dead_code)]
fn benchmark_traverse_git_repo_commit_tree_recorder() -> BenchmarkResult {
    println!(
        "开始性能测试: traverse_git_repo_commit_tree_recorder 遍历提交树，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_traverse_commit_tree";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 创建 10 个嵌套文件并提交
        let repo_path = Path::new(&test_dir);
        let nested_files = match create_nested_test_files(repo_path) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("第 {} 次测试创建嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let nested_file_refs: Vec<&str> = nested_files.iter().map(|s| s.as_str()).collect();
        let index1 = match add_files_to_git_repo_index(&mut repo, nested_file_refs) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加嵌套文件到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let commit_oid = match commit_index_to_git_repo(&mut repo, index1, "Add nested files") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交嵌套文件失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤2: 开始计时 - 仅测试 traverse_git_repo_commit_tree_recorder 的耗时
        let start = Instant::now();

        // 执行被测试的函数（遍历上一次提交）
        match traverse_git_repo_commit_tree_recorder(&repo, Some(commit_oid)) {
            Ok(_) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试遍历提交树失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}


#[allow(dead_code)]
fn benchmark_switch_git_repo_branch_no_restore() -> BenchmarkResult {
    println!(
        "开始性能测试: switch_git_repo_branch 切换分支，测试 1000 次"
    );

    let mut durations = Vec::with_capacity(1000);
    let base_dir = "bench_switch_branch";

    for i in 0..1000 {
        let test_dir = format!("{}_{}_{}", base_dir, i, std::process::id());

        // 确保目录不存在
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // 创建新的 Git 仓库
        let mut repo = match open_or_init_git_repo(&test_dir) {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("第 {} 次测试创建仓库失败: {}", i + 1, e);
                continue;
            }
        };

        // 配置用户信息
        if let Err(e) = config_git_repo_user(&mut repo, "Test User", "test@example.com") {
            eprintln!("第 {} 次测试配置用户失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤1: 在空仓库上添加一个文件并提交
        let content1 = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "file1.txt", &content1) {
            eprintln!("第 {} 次测试创建文件1失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index1 = match add_files_to_git_repo_index(&mut repo, vec!["file1.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件1到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let _commit_id1 = match commit_index_to_git_repo(&mut repo, index1, "First commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交1失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 创建分支 test_branch_1
        if let Err(e) = upsert_branch_to_git_repo(&mut repo, "test_branch_1", None) {
            eprintln!("第 {} 次测试创建分支失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        // 步骤2: 继续创建一个文件并提交
        let content2 = generate_random_file_content();
        if let Err(e) = create_test_file(&test_dir, "file2.txt", &content2) {
            eprintln!("第 {} 次测试创建文件2失败: {}", i + 1, e);
            let _ = std::fs::remove_dir_all(&test_dir);
            continue;
        }

        let index2 = match add_files_to_git_repo_index(&mut repo, vec!["file2.txt"]) {
            Ok(index) => index,
            Err(e) => {
                eprintln!("第 {} 次测试添加文件2到 index 失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        let _commit_id2 = match commit_index_to_git_repo(&mut repo, index2, "Second commit") {
            Ok(commit_id) => commit_id,
            Err(e) => {
                eprintln!("第 {} 次测试提交2失败: {}", i + 1, e);
                let _ = std::fs::remove_dir_all(&test_dir);
                continue;
            }
        };

        // 步骤3: 开始计时 - 只测试 switch_git_repo_branch 函数的耗时
        let start = Instant::now();

        // 执行被测试的函数（切换到 test_branch_1，need_restore_to_workdir 为 true）
        match switch_git_repo_branch(&mut repo, "test_branch_1", false) {
            Ok(_branch_ref) => {
                let duration = start.elapsed();
                durations.push(duration);

                if (i + 1) % 100 == 0 {
                    println!("已完成 {} 次测试", i + 1);
                }
            }
            Err(e) => {
                eprintln!("第 {} 次测试切换分支失败: {}", i + 1, e);
            }
        }

        // 清理测试目录
        if Path::new(&test_dir).exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
    }

    BenchmarkResult::new(durations)
}


#[allow(dead_code)]
fn run_benchmark() {
    println!("=== Git 仓库操作性能基准测试 ===");

    // 测试新建仓库场景
    let new_result = benchmark_open_or_init_git_repo_new_scenario(1000);
    // 测试打开已存在仓库场景
    let existing_result = benchmark_open_or_init_git_repo_existing_scenario(1000);
    // 测试配置用户信息场景
    let config_result = benchmark_config_git_repo_user(1000);
    // 测试添加文件到空仓库场景
    let add_empty_result = benchmark_add_single_file_empty_repo(1000);
    // 测试添加文件到已有文件仓库场景
    let add_existing_result = benchmark_add_single_file_existing_repo(1000);
    // 测试修改已有文件场景
    let modify_existing_result = benchmark_modify_single_file_existing_repo(1000);
    // 测试提交文件到空仓库场景
    let commit_empty_result = benchmark_commit_single_file_empty_repo(1000);
    // 测试提交新文件到已有文件仓库场景
    let commit_new_result = benchmark_commit_new_file_existing_repo(1000);
    // 测试提交修改文件到已有文件仓库场景
    let commit_modified_result = benchmark_commit_modified_file_existing_repo(1000);
    // 测试在空仓库中一次性提交10个具有多层目录结构的文件场景
    let add_commit_multiple_result = benchmark_add_commit_multiple_files_empty_repo();
    // 测试在空仓库中创建提交并打标签场景
    let create_tag_result = benchmark_create_tag_empty_repo();
    // 测试在空仓库中创建分支场景
    let upsert_branch_result = benchmark_upsert_branch_empty_repo();
    // 测试切换分支场景
    let switch_branch_result = benchmark_switch_git_repo_branch();
    // 测试切换分支场景 (不 restore)
    let switch_branch_result_no_restore = benchmark_switch_git_repo_branch_no_restore();
    // 测试重置仓库 HEAD 场景
    let reset_head_result = benchmark_reset_git_repo_head();
    // 测试清理索引场景
    let clean_index_result = benchmark_clean_git_repo_index();
    // 测试遍历提交树场景
    let traverse_commit_tree_result = benchmark_traverse_git_repo_commit_tree_recorder();
    // 测试查找文件 entry 和读取 blob 内容场景
    let lookup_read_blob_result = benchmark_lookup_and_read_git_repo_blob();

    // 打印结果
    println!("\n1. 新建仓库场景测试");
    new_result.print_summary();
    println!("\n2. 打开已存在仓库场景测试");
    existing_result.print_summary();
    println!("\n3. 配置用户信息场景测试");
    config_result.print_summary();
    println!("\n4. 空仓库添加单个文件场景测试");
    add_empty_result.print_summary();
    println!("\n5. 已有文件仓库添加新文件场景测试");
    add_existing_result.print_summary();
    println!("\n6. 已有文件仓库修改现有文件场景测试");
    modify_existing_result.print_summary();
    println!("\n7. 空仓库提交单个文件场景测试");
    commit_empty_result.print_summary();
    println!("\n8. 已有文件仓库提交新文件场景测试");
    commit_new_result.print_summary();
    println!("\n9. 已有文件仓库提交修改文件场景测试");
    commit_modified_result.print_summary();
    println!("\n10. 在空仓库中一次性提交10个具有多层目录结构的文件场景测试");
    add_commit_multiple_result.print_summary();    
    println!("\n11. 在空仓库中创建提交并打标签场景测试");
    create_tag_result.print_summary();
    println!("\n12. 创建分支场景测试");
    upsert_branch_result.print_summary();
    println!("\n13. 切换分支场景测试");
    switch_branch_result.print_summary();
    println!("\n14. 切换分支场景测试, 不 restore workdir");
    switch_branch_result_no_restore.print_summary();
    println!("\n15. 重置仓库 HEAD 场景测试");
    reset_head_result.print_summary();
    println!("\n16. 清理索引场景测试");
    clean_index_result.print_summary();
    println!("\n17. 遍历提交树场景测试");
    traverse_commit_tree_result.print_summary();
    println!("\n18. 查找文件 entry 和读取 blob 内容场景测试");
    lookup_read_blob_result.print_summary();
}


#[cfg(test)]
mod tests {
    use super::*;

    // cargo test test_run_benchmark -- --nocapture
    #[test]
    fn test_run_benchmark() {
        // 通过单测驱动 run_benchmark 函数
        run_benchmark();
    }
}
