#[test]
fn main() {
    let tmp = mktemp::Temp::new_dir().unwrap();
    let tmp = tmp.release();
    let tmp = tmp.to_str().unwrap();

    fn try_(res: eyre::Result<()>) {
        if let Err(e) = res {
            println!("{}", std::env::current_dir().unwrap().display());
            panic!("[error] {e:#}");
        }
    }

    let res = sharedfileholder::main_with_args(&["init", "-v", tmp]);
    try_(res);

    let res = sharedfileholder::main_with_args(&["backup", "-v", tmp, "backup_src", "./src"]);
    try_(res);
}
