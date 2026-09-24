fn main() {
    // 把 assets/rdesktop.ico 打进 exe(资源ID=1),Explorer 托盘/标题栏共用
    let _ = embed_resource::compile("assets/rdesktop.rc", embed_resource::NONE);
}
