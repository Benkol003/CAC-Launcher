


fn main() {
    let mut args = String::new();
    std::env::args().for_each(|x| {
        print!("{} ",x);
        args += format!("{} ",x).as_str();        
    });
    std::fs::write("main.log",args);
}