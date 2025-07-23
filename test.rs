fn main() {
    let mut args = String::new();
    std::env::args().for_each(|x| {
        println!("{} ",x);
        args += format!("{}\n",x).as_str();        
    });
    std::fs::write("main.log",args);
}