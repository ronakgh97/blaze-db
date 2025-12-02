pub async fn print_ascii() {
    let ascii_art = r#"
                                                 
    ▄▄    ▄▄                        ▄▄ ▄▄    
    ██    ██                        ██ ██    
    ████▄ ██  ▀▀█▄ ▀▀▀██ ▄█▀█▄   ▄████ ████▄ 
    ██ ██ ██ ▄█▀██   ▄█▀ ██▄█▀   ██ ██ ██ ██ 
    ████▀ ██ ▀█▄██ ▄██▄▄ ▀█▄▄▄   ▀████ ████▀ 
                                                                                  
    "#;

    println!("{}", ascii_art);
    println!();
    println!("Docs: https://github.com/ronakgh97/blaze-db");
    println!();
}
