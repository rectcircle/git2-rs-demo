use git2;
use std::{fs, path::Path};

mod bench;

fn open_or_init_git_repo(dir: &str) -> Result<git2::Repository, Box<dyn std::error::Error>> {
    let git_dir = Path::new(dir).join(".git");
    if git_dir.exists() {
        println!("Git 仓库: {} 已存在，将打开它", dir);
        let result = git2::Repository::open(dir)?;
        return Ok(result);
    }
    if Path::new(dir).exists() {
        println!("目录: {} 已存在，但是 .git ，将删除它", dir);
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    let result =
        git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))?;
    println!("初始化并打开了 Git 仓库: {}", dir);
    return Ok(result);
}

fn config_git_repo_kv_str(
    config: &mut git2::Config,
    name: &str,
    value: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut need_update = false;

    // 先尝试获取现有值，如果不存在则认为需要更新
    match config.get_string(name) {
        Ok(old_value) => {
            if old_value != value {
                need_update = true;
            }
        }
        Err(_) => {
            // 配置项不存在，需要设置
            need_update = true;
        }
    }

    if need_update {
        println!("配置了 {} = {}", name, value);
        config.set_str(name, value)?;
    } else {
        println!("无需配置 {} = {} ，跳过", name, value);
    }
    return Ok(need_update);
}

fn config_git_repo_user(
    repo: &mut git2::Repository,
    name: &str,
    email: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = repo.config()?;
    if config_git_repo_kv_str(&mut config, "user.name", name)?
        || config_git_repo_kv_str(&mut config, "user.email", email)?
    {
        println!("用户信息被更新了，写入 .git/config");
    } else {
        println!("用户信息无需更新");
    }
    Ok(())
}

fn add_files_to_git_repo_index(
    repo: &mut git2::Repository,
    file_relative_paths: Vec<&str>,
) -> Result<git2::Index, Box<dyn std::error::Error>> {
    let mut index = repo.index()?;
    let workdir = repo.workdir().ok_or("仓库没有工作目录")?;

    for file_relative_path in file_relative_paths {
        let file_path = workdir.join(file_relative_path);

        if file_path.exists() {
            println!("添加文件到 index: {}", file_relative_path);
            index.add_path(std::path::Path::new(file_relative_path))?;
        } else {
            println!("文件不存在，从 index 中移除: {}", file_relative_path);
            // 尝试从索引中移除文件，如果文件不在索引中则忽略错误
            if let Err(_) = index.remove_path(std::path::Path::new(file_relative_path)) {
                println!("文件 {} 不在索引中，跳过移除操作", file_relative_path);
            }
        }
    }
    index.write()?;
    Ok(index)
}

fn commit_index_to_git_repo(
    repo: &mut git2::Repository,
    mut index: git2::Index,
    message: &str,
) -> Result<git2::Oid, Box<dyn std::error::Error>> {
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let signature = repo.signature()?;

    // 获取 HEAD 引用，如果是第一次提交则为 None
    let parent_commit = match repo.head() {
        Ok(head) => {
            let oid = head.target().unwrap();
            Some(repo.find_commit(oid)?)
        }
        Err(_) => None,
    };

    let parents: Vec<&git2::Commit> = match &parent_commit {
        Some(commit) => vec![commit],
        None => vec![],
    };

    let commit_id = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;

    Ok(commit_id)
}

fn upsert_tag_to_git_repo<'a>(
    repo: &'a mut git2::Repository,
    tag_name: &str,
    message: &str,
    target_oid: Option<git2::Oid>,
) -> Result<git2::Reference<'a>, Box<dyn std::error::Error>> {
    let signature = repo.signature()?;

    // 确定标签指向的目标对象，如果没有指定则使用 HEAD
    let target_commit = match target_oid {
        Some(oid) => repo.find_commit(oid)?,
        None => {
            let head = repo.head()?;
            let oid = head.target().unwrap();
            repo.find_commit(oid)?
        }
    };

    let target = target_commit.as_object();

    // 检查标签是否已存在
    let tag_ref_name = format!("refs/tags/{}", tag_name);

    // 如果标签已存在，先删除它
    if let Ok(_) = repo.find_reference(&tag_ref_name) {
        println!("标签 {} 已存在，将更新它", tag_name);
    } else {
        println!("标签 {} 不存在，将创建它", tag_name);
    }

    // 创建新的标签
    let tag_oid = repo.tag(tag_name, &target, &signature, message, true)?;

    println!("upsert 标签: {} -> {}", tag_name, tag_oid);

    // 返回标签引用
    let tag_ref = repo.find_reference(&tag_ref_name)?;

    Ok(tag_ref)
}


