use log::*;

fn main() {
    env_logger::init();

    let path = "README.md";
    let content = std::fs::read_to_string(&path).unwrap();
    let mut tree = lsp_notebook::parse(&content);
    let actions = lsp_notebook::code_actions(&mut tree);
    info!("code: {:?}", actions);
}
