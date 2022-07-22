use cargo_cage::CondaInfo;

fn main() {
    let conda_info = CondaInfo::try_new("conda").unwrap();
    println!("{:?}", conda_info);
}