fn upsert_branch_to_git_repo<'a>(
    repo: &'a mut git2::Repository,
    branch_name: &str,
    target_oid: Option<git2::Oid>,
) -> Result<git2::Reference<'a>, Box<dyn std::error::Error>> {
    // 确定分支指向的目标 commit，如果没有指定则使用 HEAD
    let target_commit = match target_oid {
        Some(oid) => repo.find_commit(oid)?,
        None => {
            let head = repo.head()?;
            let oid = head.target().unwrap();
            repo.find_commit(oid)?
        }
    };

    // 检查分支是否已存在
    let branch_ref_name = format!("refs/heads/{}", branch_name);

    // 如果分支已存在，先删除它
    if let Ok(_) = repo.find_branch(branch_name, git2::BranchType::Local) {
        println!("分支 {} 已存在，将更新它", branch_name);
    } else {
        println!("分支 {} 不存在，将创建它", branch_name);
    }

    // 创建新的分支
    repo.branch(branch_name, &target_commit, true)?;

    println!("upsert 分支: {} -> {}", branch_name, target_commit.id());

    // 返回分支引用
    let branch_ref = repo.find_reference(&branch_ref_name)?;

    Ok(branch_ref)
}

fn switch_git_repo_branch<'a>(
    repo: &'a mut git2::Repository,
    branch_name: &str,
    update_workdir: bool,
) -> Result<git2::Reference<'a>, Box<dyn std::error::Error>> {
    // 查找分支引用
    let branch_ref_name = format!("refs/heads/{}", branch_name);
    // 检查分支是否存在
    _ = repo.find_reference(&branch_ref_name)?;

    // 设置 HEAD 指向目标分支
    repo.set_head(&branch_ref_name)?;

    if update_workdir {
        // 如果需要更新工作目录，则进行 checkout 操作
        let head = repo.head()?;
        let oid = head.target().unwrap();
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        // 执行 checkout 操作，更新工作目录文件
        repo.checkout_tree(
            tree.as_object(),
            Some(
                git2::build::CheckoutBuilder::new()
                    .force() // 强制覆盖工作目录中的文件
                    .remove_untracked(true), // 移除未跟踪的文件
            ),
        )?;

        println!("已切换到分支 {} 并更新工作目录", branch_name);
    } else {
        println!("已切换到分支 {} (仅更新 HEAD)", branch_name);
    }

    // 返回分支引用
    let updated_branch_ref = repo.find_reference(&branch_ref_name)?;
    Ok(updated_branch_ref)
}

fn reset_git_repo_head(
    repo: &mut git2::Repository,
    target_commit_oid: git2::Oid,
) -> Result<(), Box<dyn std::error::Error>> {
    // 查找目标 commit
    let target_commit = repo.find_commit(target_commit_oid)?;
    
    // 获取目标 commit 的 tree
    let target_tree = target_commit.tree()?;

    // 获取当前分支引用
    let head_ref = repo.head()?;

    
    // 1. 重置 HEAD 到目标 commit
    match head_ref.kind() {
        Some(git2::ReferenceType::Symbolic) => {
            let branch_name = head_ref.name().unwrap();
            repo
            .reference(&branch_name, target_commit_oid, true, format!("reset HEAD to {}", target_commit_oid).as_str())?;
        },
        Some(git2::ReferenceType::Direct) | None => {
            repo
            .set_head_detached(target_commit_oid)?;
        },
    }

    // 2. 重置索引到目标 tree
    let mut index = repo.index()?;
    index.read_tree(&target_tree)?;
    index.write()?;
    
    // 3. 重置工作目录到目标 tree (hard reset)
    repo.checkout_tree(
        target_tree.as_object(),
        Some(
            git2::build::CheckoutBuilder::new()
                .force() // 强制覆盖工作目录中的文件
                .remove_untracked(true) // 移除未跟踪的文件
                .remove_ignored(false) // 移除被忽略的文件
        ),
    )?;
    
    println!("已重置 HEAD、索引和工作目录到 commit: {}", target_commit_oid);
    
    Ok(())
}

fn clean_git_repo_index(
    repo: &mut git2::Repository,
) -> Result<git2::Index, Box<dyn std::error::Error>> {
    // 获取仓库的索引
    let mut index = repo.index()?;
    
    // 清空索引中的所有条目
    index.clear()?;
    
    // 写入索引更改
    index.write()?;
    
    println!("已清空索引中的所有文件");
    
    Ok(index)
}

#[derive(Debug)]
#[allow(dead_code)]
struct TreeEntry {
    relative_path: String,
    oid: git2::Oid,
    kind: git2::ObjectType,
}

fn traverse_git_repo_commit_tree_recorder(
    repo: &git2::Repository,
    commit_oid: Option<git2::Oid>,
) -> Result<Vec<TreeEntry>, Box<dyn std::error::Error>> {
    let mut recorder = Vec::new();

    // 确定要遍历的 commit，如果没有指定则使用 HEAD
    let target_commit = match commit_oid {
        Some(oid) => repo.find_commit(oid)?,
        None => {
            let head = repo.head()?;
            let oid = head.target().unwrap();
            repo.find_commit(oid)?
        }
    };

    // 获取 commit 对应的 tree
    let tree = target_commit.tree()?;

    // 遍历 tree 中的所有条目
    tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
        let entry_kind = match entry.kind() {
            Some(git2::ObjectType::Tree) | Some(git2::ObjectType::Blob) => entry.kind().unwrap(),
            _ => return git2::TreeWalkResult::Ok,
        };

        recorder.push(TreeEntry {
            relative_path: entry.name().unwrap().to_string(),
            kind: entry_kind,
            oid: entry.id(),
        });

        git2::TreeWalkResult::Ok
    })?;

    Ok(recorder)
}

fn lookup_entry_from_git_repo_commit_tree_by_path(
    repo: &git2::Repository,
    commit_oid: Option<git2::Oid>,
    target_path: &str,
) -> Result<Option<TreeEntry>, Box<dyn std::error::Error>> {
    // 确定要查找的 commit，如果没有指定则使用 HEAD
    let target_commit = match commit_oid {
        Some(oid) => repo.find_commit(oid)?,
        None => {
            let head = repo.head()?;
            let oid = head.target().unwrap();
            repo.find_commit(oid)?
        }
    };

    // 获取 commit 对应的 tree
    let tree = target_commit.tree()?;

    // 使用 get_path 方法查找指定路径的条目
    match tree.get_path(std::path::Path::new(target_path)) {
        Ok(tree_entry) => {
            let entry = TreeEntry {
                relative_path: target_path.to_string(),
                oid: tree_entry.id(),
                kind: tree_entry.kind().unwrap_or(git2::ObjectType::Any),
            };
            Ok(Some(entry))
        }
        Err(_) => Ok(None), // 路径不存在
    }
}

fn read_git_repo_blob_content(
    repo: &git2::Repository,
    blob_oid: git2::Oid,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // 根据 OID 查找 blob 对象
    let blob = repo.find_blob(blob_oid)?;

    // 获取 blob 的内容
    let content = blob.content().to_vec();

    Ok(content)
}

fn restore_git_repo_head_to_workdir(
    repo: &git2::Repository,
) -> Result<(), Box<dyn std::error::Error>> {
    // 获取 HEAD 引用
    let head_ref = repo.head()?;
    
    // 获取 HEAD 指向的 commit
    let head_commit = head_ref.peel_to_commit()?;
    
    // 获取 commit 的 tree
    let head_tree = head_commit.tree()?;
    
    // 使用 checkout 将工作目录恢复到 HEAD 状态
    repo.checkout_tree(
        head_tree.as_object(),
        Some(
            git2::build::CheckoutBuilder::new()
                .force() // 强制覆盖工作目录中的文件
                .remove_untracked(true) // 不移除未跟踪的文件
                .remove_ignored(false) // 不移除被忽略的文件
        ),
    )?;
    
    println!("已将工作目录恢复到 HEAD 状态");
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let test_dir = "/Users/bytedance/Workspace/ide/agent-e2e-cli";

    // rm -rf test_repo && mkdir -p test_repo && cd test_repo
    let test_dir = "test_repo";
    if Path::new(test_dir).exists() {
        fs::remove_dir_all(test_dir)?;
    }

    // git init
    let mut repo = open_or_init_git_repo(test_dir)?;
    println!("✓ 打开或创建了测试仓库: {}\n", test_dir);

    // git config user.name "TestUser"
    // git config user.email "test@example.com"
    config_git_repo_user(&mut repo, "TestUser", "test@example.com")?;
    config_git_repo_user(&mut repo, "TestUser", "test@example.com")?;
    println!("✓ 配置了用户信息\n");

    // touch test.txt
    // echo "hello world" > test.txt
    // echo "This is a test file" >> test.txt
    let file_relative_path = "test.txt";
    let file_path = repo.workdir().unwrap().join(file_relative_path);
    let file_content = "Hello, Git!\nThis is a test file";
    {
        std::fs::write(&file_path, file_content)?;
    }
    println!("创建了测试文件: {:?}", file_path);

    // git add test.txt
    let index = add_files_to_git_repo_index(&mut repo, vec![file_relative_path])?;
    println!("✓ 添加文件到 index\n");

    // git commit -m "测试提交的消息"
    let commit_id1 = commit_index_to_git_repo(&mut repo, index, "测试提交的消息")?;
    println!("✓ 创建了 commit: {}\n", commit_id1);

    // git tag test_tag_1 -m "测试创建 tag 消息"
    let tag_name = "test_tag_1";
    {
        let tag_ref = upsert_tag_to_git_repo(&mut repo, tag_name, "测试创建 tag 消息", None)?;
        println!("✓ tag 创建成功: {}\n", tag_ref.name().unwrap_or("unknown"));
    }

    // 创建分支 （不 checkout）
    // git branch test_branch_1
    let branch_name = "test_branch_1";
    {
        let branch_ref = upsert_branch_to_git_repo(&mut repo, branch_name, None)?;
        let branch_ref_name = branch_ref.name().unwrap_or("unknown").to_string();
        println!("✓ branch 创建成功: {}\n", branch_ref_name);
    }

    // 继续提交文件
    // touch test2.txt
    // echo "hello world" > test2.txt
    // echo "This is a test2 file" >> test2.txt
    let file_relative_path2 = "test2.txt";
    let file_path2 = repo.workdir().unwrap().join(file_relative_path2);
    let file_content2 = "Hello, Git!\nThis is a test2 file";
    {
        std::fs::write(&file_path2, file_content2)?;
    }
    println!("创建了测试文件2: {:?}", file_path2);
    // mkdir subdir
    // touch subdir/test3.txt
    // echo "hello world" > subdir/test3.txt
    // echo "This is a test3 file" >> subdir/test3.txt
    let dir_relative_path3 = "subdir";
    let dir_path3 = repo.workdir().unwrap().join(dir_relative_path3);
    fs::create_dir_all(&dir_path3)?;
    let file_relative_path3 = "subdir/test3.txt";
    let file_content3 = "Hello, Git!\nThis is a test3 file";
    let file_path3 = repo.workdir().unwrap().join(file_relative_path3);
    {
        std::fs::write(&file_path3, file_content3)?;
    }
    println!("创建了测试文件3: {:?}", file_path3);
    // rm -rf test.txt
    // git add test2.txt subdir/test3.txt test.txt
    fs::remove_file(file_path.clone())?;
    println!("删除测试文件1: {:?}", file_path);
    let index2 = add_files_to_git_repo_index(
        &mut repo,
        vec![file_relative_path, file_relative_path2, file_relative_path3],
    )?;
    println!("✓ 添加文件 2 文件 3 到 index, 文件 1 从 index 中移除\n");

    // git commit -m "测试提交的消息2"
    let commit_id2 = commit_index_to_git_repo(&mut repo, index2, "测试提交的消息2")?;
    println!("✓ 创建了 commit2: {}\n", commit_id2);
    let commit2_recorder = traverse_git_repo_commit_tree_recorder(&repo, Some(commit_id2))?;
    println!("✓ 遍历 commit2 树成功: {:?}\n", commit2_recorder);

    let entry = lookup_entry_from_git_repo_commit_tree_by_path(
        &repo,
        Some(commit_id2),
        file_relative_path3,
    )?;
    println!("✓ 从 commit2 树中查找文件 3 成功: {:?}\n", entry);
    let entry = entry.unwrap();
    let blob_content = read_git_repo_blob_content(&repo, entry.oid)?;
    println!(
        "✓ 从 blob 中读取文件 3 内容成功: {:?}\n",
        String::from_utf8_lossy(&blob_content)
    );

    // git branch test_branch_2
    let branch_name2 = "test_branch_2";
    {
        let branch_ref2 = upsert_branch_to_git_repo(&mut repo, branch_name2, None)?;
        let branch_ref_name2 = branch_ref2.name().unwrap_or("unknown").to_string();
        println!("✓ branch 创建成功: {}\n", branch_ref_name2);
    }

    // 切换到 test_branch_1 分支，并切换 workdir。
    // git checkout test_branch_1
    {
        let test_branch_1_ref = switch_git_repo_branch(&mut repo, branch_name, true)?;
        let test_branch_1_ref_name = test_branch_1_ref.name().unwrap_or("unknown").to_string();
        println!("✓ 已切换到分支: {} \n", test_branch_1_ref_name);
    }

    // 切换到 main 分支，并切换 workdir
    // git checkout main
    let main_branch = "main";
    {    
        let main_branch_ref = switch_git_repo_branch(&mut repo, main_branch, true)?;
        let main_branch_ref_name = main_branch_ref.name().unwrap_or("unknown").to_string();
        println!("✓ 已切换到分支: {} \n", main_branch_ref_name);
    }

    // 测试 reset hard
    // git reset --hard HEAD^1
    reset_git_repo_head(&mut repo, commit_id1)?;
    println!("✓ 已 reset hard 到 commit1: {:?}\n", commit_id1);

    // git rm --cached -r .
    let index3 = clean_git_repo_index(&mut repo)?;
    println!("✓ 已从 index 中移除所有文件\n");

    // git commit -m "清空所有文件"
    let commit_id3 = commit_index_to_git_repo(&mut repo, index3, "清空所有文件")?;
    println!("✓ 已创建 commit3: {}\n", commit_id3);

    // git restore .
    restore_git_repo_head_to_workdir(&repo)?;

    Ok(())
}
